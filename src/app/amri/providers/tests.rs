use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::{
    ComplianceEvidenceReference, ComplianceProvider, ErpReferenceProvider,
    ErpSupplierReferenceEvidence, EvidenceFreshness, ExternalProviderError, ExternalProviderKind,
    FakeComplianceProvider, FakeErpReferenceProvider, FakeTaxProvider, ProviderEvidenceMetadata,
    ProviderLookup, TaxEvidenceReference, TaxProvider, classify_http_status,
};

fn metadata() -> ProviderEvidenceMetadata {
    ProviderEvidenceMetadata {
        supplier_id: Uuid::from_u128(1),
        source_revision: Some("revision-1".to_owned()),
        observed_at: Utc.with_ymd_and_hms(2026, 11, 17, 12, 0, 0).unwrap(),
        expires_at: None,
    }
}

#[test]
fn fakes_return_deterministic_typed_read_only_evidence() {
    let erp =
        FakeErpReferenceProvider::new(Ok(ProviderLookup::Found(ErpSupplierReferenceEvidence {
            metadata: metadata(),
            external_reference: "ERP-SUP-1".to_owned(),
        })));
    let compliance =
        FakeComplianceProvider::new(Ok(ProviderLookup::Found(ComplianceEvidenceReference {
            metadata: metadata(),
            evidence_reference: "COMP-EVIDENCE-1".to_owned(),
        })));
    let tax = FakeTaxProvider::new(Ok(ProviderLookup::Found(TaxEvidenceReference {
        metadata: metadata(),
        evidence_reference: "TAX-EVIDENCE-1".to_owned(),
    })));

    assert_eq!(
        erp.lookup_supplier_reference(Uuid::from_u128(1)).unwrap(),
        ProviderLookup::Found(ErpSupplierReferenceEvidence {
            metadata: metadata(),
            external_reference: "ERP-SUP-1".to_owned(),
        })
    );
    assert!(matches!(
        compliance.lookup_supplier_evidence(Uuid::from_u128(1)),
        Ok(ProviderLookup::Found(_))
    ));
    assert!(matches!(
        tax.lookup_supplier_evidence(Uuid::from_u128(1)),
        Ok(ProviderLookup::Found(_))
    ));
}

#[test]
fn fakes_preserve_not_found_not_applicable_and_operational_failures() {
    let not_found = FakeErpReferenceProvider::new(Ok(ProviderLookup::NotFound));
    let not_applicable = FakeComplianceProvider::new(Ok(ProviderLookup::NotApplicable));
    let unavailable = FakeTaxProvider::new(Err(ExternalProviderError::Unavailable {
        provider: ExternalProviderKind::Tax,
        status: Some(503),
    }));

    assert_eq!(
        not_found.lookup_supplier_reference(Uuid::from_u128(1)),
        Ok(ProviderLookup::NotFound)
    );
    assert_eq!(
        not_applicable.lookup_supplier_evidence(Uuid::from_u128(1)),
        Ok(ProviderLookup::NotApplicable)
    );
    assert_eq!(
        unavailable.lookup_supplier_evidence(Uuid::from_u128(1)),
        Err(ExternalProviderError::Unavailable {
            provider: ExternalProviderKind::Tax,
            status: Some(503),
        })
    );
}

#[test]
fn http_status_mapping_keeps_absence_distinct_from_transport_failures() {
    assert_eq!(
        classify_http_status(ExternalProviderKind::ErpReference, 404),
        Ok(ProviderLookup::NotFound)
    );
    assert_eq!(
        classify_http_status(ExternalProviderKind::Compliance, 204),
        Ok(ProviderLookup::NotApplicable)
    );
    assert_eq!(
        classify_http_status(ExternalProviderKind::Tax, 503),
        Err(ExternalProviderError::Unavailable {
            provider: ExternalProviderKind::Tax,
            status: Some(503),
        })
    );
    assert_eq!(
        classify_http_status(ExternalProviderKind::Tax, 504),
        Err(ExternalProviderError::TimedOut(ExternalProviderKind::Tax))
    );
    assert_eq!(
        classify_http_status(ExternalProviderKind::ErpReference, 400),
        Err(ExternalProviderError::InvalidResponse(
            ExternalProviderKind::ErpReference
        ))
    );
}

#[test]
fn evidence_freshness_uses_only_explicit_source_expiry() {
    let mut evidence = metadata();
    assert_eq!(
        evidence.freshness_at(Utc.with_ymd_and_hms(2026, 11, 17, 13, 0, 0).unwrap()),
        EvidenceFreshness::UnboundedBySource
    );
    evidence.expires_at = Some(Utc.with_ymd_and_hms(2026, 11, 17, 13, 0, 0).unwrap());
    assert_eq!(
        evidence.freshness_at(Utc.with_ymd_and_hms(2026, 11, 17, 12, 59, 59).unwrap()),
        EvidenceFreshness::Fresh
    );
    assert_eq!(
        evidence.freshness_at(Utc.with_ymd_and_hms(2026, 11, 17, 13, 0, 0).unwrap()),
        EvidenceFreshness::Stale
    );
}
