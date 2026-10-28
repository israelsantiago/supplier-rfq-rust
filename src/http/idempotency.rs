//! Durable HTTP idempotency for command routes when `Idempotency-Key` is supplied.
//!
//! The ledger intentionally fails closed for in-progress/unknown keys. FACTSTR and
//! this SQL ledger are not one transaction; a crash after append but before response
//! persistence must not trigger an automatic second execution.
use std::sync::Arc;

use axum::{
    Json,
    body::{Body, to_bytes},
    extract::{MatchedPath, State},
    http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::providers::IdProvider;

const MAX_KEY_BYTES: usize = 200;
const MAX_REQUEST_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_RESPONSE_BODY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone)]
pub struct HttpIdempotencyStore {
    pool: PgPool,
    ids: Arc<dyn IdProvider>,
}

impl HttpIdempotencyStore {
    pub fn new(pool: PgPool, ids: Arc<dyn IdProvider>) -> Self {
        Self { pool, ids }
    }
}

/// Axum route middleware. Requests without a key retain the existing API behavior;
/// clients opt in by sending a stable `Idempotency-Key` on a command request.
pub async fn idempotency_middleware(
    State(store): State<HttpIdempotencyStore>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    if !matches!(
        request.method().as_str(),
        "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        return next.run(request).await;
    }

    let key_count = request.headers().get_all("Idempotency-Key").iter().count();
    if key_count == 0 {
        return next.run(request).await;
    }
    if key_count != 1 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "invalid_idempotency_key",
            "Idempotency-Key must appear exactly once",
        );
    }
    let key = match request
        .headers()
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
    {
        Some(value) if valid_key(value) => value.to_owned(),
        _ => {
            return json_error(
                StatusCode::BAD_REQUEST,
                "invalid_idempotency_key",
                "Idempotency-Key must contain 1–200 visible ASCII characters",
            );
        }
    };

    let (parts, request_body) = request.into_parts();
    let body = match to_bytes(request_body, MAX_REQUEST_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return json_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "idempotency_request_too_large",
                "request body exceeds the idempotency middleware limit",
            );
        }
    };

    let method = parts.method.as_str().to_owned();
    let path = parts.uri.path().to_owned();
    let query = parts.uri.query().unwrap_or_default().to_owned();
    let content_type = parts
        .headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let route = parts
        .extensions
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_owned())
        .unwrap_or_else(|| path.clone());
    let scope = format!("{method} {route}");
    let body_value = serde_json::from_slice::<Value>(&body)
        .unwrap_or_else(|_| json!({"__raw_body_hex": encode_hex(&body)}));
    let fingerprint_payload = json!({
        "method": method,
        "route": route,
        "path": path,
        "query": query,
        "content_type": content_type,
        "body": body_value,
    });
    // Persist only the SHA-256 digest, never the request payload itself.
    let canonical_fingerprint = serde_json::to_vec(&canonicalize_json(fingerprint_payload))
        .expect("serializing a serde_json::Value must succeed");
    let fingerprint = encode_hex(&Sha256::digest(canonical_fingerprint));

    let owner_token = match claim_key(&store.pool, store.ids.as_ref(), &scope, &key, &fingerprint)
        .await
    {
        Ok(Some(token)) => token,
        Ok(None) => {
            return inspect_existing(&store.pool, &scope, &key, &fingerprint).await;
        }
        Err(error) => {
            tracing::error!(error = %error, operation_scope = %scope, "HTTP idempotency claim failed");
            return json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "idempotency_store_unavailable",
                "idempotency state could not be checked; command was not executed",
            );
        }
    };

    let request = Request::from_parts(parts, Body::from(body));
    let response = next.run(request).await;
    let status = response.status();
    let headers = cacheable_headers(response.headers());
    let bytes = match to_bytes(response.into_body(), MAX_RESPONSE_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(error) => {
            mark_unknown(&store.pool, &scope, &key, owner_token).await;
            tracing::error!(error = %error, operation_scope = %scope, "HTTP response exceeded cache limit or could not be read");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "idempotency_response_unavailable",
                "command outcome cannot be safely replayed; do not retry with a new key",
            );
        }
    };

    if status.is_server_error() {
        mark_unknown(&store.pool, &scope, &key, owner_token).await;
        return build_response(status.as_u16(), headers, bytes.to_vec(), false);
    }

    if let Err(error) = complete_key(
        &store.pool,
        &scope,
        &key,
        owner_token,
        status.as_u16(),
        &headers,
        bytes.as_ref(),
    )
    .await
    {
        // The domain command may already have appended a fact. Never release the key
        // for a second execution merely because response persistence failed.
        mark_unknown(&store.pool, &scope, &key, owner_token).await;
        tracing::error!(error = %error, operation_scope = %scope, "could not persist HTTP idempotency response");
    }

    build_response(status.as_u16(), headers, bytes.to_vec(), false)
}

