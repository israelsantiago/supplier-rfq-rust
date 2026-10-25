//! Query RPU: ListRfqSummaries.
//! Valida os filtros e consulta uma página do resumo projetado.

mod build_context;
mod load_context;
mod process_request;

pub use process_request::{ListRfqSummariesError, ListRfqSummariesRequest, process};
