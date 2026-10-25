pub mod dto;
pub mod handlers;

use axum::{
    Router,
    routing::{get, post, put},
};
use tower_http::trace::TraceLayer;

use crate::api_state::ApiState;

pub fn router(state: ApiState) -> Router {
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
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
