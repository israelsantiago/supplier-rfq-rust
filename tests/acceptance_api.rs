use std::{net::SocketAddr, sync::Arc, time::Duration};

use chrono::{NaiveDate, Utc};
use factstr::{EventFilter, EventQuery, EventStore, NewEvent};
use factstr_postgres::PostgresStore;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use supplier_rfq_rpu_rust::{
    api_state::ApiState,
    app,
    application_state::factstr::FactstrApplicationStateProvider,
    capabilities::submit_supplier_quote::SubmitSupplierQuoteRequest,
    events::{
        RFQ_OPENED_V1, RfqItemFact, RfqOpenedFact, SUPPLIER_ELIGIBILITY_CHANGED_V1,
        SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
        SupplierRegisteredFact, SupplierStatus,
    },
    http::idempotency::HttpIdempotencyStore,
    projections::rfq_summary_projection::RfqSummaryProjection,
    providers::UuidV4Provider,
};
#[path = "common/postgres.rs"]
mod test_postgres;
use tokio::{net::TcpListener, sync::Barrier as TokioBarrier, task::JoinHandle};
use uuid::Uuid;

const RFQ_ID: &str = "11111111-1111-4111-8111-111111111111";
const SUPPLIER_ELIGIBLE_ID: &str = "22222222-2222-4222-8222-222222222222";
const SUPPLIER_BLOCKED_ID: &str = "33333333-3333-4333-8333-333333333333";
const ITEM_A_ID: &str = "44444444-4444-4444-8444-444444444441";
const ITEM_B_ID: &str = "44444444-4444-4444-8444-444444444442";

struct TestApp {
    base_url: String,
    database_url: String,
    store: Arc<PostgresStore>,
    _projection: Arc<RfqSummaryProjection>,
    server_task: JoinHandle<()>,
    // Identifica o banco exclusivo deste teste; o container é compartilhado pelo processo.
    _test_database: test_postgres::TestDatabase,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

async fn spawn_app() -> TestApp {
    let test_database = test_postgres::create_isolated_database().await;
    let database_url = test_database.database_url.clone();
    let url_for_connect = database_url.clone();
    let store = Arc::new(
        tokio::task::spawn_blocking(move || PostgresStore::connect(&url_for_connect))
            .await
            .expect("FACTSTR connection task must finish")
            .expect("FACTSTR must initialize its schema in PostgreSQL"),
    );
    seed_fixture(Arc::clone(&store)).await;
    let projection = Arc::new(
        RfqSummaryProjection::start(&database_url, Arc::clone(&store))
            .await
            .expect("RFQ summary projection must migrate and replay the durable stream"),
    );
    let idempotency_store = HttpIdempotencyStore::connect(&database_url, Arc::new(UuidV4Provider))
        .await
        .expect("HTTP idempotency ledger must use its own bounded pool and checksum history");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral port must be available");
    let address: SocketAddr = listener
        .local_addr()
        .expect("listener address must be available");
    let api = app::router(ApiState::from_shared_event_store(
        Arc::clone(&store),
        Arc::clone(&projection),
        idempotency_store,
    ));
    let server_task = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, api).await {
            panic!("test API server failed: {error}");
        }
    });

    TestApp {
        base_url: format!("http://{address}"),
        database_url: database_url.clone(),
        store,
        _projection: projection,
        server_task,
        _test_database: test_database,
    }
}

async fn seed_fixture(store: Arc<PostgresStore>) {
    tokio::task::spawn_blocking(move || {
        let rfq = RfqOpenedFact {
            rfq_id: Uuid::parse_str(RFQ_ID).unwrap(),
            reference: "RFQ-TEST-001".to_owned(),
            currency: "BRL".to_owned(),
            opened_at: Utc::now(),
            items: vec![
                RfqItemFact {
                    id: Uuid::parse_str(ITEM_A_ID).unwrap(),
                    sku: "SKU-A".to_owned(),
                    description: "Industrial component A".to_owned(),
                    required_quantity: 10,
                },
                RfqItemFact {
                    id: Uuid::parse_str(ITEM_B_ID).unwrap(),
                    sku: "SKU-B".to_owned(),
                    description: "Industrial component B".to_owned(),
                    required_quantity: 5,
                },
            ],
        };
        let eligible_supplier = SupplierRegisteredFact {
            supplier_id: Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap(),
            reference: "SUP-ELIGIBLE".to_owned(),
            name: "Supplier Eligible".to_owned(),
            registered_at: Utc::now(),
        };
        let eligible_status = SupplierEligibilityChangedFact {
            supplier_id: eligible_supplier.supplier_id,
            status: SupplierStatus::Eligible,
            changed_at: Utc::now(),
        };
        let blocked_supplier = SupplierRegisteredFact {
            supplier_id: Uuid::parse_str(SUPPLIER_BLOCKED_ID).unwrap(),
            reference: "SUP-BLOCKED".to_owned(),
            name: "Supplier Blocked".to_owned(),
            registered_at: Utc::now(),
        };
        let blocked_status = SupplierEligibilityChangedFact {
            supplier_id: blocked_supplier.supplier_id,
            status: SupplierStatus::Blocked,
            changed_at: Utc::now(),
        };

        store
            .as_ref()
            .append(vec![
                NewEvent::new(RFQ_OPENED_V1, json!(rfq)),
                NewEvent::new(SUPPLIER_REGISTERED_V1, json!(eligible_supplier)),
                NewEvent::new(SUPPLIER_ELIGIBILITY_CHANGED_V1, json!(eligible_status)),
                NewEvent::new(SUPPLIER_REGISTERED_V1, json!(blocked_supplier)),
                NewEvent::new(SUPPLIER_ELIGIBILITY_CHANGED_V1, json!(blocked_status)),
            ])
            .expect("fixture facts must append successfully");
    })
    .await
    .expect("fixture seeding task must finish");
}

