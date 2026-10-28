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
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use thiserror::Error;
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

// HTTP_IDEMPOTENCY_SETUP_ERROR: setup errors are explicit and do not hide migration drift.
#[derive(Debug, Error)]
pub enum HttpIdempotencySetupError {
    #[error("could not connect the HTTP idempotency pool: {0}")]
    Database(#[from] sqlx::Error),
    #[error(
        "HTTP idempotency migration {version} checksum mismatch: expected {expected}, found {actual}"
    )]
    MigrationChecksumMismatch {
        version: i64,
        expected: String,
        actual: String,
    },
}

const HTTP_IDEMPOTENCY_MIGRATION_VERSION: i64 = 202610270001;
const HTTP_IDEMPOTENCY_MIGRATION_LOCK: i64 = 202610270001;
const HTTP_IDEMPOTENCY_MIGRATION_SQL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/migrations_idempotency/202610270001_http_idempotency.sql"
));

impl HttpIdempotencyStore {
    /// Pool independente e limitado. Usa o mesmo banco, mas não o pool da projeção.
    pub async fn connect(
        database_url: &str,
        ids: Arc<dyn IdProvider>,
    ) -> Result<Self, HttpIdempotencySetupError> {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(database_url)
            .await?;
        initialize_http_idempotency_schema(&pool).await?;
        Ok(Self { pool, ids })
    }

    /// Construtor de baixo nível mantido para testes internos; produção deve usar `connect`.
    pub fn new(pool: PgPool, ids: Arc<dyn IdProvider>) -> Self {
        Self { pool, ids }
    }
}

