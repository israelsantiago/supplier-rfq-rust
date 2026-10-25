//! Query RPU: ListSupplierQuotes.
//! Lista cotações de uma RFQ a partir dos fatos persistidos.

mod build_context;
mod load_context;
mod process_request;

pub use process_request::{ListSupplierQuotesError, ListSupplierQuotesRequest, QuoteView, process};