fn valid_request(supplier_id: &str) -> Value {
    json!({
        "supplier_id": supplier_id,
        "currency": "BRL",
        "delivery_days": 7,
        "valid_until": NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
        "items": [
            { "rfq_item_id": ITEM_A_ID, "quantity": 10, "unit_price_cents": 1250 },
            { "rfq_item_id": ITEM_B_ID, "quantity": 5, "unit_price_cents": 3000 }
        ]
    })
}

fn quote_context_query(rfq_id: Uuid, supplier_id: Uuid) -> EventQuery {
    EventQuery::all().with_filters([EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1])
        .with_payload_predicates([json!({ "rfq_id": rfq_id, "supplier_id": supplier_id })])])
}

#[tokio::test]
async fn acceptance_opens_rfq_registers_supplier_sets_eligibility_and_submits_quote_via_rest() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let supplier_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();

    let opened = client
        .post(format!("{}/api/rfqs", app.base_url))
        .json(&json!({
            "id": rfq_id,
            "reference": "RFQ-HTTP-CREATED",
            "currency": "BRL",
            "items": [{
                "id": item_id,
                "sku": "SKU-HTTP",
                "description": "Item created through REST",
                "required_quantity": 3
            }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::CREATED);
    let opened_body: Value = opened.json().await.unwrap();
    assert_eq!(opened_body["rfq_id"], rfq_id.to_string());

    let registered = client
        .post(format!("{}/api/suppliers", app.base_url))
        .json(&json!({
            "id": supplier_id,
            "reference": "SUP-HTTP",
            "name": "Supplier created through REST"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(registered.status(), StatusCode::CREATED);
    let registered_body: Value = registered.json().await.unwrap();
    assert_eq!(registered_body["supplier_id"], supplier_id.to_string());

    let eligibility = client
        .put(format!(
            "{}/api/suppliers/{supplier_id}/eligibility",
            app.base_url
        ))
        .json(&json!({ "status": "eligible" }))
        .send()
        .await
        .unwrap();
    assert_eq!(eligibility.status(), StatusCode::OK);

    let quote = client
        .post(format!("{}/api/rfqs/{rfq_id}/quotes", app.base_url))
        .json(&json!({
            "supplier_id": supplier_id,
            "currency": "BRL",
            "delivery_days": 4,
            "valid_until": "2099-12-31",
            "items": [{ "rfq_item_id": item_id, "quantity": 3, "unit_price_cents": 700 }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(quote.status(), StatusCode::CREATED);
    let quote_body: Value = quote.json().await.unwrap();
    assert_eq!(quote_body["total_amount_cents"], 2_100);
}

#[tokio::test]
async fn acceptance_submits_quote_and_query_returns_event_sourced_view() {
    let app = spawn_app().await;
    let client = Client::new();
    let submit = client
        .post(format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url))
        .json(&valid_request(SUPPLIER_ELIGIBLE_ID))
        .send()
        .await
        .unwrap();
    assert_eq!(submit.status(), StatusCode::CREATED);
    let body: Value = submit.json().await.unwrap();
    assert_eq!(body["total_amount_cents"], 27_500);
    assert_eq!(body["currency"], "BRL");

    let query = client
        .get(format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(query.status(), StatusCode::OK);
    let quotes: Value = query.json().await.unwrap();
    assert_eq!(quotes["quotes"].as_array().unwrap().len(), 1);
    assert_eq!(quotes["quotes"][0]["total_amount_cents"], 27_500);
    assert!(quotes["quotes"][0]["sequence_number"].as_u64().is_some());
}

#[tokio::test]
async fn acceptance_rejects_blocked_supplier_without_appending_quote_fact() {
    let app = spawn_app().await;
    let client = Client::new();
    let response = client
        .post(format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url))
        .json(&valid_request(SUPPLIER_BLOCKED_ID))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["code"], "supplier_not_eligible");

    let query = quote_context_query(
        Uuid::parse_str(RFQ_ID).unwrap(),
        Uuid::parse_str(SUPPLIER_BLOCKED_ID).unwrap(),
    );
    let query_result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();
    assert!(
        query_result.event_records.is_empty(),
        "a rejected command must append no quote fact"
    );
}

#[tokio::test]
async fn acceptance_requires_registration_and_eligibility_before_quote_submission() {
    let app = spawn_app().await;
    let client = Client::new();
    let supplier_id = Uuid::new_v4();
    let quote_url = format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url);

    let unregistered = client
        .post(quote_url.clone())
        .json(&valid_request(&supplier_id.to_string()))
        .send()
        .await
        .unwrap();
    assert_eq!(unregistered.status(), StatusCode::NOT_FOUND);
    let error: Value = unregistered.json().await.unwrap();
    assert_eq!(error["code"], "supplier_not_registered");

    let registered = client
        .post(format!("{}/api/suppliers", app.base_url))
        .json(&json!({
            "id": supplier_id,
            "reference": "SUP-REGISTERED-NO-ELIGIBILITY",
            "name": "Registered, eligibility not yet set"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(registered.status(), StatusCode::CREATED);

    let no_eligibility = client
        .post(quote_url.clone())
        .json(&valid_request(&supplier_id.to_string()))
        .send()
        .await
        .unwrap();
    assert_eq!(no_eligibility.status(), StatusCode::CONFLICT);
    let error: Value = no_eligibility.json().await.unwrap();
    assert_eq!(error["code"], "supplier_eligibility_not_set");

    let quote_query = quote_context_query(Uuid::parse_str(RFQ_ID).unwrap(), supplier_id);
    let persisted = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&quote_query)
    })
    .await
    .unwrap()
    .unwrap();
    assert!(
        persisted.event_records.is_empty(),
        "rejected submissions must not append quote facts"
    );
}

#[tokio::test]
async fn acceptance_rejects_duplicate_natural_references_without_appending_facts() {
    let app = spawn_app().await;
    let client = Client::new();

    let duplicate_rfq_reference = client
        .post(format!("{}/api/rfqs", app.base_url))
        .json(&json!({
            "id": Uuid::new_v4(),
            "reference": "RFQ-TEST-001",
            "currency": "BRL",
            "items": [{
                "id": Uuid::new_v4(),
                "sku": "SKU-NATURAL-KEY-TEST",
                "description": "Natural-key collision test",
                "required_quantity": 1
            }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate_rfq_reference.status(), StatusCode::CONFLICT);
    let rfq_error: Value = duplicate_rfq_reference.json().await.unwrap();
    assert_eq!(rfq_error["code"], "rfq_reference_already_exists");

    let duplicate_supplier_reference = client
        .post(format!("{}/api/suppliers", app.base_url))
        .json(&json!({
            "id": Uuid::new_v4(),
            "reference": "SUP-ELIGIBLE",
            "name": "A different supplier using a taken reference"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate_supplier_reference.status(), StatusCode::CONFLICT);
    let supplier_error: Value = duplicate_supplier_reference.json().await.unwrap();
    assert_eq!(supplier_error["code"], "supplier_reference_already_exists");

    let rfq_reference_query = EventQuery::all()
        .with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
            .with_payload_predicates([json!({ "reference": "RFQ-TEST-001" })])]);
    let rfq_events = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&rfq_reference_query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        rfq_events.event_records.len(),
        1,
        "one RFQ reference means one opening fact"
    );

    let supplier_reference_query = EventQuery::all()
        .with_filters([EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
            .with_payload_predicates([json!({ "reference": "SUP-ELIGIBLE" })])]);
    let supplier_events = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&supplier_reference_query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        supplier_events.event_records.len(),
        1,
        "one supplier reference means one registration fact"
    );
}

#[tokio::test]
async fn acceptance_duplicate_http_submissions_return_one_created_and_one_conflict() {
    let app = spawn_app().await;
    let client = Client::new();
    let barrier = Arc::new(TokioBarrier::new(3));
    let url = format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url);

    let barrier1 = Arc::clone(&barrier);
    let client1 = client.clone();
    let url1 = url.clone();
    let request1 = tokio::spawn(async move {
        barrier1.wait().await;
        client1
            .post(url1)
            .json(&valid_request(SUPPLIER_ELIGIBLE_ID))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    });

    let barrier2 = Arc::clone(&barrier);
    let request2 = tokio::spawn(async move {
        barrier2.wait().await;
        client
            .post(url)
            .json(&valid_request(SUPPLIER_ELIGIBLE_ID))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    });
    barrier.wait().await;

    let mut statuses = vec![request1.await.unwrap(), request2.await.unwrap()];
    statuses.sort_unstable();
    assert_eq!(statuses, vec![201, 409]);

    let query = quote_context_query(
        Uuid::parse_str(RFQ_ID).unwrap(),
        Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap(),
    );
    let result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        result.event_records.len(),
        1,
        "exactly one quote fact must exist"
    );
}

#[tokio::test]
async fn concurrent_close_and_submission_preserve_rfq_context_consistency() {
    let app = spawn_app().await;
    let client = Client::new();
    let barrier = Arc::new(TokioBarrier::new(3));
    let close_url = format!("{}/api/rfqs/{RFQ_ID}/close", app.base_url);
    let quote_url = format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url);

    let close_barrier = Arc::clone(&barrier);
    let close_client = client.clone();
    let close = tokio::spawn(async move {
        close_barrier.wait().await;
        close_client.post(close_url).send().await.unwrap()
    });
    let quote_barrier = Arc::clone(&barrier);
    let quote = tokio::spawn(async move {
        quote_barrier.wait().await;
        client
            .post(quote_url)
            .json(&valid_request(SUPPLIER_ELIGIBLE_ID))
            .send()
            .await
            .unwrap()
    });
    barrier.wait().await;

    let close_response = close.await.unwrap();
    assert_eq!(close_response.status(), StatusCode::OK);
    let quote_response = quote.await.unwrap();
    let quote_status = quote_response.status();
    assert!(quote_status == StatusCode::CREATED || quote_status == StatusCode::CONFLICT);
    if quote_status == StatusCode::CONFLICT {
        let body: Value = quote_response.json().await.unwrap();
        assert!(body["code"] == "rfq_closed" || body["code"] == "command_context_changed");
    }

    let rfq_query = EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([json!({ "rfq_id": RFQ_ID })])]);
    let rfq_result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&rfq_query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        rfq_result.event_records.len(),
        1,
        "RFQ opening fact must remain singular"
    );

    let quote_query = quote_context_query(
        Uuid::parse_str(RFQ_ID).unwrap(),
        Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap(),
    );
    let quote_result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&quote_query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        quote_result.event_records.len(),
        if quote_status == StatusCode::CREATED {
            1
        } else {
            0
        }
    );
}

