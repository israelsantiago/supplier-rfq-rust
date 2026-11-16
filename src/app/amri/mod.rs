//! AMRI application coordination and context projection.
//!
//! The generic AMRI Core remains independent of RFQ, FACTSTR and storage.

pub mod execution_context;
pub mod rfq_sourcing_context;

pub use execution_context::{
    ExecutionContextFactoryError, RfqSourcingExecutionContextFactory,
    finalize_sourcing_request_fingerprint,
};
