//! Command RPU: CloseRfq.
//! Fecha uma RFQ sem aceitar decisões baseadas em contexto obsoleto.

mod append_consequences;
mod build_context;
mod generate_consequences;
mod load_context;
mod process_request;

pub use process_request::{CloseRfqError, CloseRfqRejection, CloseRfqRequest, process};
