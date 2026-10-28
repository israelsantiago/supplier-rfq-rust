//! Command RPU: OpenRfq.
//! Abre uma RFQ imutável, protegendo a identidade com Command Context Consistency.

mod append_consequences;
mod build_context;
mod generate_consequences;
mod load_context;
mod process_request;

pub(crate) use process_request::process_with_providers;
pub use process_request::{OpenRfqError, OpenRfqRequest};
