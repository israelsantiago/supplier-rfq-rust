//! Deterministic in-memory provider fakes. No network, timers, Docker or database access.

use uuid::Uuid;

use super::{
    ComplianceEvidenceReference, ComplianceProvider, ErpReferenceProvider,
    ErpSupplierReferenceEvidence, ExternalProviderError, ProviderLookup, TaxEvidenceReference,
    TaxProvider,
};

#[derive(Clone)]
pub struct FakeErpReferenceProvider {
    result: Result<ProviderLookup<ErpSupplierReferenceEvidence>, ExternalProviderError>,
}

impl FakeErpReferenceProvider {
    pub fn new(
        result: Result<ProviderLookup<ErpSupplierReferenceEvidence>, ExternalProviderError>,
    ) -> Self {
        Self { result }
    }
}

impl ErpReferenceProvider for FakeErpReferenceProvider {
    fn lookup_supplier_reference(
        &self,
        _supplier_id: Uuid,
    ) -> Result<ProviderLookup<ErpSupplierReferenceEvidence>, ExternalProviderError> {
        self.result.clone()
    }
}

#[derive(Clone)]
pub struct FakeComplianceProvider {
    result: Result<ProviderLookup<ComplianceEvidenceReference>, ExternalProviderError>,
}

impl FakeComplianceProvider {
    pub fn new(
        result: Result<ProviderLookup<ComplianceEvidenceReference>, ExternalProviderError>,
    ) -> Self {
        Self { result }
    }
}

impl ComplianceProvider for FakeComplianceProvider {
    fn lookup_supplier_evidence(
        &self,
        _supplier_id: Uuid,
    ) -> Result<ProviderLookup<ComplianceEvidenceReference>, ExternalProviderError> {
        self.result.clone()
    }
}

#[derive(Clone)]
pub struct FakeTaxProvider {
    result: Result<ProviderLookup<TaxEvidenceReference>, ExternalProviderError>,
}

impl FakeTaxProvider {
    pub fn new(
        result: Result<ProviderLookup<TaxEvidenceReference>, ExternalProviderError>,
    ) -> Self {
        Self { result }
    }
}

impl TaxProvider for FakeTaxProvider {
    fn lookup_supplier_evidence(
        &self,
        _supplier_id: Uuid,
    ) -> Result<ProviderLookup<TaxEvidenceReference>, ExternalProviderError> {
        self.result.clone()
    }
}