/// Sort object keys recursively so a JSON body's property order cannot change its fingerprint.
fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut canonical = Map::new();
            for (key, nested) in entries {
                canonical.insert(key, canonicalize_json(nested));
            }
            Value::Object(canonical)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(canonicalize_json).collect()),
        scalar => scalar,
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MAX_KEY_BYTES
        && key.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

async fn claim_key(
    pool: &PgPool,
    ids: &dyn IdProvider,
    scope: &str,
    key: &str,
    fingerprint: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    // Only completed entries expire. Pending/unknown entries require reconciliation.
    sqlx::query(
        "DELETE FROM http_idempotency_keys \
         WHERE operation_scope = $1 AND idempotency_key = $2 \
           AND state = 'completed' AND expires_at <= now()",
    )
    .bind(scope)
    .bind(key)
    .execute(pool)
    .await?;

    for _ in 0..2 {
        let owner = ids.new_uuid();
        let inserted = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO http_idempotency_keys \
             (operation_scope, idempotency_key, request_fingerprint, state, owner_token, expires_at) \
             VALUES ($1, $2, $3, 'in_progress', $4, now() + INTERVAL '24 hours') \
             ON CONFLICT (operation_scope, idempotency_key) DO NOTHING \
             RETURNING owner_token",
        )
        .bind(scope)
        .bind(key)
        .bind(fingerprint)
        .bind(owner)
        .fetch_optional(pool)
        .await?;
        if let Some(token) = inserted {
            return Ok(Some(token));
        }
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM http_idempotency_keys \
             WHERE operation_scope = $1 AND idempotency_key = $2)",
        )
        .bind(scope)
        .bind(key)
        .fetch_one(pool)
        .await?;
        if exists {
            return Ok(None);
        }
    }
    Ok(None)
}

async fn inspect_existing(
    pool: &PgPool,
    scope: &str,
    key: &str,
    fingerprint: &str,
) -> Response<Body> {
    let row = match sqlx::query(
        "SELECT request_fingerprint, state, response_status, response_headers, response_body \
         FROM http_idempotency_keys WHERE operation_scope = $1 AND idempotency_key = $2",
    )
    .bind(scope)
    .bind(key)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "idempotency_state_unavailable",
                "idempotency state changed during lookup; retry the same key",
            );
        }
        Err(error) => {
            tracing::error!(error = %error, operation_scope = %scope, "HTTP idempotency lookup failed");
            return json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "idempotency_store_unavailable",
                "idempotency state could not be checked; command was not executed",
            );
        }
    };

    let stored_fingerprint: String = match row.try_get("request_fingerprint") {
        Ok(value) => value,
        Err(error) => return storage_error(error.to_string()),
    };
    if stored_fingerprint != fingerprint {
        return json_error(
            StatusCode::CONFLICT,
            "idempotency_key_reused",
            "this Idempotency-Key is already bound to a different request",
        );
    }

    let state: String = match row.try_get("state") {
        Ok(value) => value,
        Err(error) => return storage_error(error.to_string()),
    };
    match state.as_str() {
        "completed" => {
            let status: Option<i16> = row.try_get("response_status").unwrap_or(None);
            let headers: Option<Value> = row.try_get("response_headers").unwrap_or(None);
            let body: Option<Vec<u8>> = row.try_get("response_body").unwrap_or(None);
            match (status, headers, body) {
                (Some(status), Some(headers), Some(body)) => {
                    build_response(status as u16, headers, body, true)
                }
                _ => storage_error(
                    "completed idempotency record has incomplete response data".into(),
                ),
            }
        }
        "in_progress" => json_error(
            StatusCode::CONFLICT,
            "idempotency_in_progress",
            "a request with this key is still in progress; retry the same key later",
        ),
        "unknown" => json_error(
            StatusCode::CONFLICT,
            "idempotency_outcome_unknown",
            "the previous outcome is uncertain; the key is blocked pending reconciliation",
        ),
        _ => storage_error("idempotency record has an invalid state".into()),
    }
}

