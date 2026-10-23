use axum::{
    Router,
    routing::{get, post},
};
use tower_http::trace::TraceLayer;

use crate::{application_state::AppState, http::handlers};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route(
            "/api/rfqs/{rfq_id}/quotes",
            post(handlers::submit_supplier_quote).get(handlers::list_supplier_quotes),
        )
        .route("/api/rfqs/{rfq_id}/close", post(handlers::close_rfq))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}
