//! Domain Capability / RPU: SubmitSupplierQuote.

mod append_consequences;
mod build_context;
mod decision;
mod load_context;
mod process_request;
mod request;

pub use decision::SubmissionRejection;
pub use process_request::PrepareError;
pub(crate) use process_request::process_with_providers;
pub use request::{QuotedItemInput, SubmitSupplierQuoteRequest};

/// Explicitly opt-in seam for deterministic integration tests of CCC.
/// Not part of the normal production API surface.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    pub use super::process_request::{CommitError, PreparedSubmission, commit_prepared, prepare};
}
