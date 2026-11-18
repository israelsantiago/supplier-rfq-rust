//! AMRI application coordination and context projection.
//!
//! The generic AMRI Core remains independent of RFQ, FACTSTR and storage.

pub mod execution_context;
pub(crate) mod providers;
pub mod rfq_sourcing_context;

#[cfg(feature = "test-support")]
pub mod provider_test_support {
    //! Feature-gated re-exports for black-box provider contract tests only.
    pub use super::providers::{
        ComplianceEvidenceReference, ComplianceProvider, ErpReferenceProvider,
        ErpSupplierReferenceEvidence, ExternalProviderError, ExternalProviderKind,
        FakeComplianceProvider, FakeErpReferenceProvider, FakeTaxProvider,
        ProviderEvidenceMetadata, ProviderLookup, TaxEvidenceReference, TaxProvider,
        classify_http_status,
    };
}

pub use execution_context::{
    ExecutionContextFactoryError, RfqSourcingExecutionContextFactory,
    finalize_sourcing_request_fingerprint,
};
