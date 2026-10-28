pub mod dto;
pub mod handlers;
pub mod idempotency;

use axum::{
    Router, middleware,
    routing::{get, post, put},
};
use tower_http::trace::TraceLayer;

use crate::{api_state::ApiState, http::idempotency::HttpIdempotencyStore};

pub fn router(state: ApiState) -> Router {
    let idempotency_store = HttpIdempotencyStore::new(
        state.rfq_summary_projection.database_pool(),
        std::sync::Arc::new(crate::providers::UuidV4Provider),
    );

    Router::new()
        .route("/health", get(handlers::health))
        .route("/api/rfqs", post(handlers::open_rfq))
        .route("/api/rfqs/summaries", get(handlers::list_rfq_summaries))
        .route("/api/rfqs/{rfq_id}/summary", get(handlers::get_rfq_summary))
        .route("/api/suppliers", post(handlers::register_supplier))
        .route("/api/rfqs/{rfq_id}/close", post(handlers::close_rfq))
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
        .with_state(state)
}
