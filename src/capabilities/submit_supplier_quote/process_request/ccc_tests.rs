//! White-box CCC integration tests owned by SubmitSupplierQuote.
//! They use real PostgreSQL/Testcontainers but do not start the HTTP server or projection.

use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use factstr::{EventFilter, EventQuery, EventStore, NewEvent};
use factstr_postgres::PostgresStore;
use serde_json::{Value, json};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use uuid::Uuid;

use super::{CommitError, SubmitSupplierQuoteRequest, commit_prepared, prepare};
use crate::{
    capabilities::set_supplier_eligibility::SetSupplierEligibilityRequest,
    events::{
        RFQ_OPENED_V1, RfqItemFact, RfqOpenedFact, SUPPLIER_ELIGIBILITY_CHANGED_V1,
        SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
        SupplierRegisteredFact, SupplierStatus,
    },
};

const RFQ_ID: &str = "11111111-1111-4111-8111-111111111111";
const SUPPLIER_ELIGIBLE_ID: &str = "22222222-2222-4222-8222-222222222222";
const SUPPLIER_BLOCKED_ID: &str = "33333333-3333-4333-8333-333333333333";
const SUPPLIER_OTHER_ELIGIBLE_ID: &str = "55555555-5555-4555-8555-555555555555";
const ITEM_A_ID: &str = "44444444-4444-4444-8444-444444444441";
const ITEM_B_ID: &str = "44444444-4444-4444-8444-444444444442";

struct TestDatabase {
    database_url: String,
    store: Arc<PostgresStore>,
    // Keep PostgreSQL alive while this fixture and its independent stores are used.
    _container: ContainerAsync<Postgres>,
}

async fn spawn_test_database() -> TestDatabase {
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

    TestDatabase {
        database_url,
        store,
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

/// CCC proof: two independent FACTSTR/PostgreSQL stores each read and prepare
/// the same command context before a barrier releases both conditional appends.
/// The writes carry the same expected version, so exactly one can commit.
#[tokio::test]
async fn ccc_two_independent_stores_from_same_snapshot_commit_exactly_one_quote_fact() {
    let app = spawn_test_database().await;
    let database_url_1 = app.database_url.clone();
    let database_url_2 = app.database_url.clone();
    let store_1 = Arc::new(
        tokio::task::spawn_blocking(move || PostgresStore::connect(&database_url_1))
            .await
            .unwrap()
            .expect("first independent FACTSTR store must connect"),
    );
    let store_2 = Arc::new(
        tokio::task::spawn_blocking(move || PostgresStore::connect(&database_url_2))
            .await
            .unwrap()
            .expect("second independent FACTSTR store must connect"),
    );

    let rfq_id = Uuid::parse_str(RFQ_ID).unwrap();
    let supplier_id = Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap();
    let request_1: SubmitSupplierQuoteRequest =
        serde_json::from_value(valid_request(SUPPLIER_ELIGIBLE_ID)).unwrap();
    let request_2 = request_1.clone();
    let store_1_for_prepare = Arc::clone(&store_1);
    let prepared_1 = tokio::task::spawn_blocking(move || {
        prepare(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                store_1_for_prepare.as_ref(),
            ),
            rfq_id,
            request_1,
        )
    })
    .await
    .unwrap()
    .expect("first command should be valid");
    let store_2_for_prepare = Arc::clone(&store_2);
    let prepared_2 = tokio::task::spawn_blocking(move || {
        prepare(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                store_2_for_prepare.as_ref(),
            ),
            rfq_id,
            request_2,
        )
    })
    .await
    .unwrap()
    .expect("second command should be valid against the same snapshot");

    assert_eq!(
        prepared_1.context.current_context_version, prepared_2.context.current_context_version,
        "both decisions must be prepared against the same context version before either append starts"
    );

    let barrier = Arc::new(std::sync::Barrier::new(2));
    let first_barrier = Arc::clone(&barrier);
    let first_store = Arc::clone(&store_1);
    let first = tokio::task::spawn_blocking(move || {
        first_barrier.wait();
        commit_prepared(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                first_store.as_ref(),
            ),
            prepared_1,
        )
    });
    let second_barrier = Arc::clone(&barrier);
    let second_store = Arc::clone(&store_2);
    let second = tokio::task::spawn_blocking(move || {
        second_barrier.wait();
        commit_prepared(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                second_store.as_ref(),
            ),
            prepared_2,
        )
    });
    let (result_1, result_2) = tokio::join!(first, second);
    let results = [result_1.unwrap(), result_2.unwrap()];
    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        1,
        "one append must win"
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(CommitError::ContextChanged)))
            .count(),
        1,
        "the stale append must be rejected as FACTSTR ConditionalAppendConflict"
    );

    let quote_query = quote_context_query(rfq_id, supplier_id);
    let persisted = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&quote_query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        persisted.event_records.len(),
        1,
        "exactly one quote fact must be persisted"
    );
}

