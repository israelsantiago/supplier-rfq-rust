//! Contract integration tests for the project-owned ERP Reference, Compliance and Tax mocks.
//!
//! This executable is intentionally separate from `cargo test --lib`: it requires Docker and
//! starts the Rust `wiremock` server image through Testcontainers.

use serde::Deserialize;
use supplier_rfq_rpu_rust::app::amri::provider_test_support::{
    ComplianceEvidenceReference, ErpSupplierReferenceEvidence, ExternalProviderError,
    ExternalProviderKind, ProviderLookup, TaxEvidenceReference, classify_http_status,
};
use testcontainers::{
    GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};

const IMAGE: &str = "supplier-rfq-amri-wiremock-rs";
const IMAGE_TAG: &str = "0.6.5-stage5b2-v1";
const FOUND: &str = "00000000-0000-0000-0000-000000000001";
const NOT_APPLICABLE: &str = "00000000-0000-0000-0000-000000000002";
const UNAVAILABLE: &str = "00000000-0000-0000-0000-000000000003";
const TIMED_OUT: &str = "00000000-0000-0000-0000-000000000004";
const MALFORMED: &str = "00000000-0000-0000-0000-000000000005";
const NOT_FOUND: &str = "00000000-0000-0000-0000-000000000006";
const INVALID_STATUS: &str = "00000000-0000-0000-0000-000000000007";

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
    server: String,
    wiremock_version: String,
    contract_set: String,
}

fn erp_path(supplier_id: &str) -> String {
    format!("/v1/erp-reference/suppliers/{supplier_id}/reference")
}

fn compliance_path(supplier_id: &str) -> String {
    format!("/v1/compliance/suppliers/{supplier_id}/evidence")
}

fn tax_path(supplier_id: &str) -> String {
    format!("/v1/tax/suppliers/{supplier_id}/evidence")
}

async fn assert_failure_semantics(
    client: &reqwest::Client,
    base: &str,
    path: fn(&str) -> String,
    provider: ExternalProviderKind,
) {
    let not_found = client
        .get(format!("{base}{}", path(NOT_FOUND)))
        .send()
        .await
        .expect("WireMock not-found route must respond");
    let not_found_status = not_found.status().as_u16();
    assert_eq!(not_found_status, 404);
    let not_found_body: serde_json::Value =
        not_found.json().await.expect("404 problem must be JSON");
    assert_eq!(not_found_body["code"], "resource_not_found");
    assert_eq!(not_found_body["status"], 404);
    assert_eq!(
        classify_http_status(provider, not_found_status),
        Ok(ProviderLookup::NotFound)
    );

    let not_applicable = client
        .get(format!("{base}{}", path(NOT_APPLICABLE)))
        .send()
        .await
        .expect("WireMock not-applicable route must respond");
    assert_eq!(not_applicable.status().as_u16(), 204);
    assert_eq!(
        classify_http_status(provider, not_applicable.status().as_u16()),
        Ok(ProviderLookup::NotApplicable)
    );

    let unavailable = client
        .get(format!("{base}{}", path(UNAVAILABLE)))
        .send()
        .await
        .expect("WireMock unavailable route must respond");
    assert_eq!(unavailable.status().as_u16(), 503);
    assert_eq!(
        classify_http_status(provider, unavailable.status().as_u16()),
        Err(ExternalProviderError::Unavailable {
            provider,
            status: Some(503),
        })
    );

    let timed_out = client
        .get(format!("{base}{}", path(TIMED_OUT)))
        .send()
        .await
        .expect("WireMock gateway-timeout route must respond");
    assert_eq!(timed_out.status().as_u16(), 504);
    assert_eq!(
        classify_http_status(provider, timed_out.status().as_u16()),
        Err(ExternalProviderError::TimedOut(provider))
    );

    let malformed = client
        .get(format!("{base}{}", path(MALFORMED)))
        .send()
        .await
        .expect("WireMock malformed-body route must respond");
    assert_eq!(malformed.status().as_u16(), 200);
    assert_eq!(
        classify_http_status(provider, malformed.status().as_u16()),
        Ok(ProviderLookup::Found(())),
        "HTTP status is successful; payload validation remains a separate contract step"
    );
    let decode_result = malformed.json::<serde_json::Value>().await;
    assert!(decode_result.is_err());
    assert_eq!(
        decode_result
            .map(|_| ())
            .map_err(|_| ExternalProviderError::InvalidResponse(provider)),
        Err(ExternalProviderError::InvalidResponse(provider)),
        "malformed 2xx body must be mapped to InvalidResponse by a consuming HTTP adapter"
    );

    let invalid_status = client
        .get(format!("{base}{}", path(INVALID_STATUS)))
        .send()
        .await
        .expect("WireMock invalid-status route must respond");
    assert_eq!(invalid_status.status().as_u16(), 400);
    assert_eq!(
        classify_http_status(provider, invalid_status.status().as_u16()),
        Err(ExternalProviderError::InvalidResponse(provider))
    );
}

