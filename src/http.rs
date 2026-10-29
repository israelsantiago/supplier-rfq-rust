pub mod dto;
pub mod handlers;
pub mod idempotency;

use axum::{
    Router,
    body::Body,
    http::{HeaderName, HeaderValue, Request},
    middleware,
    response::Response,
    routing::{get, post, put},
};
use tower_http::trace::TraceLayer;
use tracing::Instrument;
use uuid::Uuid;

/// Gera um identificador por requisição e o inclui no response e no span de tracing.
/// O identificador é sempre gerado pela aplicação; não confiamos em valores de entrada.
async fn request_id_middleware(request: Request<Body>, next: middleware::Next) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %request.method(),
        // Evita registrar query strings, que podem carregar dados sensíveis.
        path = %request.uri().path(),
    );
    let mut response = next.run(request).instrument(span).await;
    response.headers_mut().insert(
        HeaderName::from_static("x-request-id"),
        HeaderValue::from_bytes(request_id.as_bytes())
            .expect("a generated UUID is a valid header value"),
    );
    response
}

use crate::api_state::ApiState;

pub fn router(state: ApiState) -> Router {
    let idempotency_store = state.http_idempotency_store.clone();

    Router::new()
        .route("/health", get(handlers::health))
        .route("/api/rfqs", post(handlers::open_rfq))
        .route("/api/rfqs/summaries", get(handlers::list_rfq_summaries))
        .route("/api/rfqs/{rfq_id}/summary", get(handlers::get_rfq_summary))
        .route("/api/suppliers", post(handlers::register_supplier))
        .route("/api/rfqs/{rfq_id}/close", post(handlers::close_rfq))
        .route(
            "/api/rfqs/{rfq_id}/finalize-sourcing",
            post(handlers::finalize_rfq_sourcing),
        )
        .route(
            "/api/rfqs/{rfq_id}/quotes",
            post(handlers::submit_supplier_quote).get(handlers::list_supplier_quotes),
        )
        .route(
            "/api/suppliers/{supplier_id}/eligibility",
            put(handlers::set_supplier_eligibility),
        )
        .route_layer(middleware::from_fn_with_state(
            idempotency_store,
            idempotency::idempotency_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        // Colocado por fora da rota/middleware de idempotência para adicionar o ID
        // também às respostas antecipadas (por exemplo, chave inválida ou em andamento).
        .layer(middleware::from_fn(request_id_middleware))
        .with_state(state)
}

#[cfg(test)]
mod request_id_middleware_tests {
    use super::*;
    use axum::{http::StatusCode, routing::get};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn request_id_is_returned_even_for_an_internal_server_error() {
        let app = Router::new()
            .route(
                "/test-internal-error",
                get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
            )
            .layer(middleware::from_fn(request_id_middleware));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test HTTP server should run");
        });

        let response = reqwest::get(format!("http://{address}/test-internal-error"))
            .await
            .expect("test HTTP request should complete");
        assert_eq!(
            response.status().as_u16(),
            StatusCode::INTERNAL_SERVER_ERROR.as_u16()
        );
        let request_id = response
            .headers()
            .get("x-request-id")
            .expect("all responses should carry X-Request-Id")
            .to_str()
            .expect("request ID should be valid ASCII");
        assert!(
            Uuid::parse_str(request_id).is_ok(),
            "request ID should be a UUID"
        );
        server.abort();
    }
}