async fn wait_for_summary(
    client: &Client,
    base_url: &str,
    rfq_id: Uuid,
    predicate: impl Fn(&Value) -> bool,
) -> Value {
    for _ in 0..120 {
        let response = client
            .get(format!("{base_url}/api/rfqs/{rfq_id}/summary"))
            .send()
            .await
            .expect("summary request must complete");
        if response.status().is_success() {
            let body: Value = response.json().await.expect("summary must be JSON");
            if predicate(&body) {
                return body;
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("RFQ summary projection did not reach the expected state for {rfq_id}");
}

#[tokio::test]
async fn acceptance_rfq_summary_projection_tracks_open_quote_close_and_paginated_reads() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let supplier_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let reference = format!("RFQ-PROJECTION-{}", Uuid::new_v4());

    let opened = client
        .post(format!("{}/api/rfqs", app.base_url))
        .json(&json!({
            "id": rfq_id,
            "reference": reference,
            "currency": "BRL",
            "items": [{
                "id": item_id,
                "sku": "SKU-PROJECTION-1",
                "description": "Projection test item",
                "required_quantity": 2
            }]
        }))
        .send()
        .await
        .expect("open RFQ request must complete");
    assert_eq!(opened.status(), StatusCode::CREATED);

    let opened_summary = wait_for_summary(&client, &app.base_url, rfq_id, |summary| {
        summary["status"] == "open" && summary["item_count"] == 1
    })
    .await;
    assert_eq!(opened_summary["quote_count"], 0);
    assert_eq!(opened_summary["lowest_quote_amount_cents"], Value::Null);

    let registered = client
        .post(format!("{}/api/suppliers", app.base_url))
        .json(&json!({
            "id": supplier_id,
            "reference": format!("SUP-PROJECTION-{}", Uuid::new_v4()),
            "name": "Projection Test Supplier"
        }))
        .send()
        .await
        .expect("supplier registration must complete");
    assert_eq!(registered.status(), StatusCode::CREATED);

    let eligibility = client
        .put(format!(
            "{}/api/suppliers/{supplier_id}/eligibility",
            app.base_url
        ))
        .json(&json!({ "status": "eligible" }))
        .send()
        .await
        .expect("supplier eligibility request must complete");
    assert_eq!(eligibility.status(), StatusCode::OK);

    let quote = client
        .post(format!("{}/api/rfqs/{rfq_id}/quotes", app.base_url))
        .json(&json!({
            "supplier_id": supplier_id,
            "currency": "BRL",
            "delivery_days": 5,
            "valid_until": "2099-12-31",
            "items": [{
                "rfq_item_id": item_id,
                "quantity": 2,
                "unit_price_cents": 1750
            }]
        }))
        .send()
        .await
        .expect("quote submission must complete");
    assert_eq!(quote.status(), StatusCode::CREATED);

    let quoted_summary = wait_for_summary(&client, &app.base_url, rfq_id, |summary| {
        summary["quote_count"] == 1 && summary["lowest_quote_amount_cents"] == 3500
    })
    .await;
    assert_eq!(
        quoted_summary["lowest_quote_supplier_id"],
        supplier_id.to_string()
    );
    assert!(
        quoted_summary["last_event_sequence"].as_i64().unwrap()
            > opened_summary["last_event_sequence"].as_i64().unwrap()
    );

    let closed = client
        .post(format!("{}/api/rfqs/{rfq_id}/close", app.base_url))
        .send()
        .await
        .expect("close RFQ request must complete");
    assert_eq!(closed.status(), StatusCode::OK);

    let closed_summary = wait_for_summary(&client, &app.base_url, rfq_id, |summary| {
        summary["status"] == "closed" && !summary["closed_at"].is_null()
    })
    .await;
    assert_eq!(closed_summary["quote_count"], 1);

    let page_response = client
        .get(format!(
            "{}/api/rfqs/summaries?status=closed&limit=10&offset=0",
            app.base_url
        ))
        .send()
        .await
        .expect("list summary request must complete");
    let page_status = page_response.status();
    let page_body = page_response
        .text()
        .await
        .expect("summary list response body must be readable");

    assert_eq!(
        page_status,
        StatusCode::OK,
        "GET /api/rfqs/summaries failed: status={page_status}, body={page_body}"
    );

    let page: Value =
        serde_json::from_str(&page_body).expect("summary list endpoint must return valid JSON");
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["rfq_id"], rfq_id.to_string());

    let invalid_status = client
        .get(format!(
            "{}/api/rfqs/summaries?status=cancelled",
            app.base_url
        ))
        .send()
        .await
        .expect("invalid filter request must complete");
    assert_eq!(invalid_status.status(), StatusCode::BAD_REQUEST);
}

/// FACTSTR EventQuery: filtros distintos representam alternativas (OR).
#[tokio::test]
async fn event_query_ors_separate_filters() {
    let app = spawn_app().await;
    let rfq_id = Uuid::parse_str(RFQ_ID).unwrap();
    let supplier_id = Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap();
    let query = EventQuery::all().with_filters([
        EventFilter::for_event_types([RFQ_OPENED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
        EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
            .with_payload_predicates([json!({ "supplier_id": supplier_id })]),
    ]);

    let result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.event_records.len(), 2);
    assert!(
        result
            .event_records
            .iter()
            .any(|record| record.event_type.as_str() == RFQ_OPENED_V1)
    );
    assert!(result.event_records.iter().any(|record| {
        record.event_type.as_str() == SUPPLIER_REGISTERED_V1
            && record.payload.get("supplier_id").and_then(Value::as_str)
                == Some(SUPPLIER_ELIGIBLE_ID)
    }));
}

/// Dentro de um EventFilter, tipos e predicados são alternativas (OR).
#[tokio::test]
async fn event_filter_ors_event_types_and_payload_predicates() {
    let app = spawn_app().await;
    let rfq_id = Uuid::parse_str(RFQ_ID).unwrap();
    let supplier_id = Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap();
    let query = EventQuery::all().with_filters([EventFilter::for_event_types([
        RFQ_OPENED_V1,
        SUPPLIER_REGISTERED_V1,
    ])
    .with_payload_predicates([
        json!({ "rfq_id": rfq_id }),
        json!({ "supplier_id": supplier_id }),
    ])]);

    let result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(result.event_records.len(), 2);
    assert!(
        result
            .event_records
            .iter()
            .any(|record| record.event_type.as_str() == RFQ_OPENED_V1)
    );
    assert!(
        result
            .event_records
            .iter()
            .any(|record| record.event_type.as_str() == SUPPLIER_REGISTERED_V1)
    );
}

/// Tipo de evento e payload são condições conjuntas (AND), não alternativas.
#[tokio::test]
async fn event_filter_requires_event_type_and_payload_match() {
    let app = spawn_app().await;
    let supplier_id = Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap();
    // O payload supplier_id existe em supplier_registered.v1, mas não em rfq_opened.v1.
    let query = EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([json!({ "supplier_id": supplier_id })])]);

    let result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();

    assert!(
        result.event_records.is_empty(),
        "a matching payload on another event type must not satisfy this filter"
    );
}