/// A supplier eligibility fact appended after a quote decision invalidates that
/// decision's command context. This covers a business fact, not just duplicates.
#[tokio::test]
async fn ccc_supplier_eligibility_change_invalidates_a_prepared_quote() {
    let app = spawn_test_database().await;
    let database_url_1 = app.database_url.clone();
    let database_url_2 = app.database_url.clone();
    let store_1 = Arc::new(
        tokio::task::spawn_blocking(move || PostgresStore::connect(&database_url_1))
            .await
            .unwrap()
            .expect("first independent FACTSTR store must connect"),
    );
    let store_2 = Arc::new(
        tokio::task::spawn_blocking(move || PostgresStore::connect(&database_url_2))
            .await
            .unwrap()
            .expect("second independent FACTSTR store must connect"),
    );

    let rfq_id = Uuid::parse_str(RFQ_ID).unwrap();
    let supplier_id = Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap();
    let request: SubmitSupplierQuoteRequest =
        serde_json::from_value(valid_request(SUPPLIER_ELIGIBLE_ID)).unwrap();
    let store_for_prepare = Arc::clone(&store_1);
    let prepared = tokio::task::spawn_blocking(move || {
        prepare(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                store_for_prepare.as_ref(),
            ),
            rfq_id,
            request,
        )
    })
    .await
    .unwrap()
    .expect("quote should be valid while supplier is eligible");
    let prepared_version = prepared.context.current_context_version.clone();

    let store_for_change = Arc::clone(&store_2);
    let commands = crate::app::CommandRpus::new(
        Arc::new(
            crate::application_state::factstr::FactstrApplicationStateProvider::new(
                store_for_change,
            ),
        ),
        Arc::new(crate::providers::SystemClockProvider),
        Arc::new(crate::providers::UuidV4Provider),
    );
    tokio::task::spawn_blocking(move || {
        commands.set_supplier_eligibility(
            supplier_id,
            SetSupplierEligibilityRequest {
                status: SupplierStatus::Blocked,
            },
        )
    })
    .await
    .unwrap()
    .expect("blocking a registered supplier must append an eligibility fact");

    let store_for_commit = Arc::clone(&store_1);
    let commit = tokio::task::spawn_blocking(move || {
        commit_prepared(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                store_for_commit.as_ref(),
            ),
            prepared,
        )
    })
    .await
    .unwrap();
    assert!(matches!(commit, Err(CommitError::ContextChanged)));
    assert!(
        prepared_version.is_present(),
        "the command must have observed supplier facts"
    );

    let quote_query = quote_context_query(rfq_id, supplier_id);
    let persisted = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&quote_query)
    })
    .await
    .unwrap()
    .unwrap();
    assert!(
        persisted.event_records.is_empty(),
        "a stale eligibility decision must not append a quote fact"
    );
}