async fn initialize_http_idempotency_schema(
    pool: &PgPool,
) -> Result<(), HttpIdempotencySetupError> {
    use sha2::{Digest, Sha256};
    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(HTTP_IDEMPOTENCY_MIGRATION_LOCK)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS http_idempotency_migrations (version BIGINT PRIMARY KEY, checksum TEXT NOT NULL, installed_at TIMESTAMPTZ NOT NULL DEFAULT now())")
        .execute(&mut *transaction).await?;
    let expected_checksum = encode_hex(&Sha256::digest(HTTP_IDEMPOTENCY_MIGRATION_SQL.as_bytes()));
    let installed_checksum = sqlx::query_scalar::<_, String>(
        "SELECT checksum FROM http_idempotency_migrations WHERE version = $1",
    )
    .bind(HTTP_IDEMPOTENCY_MIGRATION_VERSION)
    .fetch_optional(&mut *transaction)
    .await?;
    match installed_checksum {
        Some(actual) if actual == expected_checksum => {}
        Some(actual) => {
            return Err(HttpIdempotencySetupError::MigrationChecksumMismatch {
                version: HTTP_IDEMPOTENCY_MIGRATION_VERSION,
                expected: expected_checksum,
                actual,
            });
        }
        None => {
            // The legacy migration in `migrations/` is intentionally retained for SQLx history
            // compatibility. Its DDL is idempotent; this subsystem separately validates checksum.
            sqlx::raw_sql(HTTP_IDEMPOTENCY_MIGRATION_SQL)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(
                "INSERT INTO http_idempotency_migrations (version, checksum) VALUES ($1, $2)",
            )
            .bind(HTTP_IDEMPOTENCY_MIGRATION_VERSION)
            .bind(&expected_checksum)
            .execute(&mut *transaction)
            .await?;
        }
    }
    transaction.commit().await?;
    Ok(())
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
    use super::{
        HttpIdempotencyStore, canonicalize_json, claim_key, complete_key, encode_hex,
        inspect_existing, mark_unknown, valid_key,
    };
    use crate::providers::UuidV4Provider;
    use axum::{
        body::{Body, to_bytes},
        http::Response,
    };
    use serde_json::{Value, json};
    use std::sync::Arc;
    use testcontainers_modules::{
        postgres::Postgres,
        testcontainers::{ImageExt, runners::AsyncRunner},
    };

    #[test]
    fn idempotency_key_validation_accepts_visible_ascii_only() {
        assert!(valid_key("request-123"));
        assert!(valid_key("x"));
        assert!(valid_key(&"x".repeat(200)));
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

    async fn response_code(response: Response<Body>) -> String {
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        serde_json::from_slice::<Value>(&bytes).unwrap()["code"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[tokio::test]
    async fn ledger_state_machine_fails_closed_and_expires_only_completed_records() {
        let container = Postgres::default()
            .with_db_name("supplier_rfq")
            .with_user("rfq_user")
            .with_password("rfq_password")
            .with_tag("16-alpine")
            .start()
            .await
            .expect("PostgreSQL container must start; Docker must be available");
        let host = container
            .get_host()
            .await
            .expect("container host must exist");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("PostgreSQL port must be mapped");
        let url = format!("postgres://rfq_user:rfq_password@{host}:{port}/supplier_rfq");
        let store = HttpIdempotencyStore::connect(&url, Arc::new(UuidV4Provider))
            .await
            .expect("dedicated ledger migration and pool must initialize");
        let repeated = HttpIdempotencyStore::connect(&url, Arc::new(UuidV4Provider))
            .await
            .expect("startup must verify the previously applied migration checksum");
        drop(repeated);

        let dedicated_migrations: bool =
            sqlx::query_scalar("SELECT to_regclass('http_idempotency_migrations') IS NOT NULL")
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert!(dedicated_migrations, "ledger has its own checksum history");

        let scope = "POST /test-command";
        let fingerprint = "a".repeat(64);
        let key = "same-key-concurrent";
        let (one, two) = tokio::join!(
            claim_key(&store.pool, store.ids.as_ref(), scope, key, &fingerprint),
            claim_key(&store.pool, store.ids.as_ref(), scope, key, &fingerprint),
        );
        let one = one.unwrap();
        let two = two.unwrap();
        assert_ne!(
            one.is_some(),
            two.is_some(),
            "exactly one concurrent claim owns execution"
        );
        let owner = one.or(two).unwrap();
        let pending = inspect_existing(&store.pool, scope, key, &fingerprint).await;
        assert_eq!(pending.status(), axum::http::StatusCode::CONFLICT);
        assert_eq!(response_code(pending).await, "idempotency_in_progress");

        mark_unknown(&store.pool, scope, key, owner).await;
        sqlx::query("UPDATE http_idempotency_keys SET expires_at = now() - INTERVAL '1 hour' WHERE operation_scope = $1 AND idempotency_key = $2")
            .bind(scope).bind(key).execute(&store.pool).await.unwrap();
        assert!(
            claim_key(&store.pool, store.ids.as_ref(), scope, key, &fingerprint)
                .await
                .unwrap()
                .is_none(),
            "an expired unknown record must still block a second execution"
        );
        let unknown = inspect_existing(&store.pool, scope, key, &fingerprint).await;
        assert_eq!(response_code(unknown).await, "idempotency_outcome_unknown");

        let failed_key = "response-completion-fails";
        let failed_owner = claim_key(
            &store.pool,
            store.ids.as_ref(),
            scope,
            failed_key,
            &fingerprint,
        )
        .await
        .unwrap()
        .expect("new key must be claimed");
        sqlx::query("CREATE OR REPLACE FUNCTION fail_http_idempotency_completion() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.state = 'completed' THEN RAISE EXCEPTION 'injected completion failure'; END IF; RETURN NEW; END; $$")
            .execute(&store.pool).await.unwrap();
        sqlx::query("CREATE TRIGGER fail_http_idempotency_completion BEFORE UPDATE ON http_idempotency_keys FOR EACH ROW EXECUTE FUNCTION fail_http_idempotency_completion()")
            .execute(&store.pool).await.unwrap();
        let result = complete_key(
            &store.pool,
            scope,
            failed_key,
            failed_owner,
            201,
            &json!({"content-type":"application/json"}),
            br#"{"ok":true}"#,
        )
        .await;
        assert!(
            result.is_err(),
            "an injected completion write failure must surface"
        );
        mark_unknown(&store.pool, scope, failed_key, failed_owner).await;
        let failed = inspect_existing(&store.pool, scope, failed_key, &fingerprint).await;
        assert_eq!(response_code(failed).await, "idempotency_outcome_unknown");
        sqlx::query("DROP TRIGGER fail_http_idempotency_completion ON http_idempotency_keys")
            .execute(&store.pool)
            .await
            .unwrap();
        sqlx::query("DROP FUNCTION fail_http_idempotency_completion()")
            .execute(&store.pool)
            .await
            .unwrap();

        let completed_key = "completed-expiry";
        let completed_owner = claim_key(
            &store.pool,
            store.ids.as_ref(),
            scope,
            completed_key,
            &fingerprint,
        )
        .await
        .unwrap()
        .expect("new key must be claimed");
        complete_key(
            &store.pool,
            scope,
            completed_key,
            completed_owner,
            201,
            &json!({"content-type":"application/json"}),
            br#"{"created":true}"#,
        )
        .await
        .unwrap();
        sqlx::query("UPDATE http_idempotency_keys SET expires_at = now() - INTERVAL '1 second' WHERE operation_scope = $1 AND idempotency_key = $2")
            .bind(scope).bind(completed_key).execute(&store.pool).await.unwrap();
        assert!(
            claim_key(
                &store.pool,
                store.ids.as_ref(),
                scope,
                completed_key,
                &fingerprint
            )
            .await
            .unwrap()
            .is_some(),
            "expired completed rows may be reclaimed"
        );
        drop(store);
        drop(container);
    }

    #[tokio::test]
    async fn oversized_response_marks_key_unknown_and_blocks_retry() {
        let container = Postgres::default()
            .with_db_name("supplier_rfq")
            .with_user("rfq_user")
            .with_password("rfq_password")
            .with_tag("16-alpine")
            .start()
            .await
            .expect("PostgreSQL container must start; Docker must be available");
        let host = container
            .get_host()
            .await
            .expect("container host must exist");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("PostgreSQL port must be mapped");
        let url = format!("postgres://rfq_user:rfq_password@{host}:{port}/supplier_rfq");
        let store = HttpIdempotencyStore::connect(&url, Arc::new(UuidV4Provider))
            .await
            .unwrap();
        let router = axum::Router::new()
            .route(
                "/large",
                axum::routing::post(|| async {
                    (
                        axum::http::StatusCode::CREATED,
                        vec![b'x'; 4 * 1024 * 1024 + 1],
                    )
                }),
            )
            .route_layer(axum::middleware::from_fn_with_state(
                store.clone(),
                super::idempotency_middleware,
            ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let client = reqwest::Client::new();
        let first = client
            .post(format!("http://{address}/large"))
            .header("Idempotency-Key", "large-response-key")
            .send()
            .await
            .unwrap();
        assert_eq!(
            first.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            first.json::<serde_json::Value>().await.unwrap()["code"],
            "idempotency_response_unavailable"
        );
        let retry = client
            .post(format!("http://{address}/large"))
            .header("Idempotency-Key", "large-response-key")
            .send()
            .await
            .unwrap();
        assert_eq!(retry.status(), axum::http::StatusCode::CONFLICT);
        assert_eq!(
            retry.json::<serde_json::Value>().await.unwrap()["code"],
            "idempotency_outcome_unknown"
        );
        server.abort();
        drop(store);
        drop(container);
    }

    #[test]
    fn raw_body_fallback_has_deterministic_hex_encoding() {
        assert_eq!(encode_hex(&[0x00, 0x20, 0xff]), "0020ff");
    }
}