#[tokio::test]
async fn rust_wiremock_container_serves_all_three_versioned_provider_contracts() {
    let container = GenericImage::new(IMAGE, IMAGE_TAG)
        .with_wait_for(WaitFor::message_on_stdout("AMRI_WIREMOCK_RS_READY"))
        .with_exposed_port(8080.tcp())
        .with_label(
            "io.github.israelsantiago.supplier-rfq.test-fixture",
            "amri-wiremock-rs-v1",
        )
        .start()
        .await
        .expect("Rust wiremock-rs Testcontainer must start; Docker and the image must exist");

    let host = container
        .get_host()
        .await
        .expect("Testcontainers must expose the WireMock host");
    let port = container
        .get_host_port_ipv4(8080)
        .await
        .expect("Testcontainers must publish WireMock port 8080");
    let base = format!("http://{host}:{port}");
    let client = reqwest::Client::new();

    let health: HealthResponse = client
        .get(format!("{base}/healthz"))
        .send()
        .await
        .expect("health request must complete")
        .error_for_status()
        .expect("health endpoint must return 2xx")
        .json()
        .await
        .expect("health response must match its JSON contract");
    assert_eq!(health.status, "ok");
    assert_eq!(health.server, "wiremock-rs");
    assert_eq!(health.wiremock_version, "0.6.5");
    assert_eq!(health.contract_set, "amri-external-providers-v1");

    let erp: ErpSupplierReferenceEvidence = client
        .get(format!("{base}{}", erp_path(FOUND)))
        .send()
        .await
        .expect("ERP reference lookup must complete")
        .error_for_status()
        .expect("ERP reference contract must return 2xx")
        .json()
        .await
        .expect("ERP response must deserialize into the typed contract");
    assert_eq!(erp.metadata.supplier_id.to_string(), FOUND);
    assert_eq!(erp.external_reference, "ERP-SUPPLIER-REF-001");
    assert_eq!(erp.metadata.source_revision.as_deref(), Some("revision-1"));

    let compliance: ComplianceEvidenceReference = client
        .get(format!("{base}{}", compliance_path(FOUND)))
        .send()
        .await
        .expect("Compliance evidence lookup must complete")
        .error_for_status()
        .expect("Compliance contract must return 2xx")
        .json()
        .await
        .expect("Compliance response must deserialize into the typed contract");
    assert_eq!(compliance.metadata.supplier_id.to_string(), FOUND);
    assert_eq!(compliance.evidence_reference, "COMPLIANCE-EVIDENCE-001");

    let tax: TaxEvidenceReference = client
        .get(format!("{base}{}", tax_path(FOUND)))
        .send()
        .await
        .expect("Tax evidence lookup must complete")
        .error_for_status()
        .expect("Tax contract must return 2xx")
        .json()
        .await
        .expect("Tax response must deserialize into the typed contract");
    assert_eq!(tax.metadata.supplier_id.to_string(), FOUND);
    assert_eq!(tax.evidence_reference, "TAX-EVIDENCE-001");

    assert_failure_semantics(&client, &base, erp_path, ExternalProviderKind::ErpReference).await;
    assert_failure_semantics(
        &client,
        &base,
        compliance_path,
        ExternalProviderKind::Compliance,
    )
    .await;
    assert_failure_semantics(&client, &base, tax_path, ExternalProviderKind::Tax).await;

    // No HTTP endpoint or real external credentials are used; container teardown is RAII.
    drop(container);
}