/// Different supplier-specific contexts can both commit for one RFQ.
#[tokio::test]
async fn ccc_independent_supplier_contexts_commit_without_false_conflict() {
    let app = spawn_test_database().await;
    let other_supplier_id = Uuid::parse_str(SUPPLIER_OTHER_ELIGIBLE_ID).unwrap();
    let other_supplier = SupplierRegisteredFact {
        supplier_id: other_supplier_id,
        reference: "SUP-OTHER-ELIGIBLE".to_owned(),
        name: "Other Eligible Supplier".to_owned(),
        registered_at: Utc::now(),
    };
    let other_eligibility = SupplierEligibilityChangedFact {
        supplier_id: other_supplier_id,
        status: SupplierStatus::Eligible,
        changed_at: Utc::now(),
    };
    tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || {
            store
                .as_ref()
                .append(vec![
                    NewEvent::new(SUPPLIER_REGISTERED_V1, json!(other_supplier)),
                    NewEvent::new(SUPPLIER_ELIGIBILITY_CHANGED_V1, json!(other_eligibility)),
                ])
                .expect("second supplier facts must append");
        }
    })
    .await
    .unwrap();

    let store_1_url = app.database_url.clone();
    let store_2_url = app.database_url.clone();
    let store_1 = Arc::new(
        tokio::task::spawn_blocking(move || PostgresStore::connect(&store_1_url))
            .await
            .unwrap()
            .expect("first independent FACTSTR store must connect"),
    );
    let store_2 = Arc::new(
        tokio::task::spawn_blocking(move || PostgresStore::connect(&store_2_url))
            .await
            .unwrap()
            .expect("second independent FACTSTR store must connect"),
    );

    let rfq_id = Uuid::parse_str(RFQ_ID).unwrap();
    let request_1: SubmitSupplierQuoteRequest =
        serde_json::from_value(valid_request(SUPPLIER_ELIGIBLE_ID)).unwrap();
    let request_2: SubmitSupplierQuoteRequest =
        serde_json::from_value(valid_request(SUPPLIER_OTHER_ELIGIBLE_ID)).unwrap();
    let store_1_for_prepare = Arc::clone(&store_1);
    let prepared_1 = tokio::task::spawn_blocking(move || {
        prepare(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                store_1_for_prepare.as_ref(),
            ),
            rfq_id,
            request_1,
        )
    })
    .await
    .unwrap()
    .expect("first supplier quote should be valid");
    let store_2_for_prepare = Arc::clone(&store_2);
    let prepared_2 = tokio::task::spawn_blocking(move || {
        prepare(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                store_2_for_prepare.as_ref(),
            ),
            rfq_id,
            request_2,
        )
    })
    .await
    .unwrap()
    .expect("second supplier quote should be valid");

    let barrier = Arc::new(std::sync::Barrier::new(2));
    let first_barrier = Arc::clone(&barrier);
    let first_store = Arc::clone(&store_1);
    let first = tokio::task::spawn_blocking(move || {
        first_barrier.wait();
        commit_prepared(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                first_store.as_ref(),
            ),
            prepared_1,
        )
    });
    let second_barrier = Arc::clone(&barrier);
    let second_store = Arc::clone(&store_2);
    let second = tokio::task::spawn_blocking(move || {
        second_barrier.wait();
        commit_prepared(
            &crate::application_state::factstr::FactstrApplicationStateProvider::new(
                second_store.as_ref(),
            ),
            prepared_2,
        )
    });
    let (result_1, result_2) = tokio::join!(first, second);
    assert!(
        result_1.unwrap().is_ok(),
        "first supplier context should commit"
    );
    assert!(
        result_2.unwrap().is_ok(),
        "second supplier context should remain valid"
    );

    let quote_query = EventQuery::all()
        .with_filters([EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })])]);
    let persisted = tokio::task::spawn_blocking({
        let store = Arc::clone(&app.store);
        move || store.as_ref().query(&quote_query)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        persisted.event_records.len(),
        2,
        "both supplier quote facts must persist"
    );
}
