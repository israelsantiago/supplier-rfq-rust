use axum::Router;

pub mod atomic_sourcing_flow;
pub mod command_rpus;
pub use command_rpus::CommandRpus;
pub mod query_rpus;
pub use query_rpus::QueryRpus;

use crate::{api_state::ApiState, http};

pub fn router(state: ApiState) -> Router {
    http::router(state)
}
