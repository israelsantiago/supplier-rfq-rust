use std::{net::SocketAddr, sync::Arc, time::Duration};

use chrono::{NaiveDate, Utc};
use factstr::{EventFilter, EventQuery, EventStore, NewEvent};
use factstr_postgres::PostgresStore;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use supplier_rfq_rpu_rust::{
    api_state::ApiState,
    app,
    events::{
        RFQ_OPENED_V1, RfqItemFact, RfqOpenedFact, SUPPLIER_ELIGIBILITY_CHANGED_V1,
        SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
        SupplierRegisteredFact, SupplierStatus,
    },
    projections::rfq_summary_projection::RfqSummaryProjection,
};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tokio::{net::TcpListener, sync::Barrier as TokioBarrier, task::JoinHandle};
use uuid::Uuid;

const RFQ_ID: &str = "11111111-1111-4111-8111-111111111111";
const SUPPLIER_ELIGIBLE_ID: &str = "22222222-2222-4222-8222-222222222222";
const SUPPLIER_BLOCKED_ID: &str = "33333333-3333-4333-8333-333333333333";
const ITEM_A_ID: &str = "44444444-4444-4444-8444-444444444441";
const ITEM_B_ID: &str = "44444444-4444-4444-8444-444444444442";

struct TestApp {
    base_url: String,
    store: Arc<PostgresStore>,
    _projection: Arc<RfqSummaryProjection>,
    server_task: JoinHandle<()>,
    // Keep PostgreSQL alive until the server and all FACTSTR handles are dropped.
    _container: ContainerAsync<Postgres>,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

async fn spawn_app() -> TestApp {
    let container = Postgres::default()
        .with_db_name("supplier_rfq")
        .with_user("rfq_user")
        .with_password("rfq_password")
        .with_tag("16-alpine")
        .start()
        .await
        .expect("PostgreSQL Testcontainer must start; Docker must be available");

    let host = container
        .get_host()
        .await
        .expect("container host must be available");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped PostgreSQL port must be available");
    let database_url = format!("postgres://rfq_user:rfq_password@{host}:{port}/supplier_rfq");
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

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral port must be available");
    let address: SocketAddr = listener
        .local_addr()
        .expect("listener address must be available");
    let api = app::router(ApiState::from_shared_event_store(
        Arc::clone(&store),
        Arc::clone(&projection),
    ));
    let server_task = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, api).await {
            panic!("test API server failed: {error}");
        }
    });

    TestApp {
        base_url: format!("http://{address}"),
        store,
        _projection: projection,
        server_task,
        _container: container,
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