struct ConflictOnceStore {
    inner: Arc<PostgresStore>,
    supplier_id: Uuid,
    injected: Arc<std::sync::atomic::AtomicBool>,
    synthetic_conflict: bool,
}

impl factstr::EventStore for ConflictOnceStore {
    fn query(
        &self,
        query: &factstr::EventQuery,
    ) -> Result<factstr::QueryResult, factstr::EventStoreError> {
        self.inner.query(query)
    }

    fn append(
        &self,
        events: Vec<factstr::NewEvent>,
    ) -> Result<factstr::AppendResult, factstr::EventStoreError> {
        self.inner.append(events)
    }

    fn append_if(
        &self,
        events: Vec<factstr::NewEvent>,
        query: &factstr::EventQuery,
        expected_context_version: Option<u64>,
    ) -> Result<factstr::AppendResult, factstr::EventStoreError> {
        if !self
            .injected
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            if self.synthetic_conflict {
                return Err(factstr::EventStoreError::ConditionalAppendConflict {
                    expected: expected_context_version,
                    actual: expected_context_version
                        .map(|version| version.saturating_add(1))
                        .or(Some(1)),
                });
            }
            let harmless_eligibility_reaffirmation = SupplierEligibilityChangedFact {
                supplier_id: self.supplier_id,
                status: SupplierStatus::Eligible,
                changed_at: chrono::DateTime::parse_from_rfc3339("2026-10-27T12:00:00Z")
                    .expect("fixed timestamp is valid")
                    .with_timezone(&Utc),
            };
            self.inner.append(vec![NewEvent::new(
                SUPPLIER_ELIGIBILITY_CHANGED_V1,
                json!(harmless_eligibility_reaffirmation),
            )])?;
        }
        self.inner
            .append_if(events, query, expected_context_version)
    }

    fn stream_all(
        &self,
        handler: factstr::HandleStream,
    ) -> Result<factstr::EventStream, factstr::EventStoreError> {
        self.inner.stream_all(handler)
    }

    fn stream_to(
        &self,
        query: &factstr::EventQuery,
        handler: factstr::HandleStream,
    ) -> Result<factstr::EventStream, factstr::EventStoreError> {
        self.inner.stream_to(query, handler)
    }

    fn stream_all_durable(
        &self,
        stream: &factstr::DurableStream,
        handler: factstr::HandleStream,
    ) -> Result<factstr::EventStream, factstr::EventStoreError> {
        self.inner.stream_all_durable(stream, handler)
    }

    fn stream_to_durable(
        &self,
        stream: &factstr::DurableStream,
        query: &factstr::EventQuery,
        handler: factstr::HandleStream,
    ) -> Result<factstr::EventStream, factstr::EventStoreError> {
        self.inner.stream_to_durable(stream, query, handler)
    }
}

