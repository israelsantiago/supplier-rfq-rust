use axum::Router;

use crate::{api_state::ApiState, http};

pub fn router(state: ApiState) -> Router {
    http::router(state)
}
