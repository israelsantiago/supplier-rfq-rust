//! Domain Capability / RPU: SubmitSupplierQuote.

mod append_consequences;
pub mod build_context;
pub mod decision;
mod load_context;
pub mod process_request;
pub use process_request as process;
pub mod request;

pub use decision::SubmissionRejection;
pub use process_request::{
    CommitError, PrepareError, PreparedSubmission, commit_prepared, prepare, process,
};
pub use request::{QuotedItemInput, SubmitSupplierQuoteRequest};