struct CountingFixedClock {
    instant: chrono::DateTime<Utc>,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl supplier_rfq_rpu_rust::providers::ClockProvider for CountingFixedClock {
    fn now_utc(&self) -> chrono::DateTime<Utc> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.instant
    }
}

struct CountingFixedId {
    id: Uuid,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl supplier_rfq_rpu_rust::providers::IdProvider for CountingFixedId {
    fn new_uuid(&self) -> Uuid {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.id
    }
}

#[tokio::test]
async fn quote_id_and_submitted_at_are_stable_across_ccc_retry() {
    let app = spawn_app().await;
    let rfq_id = Uuid::parse_str(RFQ_ID).unwrap();
    let supplier_id = Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap();
    let fixed_quote_id = Uuid::from_u128(0xaaaaaaaa_bbbb_4ccc_8ddd_eeeeeeeeeeee);
    let fixed_time = chrono::DateTime::parse_from_rfc3339("2026-10-27T12:34:56Z")
        .unwrap()
        .with_timezone(&Utc);
    let clock_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let id_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clock = CountingFixedClock {
        instant: fixed_time,
        calls: Arc::clone(&clock_calls),
    };
    let ids = CountingFixedId {
        id: fixed_quote_id,
        calls: Arc::clone(&id_calls),
    };
    let injected = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let store = ConflictOnceStore {
        inner: Arc::clone(&app.store),
        supplier_id,
        injected: Arc::clone(&injected),
        synthetic_conflict: false,
    };
    let request: SubmitSupplierQuoteRequest =
        serde_json::from_value(valid_request(SUPPLIER_ELIGIBLE_ID)).unwrap();

    let commands = supplier_rfq_rpu_rust::app::CommandRpus::new(
        Arc::new(FactstrApplicationStateProvider::new(Arc::new(store))),
        Arc::new(clock),
        Arc::new(ids),
    );
    let quote =
        tokio::task::spawn_blocking(move || commands.submit_supplier_quote(rfq_id, request))
            .await
            .unwrap()
            .expect("the same logical submission should succeed after a harmless CCC conflict");

    assert!(
        injected.load(std::sync::atomic::Ordering::SeqCst),
        "the test must induce a CCC context change"
    );
    assert_eq!(quote.quote_id, fixed_quote_id);
    assert_eq!(quote.submitted_at, fixed_time);
    assert_eq!(clock_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(id_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn close_rfq_timestamp_is_stable_across_ccc_retry() {
    let app = spawn_app().await;
    let rfq_id = Uuid::parse_str(RFQ_ID).unwrap();
    let fixed_time = chrono::DateTime::parse_from_rfc3339("2026-10-27T12:34:56Z")
        .unwrap()
        .with_timezone(&Utc);
    let clock_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let clock = CountingFixedClock {
        instant: fixed_time,
        calls: Arc::clone(&clock_calls),
    };
    let ids = CountingFixedId {
        id: Uuid::from_u128(0xbbbbbbbb_cccc_4ddd_8eee_ffffffffffff),
        calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };
    let injected = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let store = ConflictOnceStore {
        inner: Arc::clone(&app.store),
        supplier_id: Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap(),
        injected: Arc::clone(&injected),
        synthetic_conflict: true,
    };
    let commands = supplier_rfq_rpu_rust::app::CommandRpus::new(
        Arc::new(FactstrApplicationStateProvider::new(Arc::new(store))),
        Arc::new(clock),
        Arc::new(ids),
    );

    let closed = tokio::task::spawn_blocking(move || {
        commands
            .close_rfq(supplier_rfq_rpu_rust::capabilities::close_rfq::CloseRfqRequest { rfq_id })
    })
    .await
    .unwrap()
    .expect("close should succeed after one synthetic CCC conflict");

    assert!(injected.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(closed.rfq_id, rfq_id);
    assert_eq!(closed.closed_at, fixed_time);
    assert_eq!(clock_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn http_idempotency_replays_the_original_success_response() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let key = format!("open-rfq-{rfq_id}");
    let body = json!({
        "id": rfq_id,
        "reference": format!("RFQ-IDEMPOTENCY-{rfq_id}"),
        "currency": "BRL",
        "items": [{
            "id": item_id,
            "sku": "SKU-IDEMPOTENCY",
            "description": "Idempotency integration test",
            "required_quantity": 1
        }]
    });
    let url = format!("{}/api/rfqs", app.base_url);

    let first = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_body = first.bytes().await.unwrap();

    let second = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CREATED);
    assert_eq!(
        second.headers().get("idempotency-replayed").unwrap(),
        "true"
    );
    assert_eq!(second.bytes().await.unwrap(), first_body);

    let query = EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([json!({ "rfq_id": rfq_id })])]);
    let result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        result.event_records.len(),
        1,
        "the repeated HTTP request must not append a second RFQ fact"
    );
}

#[tokio::test]
async fn http_idempotency_concurrent_same_key_appends_one_fact() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let key = format!("open-rfq-concurrent-{rfq_id}");
    let body = json!({ "id": rfq_id, "reference": format!("RFQ-IDEMPOTENCY-CONCURRENT-{rfq_id}"),
        "currency": "BRL", "items": [{ "id": item_id, "sku": "SKU-IDEMPOTENCY-CONCURRENT",
        "description": "Concurrent idempotency test", "required_quantity": 1 }] });
    let url = format!("{}/api/rfqs", app.base_url);
    let c1 = client.clone();
    let u1 = url.clone();
    let b1 = body.clone();
    let k1 = key.clone();
    let first = async move {
        c1.post(u1)
            .header("Idempotency-Key", k1)
            .json(&b1)
            .send()
            .await
            .unwrap()
    };
    let c2 = client.clone();
    let u2 = url.clone();
    let b2 = body.clone();
    let k2 = key.clone();
    let second = async move {
        c2.post(u2)
            .header("Idempotency-Key", k2)
            .json(&b2)
            .send()
            .await
            .unwrap()
    };
    let (one, two) = tokio::join!(first, second);
    let mut original = 0;
    let mut replayed = 0;
    let mut pending = 0;
    for response in [one, two] {
        if response.status() == StatusCode::CREATED {
            if response
                .headers()
                .get("idempotency-replayed")
                .is_some_and(|v| v.as_bytes() == b"true")
            {
                replayed += 1;
            } else {
                original += 1;
            }
        } else {
            assert_eq!(response.status(), StatusCode::CONFLICT);
            let error: Value = response.json().await.unwrap();
            assert_eq!(error["code"], "idempotency_in_progress");
            pending += 1;
        }
    }
    assert_eq!(original, 1, "one request must own the domain execution");
    assert_eq!(
        replayed + pending,
        1,
        "the second request is replayed or blocked in progress"
    );
    if pending == 1 {
        let mut eventually_replayed = false;
        for _ in 0..30 {
            let response = client
                .post(&url)
                .header("Idempotency-Key", &key)
                .json(&body)
                .send()
                .await
                .unwrap();
            if response.status() == StatusCode::CREATED {
                assert_eq!(
                    response.headers().get("idempotency-replayed").unwrap(),
                    "true"
                );
                eventually_replayed = true;
                break;
            }
            assert_eq!(response.status(), StatusCode::CONFLICT);
            let error: Value = response.json().await.unwrap();
            assert_eq!(error["code"], "idempotency_in_progress");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            eventually_replayed,
            "retry must eventually replay the completed result"
        );
    }
    let query = EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([json!({"rfq_id":rfq_id})])]);
    let result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        result.event_records.len(),
        1,
        "one logical request produces one fact"
    );
}