async fn complete_key(
    pool: &PgPool,
    scope: &str,
    key: &str,
    owner: Uuid,
    status: u16,
    headers: &Value,
    body: &[u8],
) -> Result<(), sqlx::Error> {
    let result = sqlx::query(
        "UPDATE http_idempotency_keys \
         SET state = 'completed', response_status = $4, response_headers = $5, \
             response_body = $6, updated_at = now(), expires_at = now() + INTERVAL '24 hours' \
         WHERE operation_scope = $1 AND idempotency_key = $2 AND owner_token = $3 AND state = 'in_progress'",
    )
    .bind(scope)
    .bind(key)
    .bind(owner)
    .bind(status as i16)
    .bind(headers.clone())
    .bind(body.to_vec())
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(sqlx::Error::Protocol(
            "idempotency record was not owned by this request".to_owned(),
        ));
    }
    Ok(())
}

async fn mark_unknown(pool: &PgPool, scope: &str, key: &str, owner: Uuid) {
    if let Err(error) = sqlx::query(
        "UPDATE http_idempotency_keys \
         SET state = 'unknown', response_status = NULL, response_headers = NULL, \
             response_body = NULL, updated_at = now() \
         WHERE operation_scope = $1 AND idempotency_key = $2 AND owner_token = $3 AND state = 'in_progress'",
    )
    .bind(scope)
    .bind(key)
    .bind(owner)
    .execute(pool)
    .await
    {
        tracing::error!(error = %error, operation_scope = %scope, "could not mark idempotency record unknown");
    }
}

fn cacheable_headers(headers: &HeaderMap) -> Value {
    let mut result = Map::new();
    for name in ["content-type", "cache-control", "location", "vary"] {
        if let Some(value) = headers.get(name) {
            if let Ok(value) = value.to_str() {
                result.insert(name.to_owned(), Value::String(value.to_owned()));
            }
        }
    }
    Value::Object(result)
}

fn build_response(status: u16, headers: Value, body: Vec<u8>, replayed: bool) -> Response<Body> {
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    if let Some(headers) = headers.as_object() {
        for (name, value) in headers {
            let (Ok(name), Some(value)) = (HeaderName::from_bytes(name.as_bytes()), value.as_str())
            else {
                continue;
            };
            if let Ok(value) = HeaderValue::from_str(value) {
                response.headers_mut().insert(name, value);
            }
        }
    }
    if replayed {
        response.headers_mut().insert(
            HeaderName::from_static("idempotency-replayed"),
            HeaderValue::from_static("true"),
        );
    }
    response
}

fn json_error(status: StatusCode, code: &'static str, message: &'static str) -> Response<Body> {
    (status, Json(json!({ "code": code, "message": message }))).into_response()
}

fn storage_error(message: String) -> Response<Body> {
    tracing::error!(error = %message, "invalid HTTP idempotency ledger record");
    json_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "idempotency_state_unavailable",
        "idempotency state could not be safely interpreted",
    )
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_json, encode_hex, valid_key};

    #[test]
    fn idempotency_key_validation_accepts_visible_ascii_only() {
        assert!(valid_key("request-123"));
        assert!(!valid_key(""));
        assert!(!valid_key("contains space"));
        assert!(!valid_key("non-ascii-ç"));
        assert!(!valid_key(&"x".repeat(201)));
    }

    #[test]
    fn canonical_json_recursively_sorts_object_keys_but_preserves_array_order() {
        let first: serde_json::Value =
            serde_json::from_str(r#"{"z":1,"nested":{"b":2,"a":3},"items":[{"y":1,"x":2},4]}"#)
                .unwrap();
        let second: serde_json::Value =
            serde_json::from_str(r#"{"items":[{"x":2,"y":1},4],"nested":{"a":3,"b":2},"z":1}"#)
                .unwrap();
        assert_eq!(canonicalize_json(first), canonicalize_json(second));
    }

    #[test]
    fn raw_body_fallback_has_deterministic_hex_encoding() {
        assert_eq!(encode_hex(&[0x00, 0x20, 0xff]), "0020ff");
    }
}
