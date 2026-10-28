use axum::Router;

pub mod command_rpus;
pub use command_rpus::CommandRpus;

use crate::{api_state::ApiState, http};

pub fn router(state: ApiState) -> Router {
    http::router(state)
}