#[tokio::test]
async fn http_idempotency_store_unavailable_prevents_command_execution() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let key = format!("ledger-down-{rfq_id}");
    let body = json!({"id":rfq_id,"reference":format!("RFQ-LEDGER-DOWN-{rfq_id}"),"currency":"BRL",
        "items":[{"id":item_id,"sku":"SKU-LEDGER-DOWN","description":"Ledger availability test","required_quantity":1}]});
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&app.database_url)
        .await
        .unwrap();
    sqlx::query("DROP TABLE http_idempotency_keys CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    let response = client
        .post(format!("{}/api/rfqs", app.base_url))
        .header("Idempotency-Key", &key)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let error: Value = response.json().await.unwrap();
    assert_eq!(error["code"], "idempotency_store_unavailable");
    let query = EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([json!({"rfq_id":rfq_id})])]);
    let result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();
    assert!(
        result.event_records.is_empty(),
        "command must not run if the ledger cannot claim the key"
    );
    pool.close().await;
}

#[tokio::test]
async fn http_idempotency_failure_after_append_marks_unknown_without_duplicate_fact() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let key = format!("open-rfq-response-persist-failure-{rfq_id}");
    let body = json!({"id":rfq_id,"reference":format!("RFQ-IDEMPOTENCY-RESPONSE-FAILURE-{rfq_id}"),
        "currency":"BRL","items":[{"id":item_id,"sku":"SKU-IDEMPOTENCY-RESPONSE-FAILURE",
        "description":"Response ledger failure after domain append","required_quantity":1}]});
    let url = format!("{}/api/rfqs", app.base_url);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&app.database_url)
        .await
        .unwrap();
    sqlx::query("CREATE OR REPLACE FUNCTION fail_http_idempotency_completion() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.state = 'completed' THEN RAISE EXCEPTION 'injected completion failure after domain append'; END IF; RETURN NEW; END; $$")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER fail_http_idempotency_completion BEFORE UPDATE ON http_idempotency_keys FOR EACH ROW EXECUTE FUNCTION fail_http_idempotency_completion()")
        .execute(&pool).await.unwrap();
    let first = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        first.status(),
        StatusCode::CREATED,
        "domain execution already succeeded"
    );
    let _ = first.bytes().await.unwrap();
    let retry = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::CONFLICT);
    let error: Value = retry.json().await.unwrap();
    assert_eq!(error["code"], "idempotency_outcome_unknown");
    let query = EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([json!({"rfq_id":rfq_id})])]);
    let result = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        result.event_records.len(),
        1,
        "retry after ledger failure must not duplicate the domain fact"
    );
    sqlx::query("DROP TRIGGER fail_http_idempotency_completion ON http_idempotency_keys")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION fail_http_idempotency_completion()")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}

