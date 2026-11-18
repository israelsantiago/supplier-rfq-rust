//! Application-owned, read-only ports for external reference/evidence sources.
//!
//! These ports deliberately provide evidence only. They do not evaluate domain policy,
//! reserve external resources, mutate remote systems, or participate in FACTSTR atomicity.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProviderKind {
    ErpReference,
    Compliance,
    Tax,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderLookup<T> {
    Found(T),
    NotFound,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExternalProviderError {
    #[error("external provider {provider:?} is unavailable (HTTP status: {status:?})")]
    Unavailable {
        provider: ExternalProviderKind,
        status: Option<u16>,
    },
    #[error("external provider {0:?} timed out")]
    TimedOut(ExternalProviderKind),
    #[error("external provider {0:?} returned an invalid response")]
    InvalidResponse(ExternalProviderKind),
}

/// Provenance shared by external read-only evidence contracts.
///
/// `expires_at` is source metadata. `freshness_at` evaluates only this explicit expiry;
/// it never infers a maximum age or turns staleness into a business rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceFreshness {
    Fresh,
    Stale,
    UnboundedBySource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEvidenceMetadata {
    pub supplier_id: Uuid,
    pub source_revision: Option<String>,
    pub observed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl ProviderEvidenceMetadata {
    /// Evaluates only an explicit source expiry. No universal max-age is inferred.
    pub fn freshness_at(&self, instant: DateTime<Utc>) -> EvidenceFreshness {
        match self.expires_at {
            Some(expires_at) if instant >= expires_at => EvidenceFreshness::Stale,
            Some(_) => EvidenceFreshness::Fresh,
            None => EvidenceFreshness::UnboundedBySource,
        }
    }
}

/// ERP lookup result: an opaque external reference mapped to an existing supplier identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErpSupplierReferenceEvidence {
    pub metadata: ProviderEvidenceMetadata,
    pub external_reference: String,
}

/// Compliance evidence pointer only. It deliberately contains no pass/fail or risk decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComplianceEvidenceReference {
    pub metadata: ProviderEvidenceMetadata,
    pub evidence_reference: String,
}

/// Tax evidence pointer only. It deliberately contains no tax code, rate, base or calculation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaxEvidenceReference {
    pub metadata: ProviderEvidenceMetadata,
    pub evidence_reference: String,
}

pub trait ErpReferenceProvider: Send + Sync {
    fn lookup_supplier_reference(
        &self,
        supplier_id: Uuid,
    ) -> Result<ProviderLookup<ErpSupplierReferenceEvidence>, ExternalProviderError>;
}

pub trait ComplianceProvider: Send + Sync {
    fn lookup_supplier_evidence(
        &self,
        supplier_id: Uuid,
    ) -> Result<ProviderLookup<ComplianceEvidenceReference>, ExternalProviderError>;
}

pub trait TaxProvider: Send + Sync {
    fn lookup_supplier_evidence(
        &self,
        supplier_id: Uuid,
    ) -> Result<ProviderLookup<TaxEvidenceReference>, ExternalProviderError>;
}

/// Maps the versioned HTTP status contract to semantic provider outcomes.
///
/// Body decoding is separate: a malformed 2xx payload must become `InvalidResponse`,
/// not `NotFound`, `NotApplicable`, or a business rejection.
pub fn classify_http_status(
    provider: ExternalProviderKind,
    status: u16,
) -> Result<ProviderLookup<()>, ExternalProviderError> {
    match status {
        200 => Ok(ProviderLookup::Found(())),
        204 => Ok(ProviderLookup::NotApplicable),
        404 => Ok(ProviderLookup::NotFound),
        504 => Err(ExternalProviderError::TimedOut(provider)),
        429 | 500..=599 => Err(ExternalProviderError::Unavailable {
            provider,
            status: Some(status),
        }),
        _ => Err(ExternalProviderError::InvalidResponse(provider)),
    }
}

#[cfg(any(test, feature = "test-support"))]
mod testing;

#[cfg(any(test, feature = "test-support"))]
pub use testing::{FakeComplianceProvider, FakeErpReferenceProvider, FakeTaxProvider};

#[cfg(test)]
mod tests;
