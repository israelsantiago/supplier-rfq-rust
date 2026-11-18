use std::{io::Write, net::TcpListener};

use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const LISTEN_ADDRESS: &str = "0.0.0.0:8080";
const OBSERVED_AT: &str = "2026-11-17T12:00:00Z";
const FOUND: &str = "00000000-0000-0000-0000-000000000001";
const NOT_APPLICABLE: &str = "00000000-0000-0000-0000-000000000002";
const UNAVAILABLE: &str = "00000000-0000-0000-0000-000000000003";
const TIMED_OUT: &str = "00000000-0000-0000-0000-000000000004";
const MALFORMED: &str = "00000000-0000-0000-0000-000000000005";
const NOT_FOUND: &str = "00000000-0000-0000-0000-000000000006";
const INVALID_STATUS: &str = "00000000-0000-0000-0000-000000000007";

fn evidence_metadata(supplier_id: &str, revision: &str) -> serde_json::Value {
    json!({
        "supplier_id": supplier_id,
        "source_revision": revision,
        "observed_at": OBSERVED_AT,
        "expires_at": null
    })
}

async fn register_service(
    server: &MockServer,
    service_path: &str,
    service_name: &str,
    reference_field: &str,
    reference_value: &str,
) {
    let found_path = service_path.replace("{supplier_id}", FOUND);
    let not_found_path = service_path.replace("{supplier_id}", NOT_FOUND);
    let not_applicable_path = service_path.replace("{supplier_id}", NOT_APPLICABLE);
    let unavailable_path = service_path.replace("{supplier_id}", UNAVAILABLE);
    let timed_out_path = service_path.replace("{supplier_id}", TIMED_OUT);
    let malformed_path = service_path.replace("{supplier_id}", MALFORMED);
    let invalid_status_path = service_path.replace("{supplier_id}", INVALID_STATUS);

    let mut found_body = serde_json::Map::new();
    found_body.insert(
        "metadata".to_owned(),
        evidence_metadata(FOUND, "revision-1"),
    );
    found_body.insert(reference_field.to_owned(), json!(reference_value));

    Mock::given(method("GET"))
        .and(path(found_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(found_body))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(not_found_path))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "type": "about:blank",
            "title": "Not Found",
            "status": 404,
            "code": "resource_not_found",
            "service": service_name
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(not_applicable_path))
        .respond_with(ResponseTemplate::new(204))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(unavailable_path))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({
            "type": "about:blank",
            "title": "Service Unavailable",
            "status": 503,
            "code": "provider_unavailable",
            "service": service_name
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(timed_out_path))
        .respond_with(ResponseTemplate::new(504).set_body_json(json!({
            "type": "about:blank",
            "title": "Gateway Timeout",
            "status": 504,
            "code": "provider_timeout",
            "service": service_name
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(malformed_path))
        .respond_with(ResponseTemplate::new(200).set_body_string("{not-json"))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(invalid_status_path))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "type": "about:blank",
            "title": "Bad Request",
            "status": 400,
            "code": "invalid_lookup_request",
            "service": service_name
        })))
        .mount(server)
        .await;
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let listener = TcpListener::bind(LISTEN_ADDRESS)
        .unwrap_or_else(|error| panic!("could not bind {LISTEN_ADDRESS}: {error}"));
    let server = MockServer::builder().listener(listener).start().await;

    Mock::given(method("GET"))
        .and(path("/healthz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok",
            "server": "wiremock-rs",
            "wiremock_version": "0.6.5",
            "contract_set": "amri-external-providers-v1"
        })))
        .mount(&server)
        .await;

    register_service(
        &server,
        "/v1/erp-reference/suppliers/{supplier_id}/reference",
        "erp-reference",
        "external_reference",
        "ERP-SUPPLIER-REF-001",
    )
    .await;
    register_service(
        &server,
        "/v1/compliance/suppliers/{supplier_id}/evidence",
        "compliance",
        "evidence_reference",
        "COMPLIANCE-EVIDENCE-001",
    )
    .await;
    register_service(
        &server,
        "/v1/tax/suppliers/{supplier_id}/evidence",
        "tax",
        "evidence_reference",
        "TAX-EVIDENCE-001",
    )
    .await;

    println!(
        "AMRI_WIREMOCK_RS_READY address={LISTEN_ADDRESS} uri={}",
        server.uri()
    );
    std::io::stdout()
        .flush()
        .expect("readiness marker must reach Testcontainers");
    // The Testcontainers owner controls process lifetime; container teardown terminates us.
    std::future::pending::<()>().await;
}