#[tokio::test]
async fn http_idempotency_replays_same_semantic_json_with_reordered_keys() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let reference = format!("RFQ-CANONICAL-{rfq_id}");
    let key = format!("canonical-json-{rfq_id}");
    let body_first = format!(
        r#"{{"id":"{rfq_id}","reference":"{reference}","currency":"BRL","items":[{{"id":"{item_id}","sku":"SKU-CANONICAL","description":"Canonical JSON property order","required_quantity":1}}]}}"#
    );
    let body_reordered = format!(
        r#"{{"items":[{{"required_quantity":1,"description":"Canonical JSON property order","sku":"SKU-CANONICAL","id":"{item_id}"}}],"currency":"BRL","reference":"{reference}","id":"{rfq_id}"}}"#
    );
    let url = format!("{}/api/rfqs", app.base_url);

    let first = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .header("content-type", "application/json")
        .body(body_first)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let content_type = first
        .headers()
        .get("content-type")
        .expect("content type must exist")
        .clone();
    let first_body = first.bytes().await.unwrap();

    let second = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .header("content-type", "application/json")
        .body(body_reordered)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CREATED);
    assert_eq!(
        second.headers().get("idempotency-replayed").unwrap(),
        "true"
    );
    assert_eq!(second.headers().get("content-type"), Some(&content_type));
    assert_eq!(second.bytes().await.unwrap(), first_body);

    let query = EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([json!({"rfq_id":rfq_id})])]);
    let persisted = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        persisted.event_records.len(),
        1,
        "canonical JSON replay must not append twice"
    );
}

#[tokio::test]
async fn http_idempotency_rejects_invalid_key_and_oversized_request_before_command() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let reference = format!("RFQ-INVALID-KEY-{rfq_id}");
    let key = format!("request-limits-{rfq_id}");
    let body = json!({ "id": rfq_id, "reference": reference, "currency": "BRL",
        "items": [{ "id": item_id, "sku": "SKU-LIMIT", "description": "Request limit test", "required_quantity": 1 }] });
    let url = format!("{}/api/rfqs", app.base_url);

    let invalid = client
        .post(&url)
        .header("Idempotency-Key", "bad key")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_error: Value = invalid.json().await.unwrap();
    assert_eq!(invalid_error["code"], "invalid_idempotency_key");

    let mut duplicate_request = client.post(&url).json(&body).build().unwrap();
    let duplicate_headers = duplicate_request.headers_mut();
    duplicate_headers.append(
        "Idempotency-Key",
        reqwest::header::HeaderValue::from_static("duplicate-one"),
    );
    duplicate_headers.append(
        "Idempotency-Key",
        reqwest::header::HeaderValue::from_static("duplicate-two"),
    );
    let duplicate = client.execute(duplicate_request).await.unwrap();
    assert_eq!(duplicate.status(), StatusCode::BAD_REQUEST);
    let duplicate_error: Value = duplicate.json().await.unwrap();
    assert_eq!(duplicate_error["code"], "invalid_idempotency_key");

    let padding = "x".repeat(2 * 1024 * 1024);
    let oversized = format!(
        r#"{{"id":"{rfq_id}","reference":"{reference}","currency":"BRL","items":[{{"id":"{item_id}","sku":"SKU-LIMIT","description":"Request limit test","required_quantity":1}}],"padding":"{padding}"}}"#
    );
    let too_large = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .header("content-type", "application/json")
        .body(oversized)
        .send()
        .await
        .unwrap();
    assert_eq!(too_large.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let size_error: Value = too_large.json().await.unwrap();
    assert_eq!(size_error["code"], "idempotency_request_too_large");

    let query = EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([json!({"rfq_id":rfq_id})])]);
    let persisted = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();
    assert!(
        persisted.event_records.is_empty(),
        "invalid key and oversized body must not run the command"
    );
}

#[tokio::test]
async fn http_idempotency_fingerprint_includes_path_query_and_content_type() {
    let app = spawn_app().await;
    let client = Client::new();
    let key = format!("quote-fingerprint-{}", Uuid::new_v4());
    let body = serde_json::to_string(&valid_request(SUPPLIER_ELIGIBLE_ID)).unwrap();
    let base = format!("{}/api/rfqs/{}/quotes", app.base_url, RFQ_ID);
    let original_url = format!("{base}?source=one");

    let first = client
        .post(&original_url)
        .header("Idempotency-Key", &key)
        .header("content-type", "application/json")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let _ = first.bytes().await.unwrap();

    let content_type_change = client
        .post(&original_url)
        .header("Idempotency-Key", &key)
        .header("content-type", "application/json; charset=utf-8")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(content_type_change.status(), StatusCode::CONFLICT);
    assert_eq!(
        content_type_change.json::<Value>().await.unwrap()["code"],
        "idempotency_key_reused"
    );

    let query_change = client
        .post(format!("{base}?source=two"))
        .header("Idempotency-Key", &key)
        .header("content-type", "application/json")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(query_change.status(), StatusCode::CONFLICT);
    assert_eq!(
        query_change.json::<Value>().await.unwrap()["code"],
        "idempotency_key_reused"
    );

    let path_change = format!(
        "{}/api/rfqs/{}/quotes?source=one",
        app.base_url,
        Uuid::new_v4()
    );
    let path_mismatch = client
        .post(path_change)
        .header("Idempotency-Key", &key)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(path_mismatch.status(), StatusCode::CONFLICT);
    assert_eq!(
        path_mismatch.json::<Value>().await.unwrap()["code"],
        "idempotency_key_reused"
    );

    let query = quote_context_query(
        Uuid::parse_str(RFQ_ID).unwrap(),
        Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap(),
    );
    let persisted = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        persisted.event_records.len(),
        1,
        "fingerprint mismatches must be rejected before a second append"
    );
}

