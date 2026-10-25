//! Query RPU: GetRfqSummary.
//! Consulta o resumo projetado de uma RFQ.

mod load_context;
mod process_request;

pub use process_request::{GetRfqSummaryError, GetRfqSummaryRequest, process};
