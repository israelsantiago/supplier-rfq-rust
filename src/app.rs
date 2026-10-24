use axum::Router;

use crate::{application_state::AppState, http};

pub fn router(state: AppState) -> Router {
    http::router(state)
}