#[tokio::test]
async fn http_idempotency_replays_final_business_4xx_response() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let key = format!("close-missing-rfq-{rfq_id}");
    let url = format!("{}/api/rfqs/{rfq_id}/close", app.base_url);

    let first = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::NOT_FOUND);
    let first_body = first.bytes().await.unwrap();

    let second = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        second.headers().get("idempotency-replayed").unwrap(),
        "true"
    );
    assert_eq!(
        second.bytes().await.unwrap(),
        first_body,
        "final business errors below 500 must be replayed as the original result"
    );
}

#[tokio::test]
async fn http_idempotency_rejects_same_key_with_different_request() {
    let app = spawn_app().await;
    let client = Client::new();
    let rfq_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();
    let key = format!("open-rfq-mismatch-{rfq_id}");
    let body = json!({
        "id": rfq_id,
        "reference": format!("RFQ-IDEMPOTENCY-MISMATCH-{rfq_id}"),
        "currency": "BRL",
        "items": [{
            "id": item_id,
            "sku": "SKU-IDEMPOTENCY-MISMATCH",
            "description": "Idempotency mismatch test",
            "required_quantity": 1
        }]
    });
    let url = format!("{}/api/rfqs", app.base_url);
    let first = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let _ = first.bytes().await.unwrap();

    let mut changed_body = body;
    changed_body["currency"] = json!("USD");
    let second = client
        .post(&url)
        .header("Idempotency-Key", &key)
        .json(&changed_body)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
    let error: Value = second.json().await.unwrap();
    assert_eq!(error["code"], "idempotency_key_reused");
}

// Additive API acceptance test; existing tests remain unchanged.
#[tokio::test]
async fn finalize_rfq_sourcing_endpoint_returns_the_atomic_business_result() {
    let app = spawn_app().await;
    let client = Client::new();
    let quote_body = json!({
        "supplier_id": SUPPLIER_ELIGIBLE_ID,
        "currency": "BRL",
        "delivery_days": 5,
        "valid_until": "2099-12-31",
        "items": [
            {"rfq_item_id": ITEM_A_ID, "quantity": 10, "unit_price_cents": 100},
            {"rfq_item_id": ITEM_B_ID, "quantity": 5, "unit_price_cents": 200}
        ]
    });
    let submitted = client
        .post(format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url))
        .json(&quote_body)
        .send()
        .await
        .expect("quote submission request should complete");
    assert_eq!(submitted.status(), StatusCode::CREATED);

    let quote_list: Value = client
        .get(format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url))
        .send()
        .await
        .expect("quote list request should complete")
        .error_for_status()
        .expect("quote list should return 200")
        .json()
        .await
        .expect("quote list response should be JSON");
    let quotes = quote_list["quotes"]
        .as_array()
        .expect("quotes array is required");
    let selected_quote = quotes
        .iter()
        .find(|quote| quote["supplier_id"] == SUPPLIER_ELIGIBLE_ID)
        .expect("submitted quote must appear in the quote view");
    let selected_quote_id = selected_quote["quote_id"]
        .as_str()
        .expect("quote_id must be a string")
        .to_owned();
    let quote_set_version = quotes
        .iter()
        .filter_map(|quote| quote["sequence_number"].as_u64())
        .max()
        .expect("at least one quote must have a sequence number");

    let response = client
        .post(format!(
            "{}/api/rfqs/{RFQ_ID}/finalize-sourcing",
            app.base_url
        ))
        .json(&json!({
            "supplier_id": SUPPLIER_ELIGIBLE_ID,
            "quote_id": selected_quote_id,
            "expected_quote_set_version": quote_set_version
        }))
        .send()
        .await
        .expect("sourcing finalization request should complete");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response
        .json()
        .await
        .expect("finalization response should be JSON");
    assert_eq!(body["closed"]["rfq_id"], RFQ_ID);
    assert_eq!(body["awarded"]["quote_id"], selected_quote_id);
    assert_eq!(body["purchase_order"]["source_quote_id"], selected_quote_id);
    assert_eq!(
        body["dispatch_requested"]["purchase_order_id"],
        body["purchase_order"]["purchase_order_id"]
    );

    let count = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || {
            let query = EventQuery::all().with_filters([
                EventFilter::for_event_types(["rfq_closed.v1"])
                    .with_payload_predicates([json!({"rfq_id": RFQ_ID})]),
                EventFilter::for_event_types(["rfq_awarded.v1"])
                    .with_payload_predicates([json!({"rfq_id": RFQ_ID})]),
                EventFilter::for_event_types(["purchase_order_created.v1"])
                    .with_payload_predicates([json!({"rfq_id": RFQ_ID})]),
                EventFilter::for_event_types(["purchase_order_dispatch_requested.v1"])
                    .with_payload_predicates([json!({"rfq_id": RFQ_ID})]),
            ]);
            store
                .as_ref()
                .query(&query)
                .expect("atomic sourcing facts should be queryable")
                .event_records
                .len()
        }
    })
    .await
    .expect("event count task should complete");
    assert_eq!(
        count, 4,
        "all four facts must appear together for one successful finalization"
    );

    // The existing quote RPU is not reused by the composite flow. Its unchanged CCC
    // context still observes the atomically appended rfq_closed.v1 when called afterward.
    let quote_after_close = client
        .post(format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url))
        .json(&quote_body)
        .send()
        .await
        .expect("quote request after close should complete");
    assert_eq!(quote_after_close.status(), StatusCode::CONFLICT);
    let error: Value = quote_after_close
        .json()
        .await
        .expect("conflict response should be JSON");
    assert_eq!(error["code"], "rfq_closed");
}
