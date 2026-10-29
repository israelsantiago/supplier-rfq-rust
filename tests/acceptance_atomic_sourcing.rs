//! Acceptance tests for the new atomic sourcing flow. Existing per-RPU test suites stay intact.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use factstr::{EventFilter, EventQuery, EventStore, NewEvent};
use factstr_postgres::PostgresStore;
use serde_json::json;
use supplier_rfq_rpu_rust::{
    app::atomic_sourcing_flow::AtomicSourcingFlow,
    application_state::atomic_sourcing_factstr::FactstrAtomicSourcingStateProvider,
    capabilities::finalize_rfq_sourcing::{
        FinalizeRfqSourcingError, FinalizeRfqSourcingRequest, SourcingRejection,
        test_support::{commit_prepared, prepare},
    },
    events::{
        PURCHASE_ORDER_CREATED_V1, PURCHASE_ORDER_DISPATCH_REQUESTED_V1, QuotedItemFact,
        RFQ_AWARDED_V1, RFQ_CLOSED_V1, RFQ_OPENED_V1, RfqItemFact, RfqOpenedFact,
        SUPPLIER_ELIGIBILITY_CHANGED_V1, SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1,
        SupplierEligibilityChangedFact, SupplierQuoteSubmittedFact, SupplierRegisteredFact,
        SupplierStatus,
    },
    providers::{ClockProvider, IdProvider},
};
#[path = "common/postgres.rs"]
mod test_postgres;
use uuid::Uuid;

const RFQ_ID: u128 = 1;
const SUPPLIER_ID: u128 = 2;
const ITEM_ID: u128 = 3;
const QUOTE_ID: u128 = 4;

#[derive(Clone)]
struct FixedClock(DateTime<Utc>);
impl ClockProvider for FixedClock {
    fn now_utc(&self) -> DateTime<Utc> {
        self.0.to_owned()
    }
}

struct FixedIds(Mutex<VecDeque<Uuid>>);
impl FixedIds {
    fn new() -> Self {
        Self(Mutex::new(VecDeque::from([
            Uuid::from_u128(10),
            Uuid::from_u128(11),
            Uuid::from_u128(12),
        ])))
    }
}
impl IdProvider for FixedIds {
    fn new_uuid(&self) -> Uuid {
        self.0
            .lock()
            .expect("ID queue mutex should not be poisoned")
            .pop_front()
            .expect("the test flow requests exactly three IDs")
    }
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap()
}

fn request(expected_quote_set_version: u64) -> FinalizeRfqSourcingRequest {
    FinalizeRfqSourcingRequest {
        rfq_id: Uuid::from_u128(RFQ_ID),
        supplier_id: Uuid::from_u128(SUPPLIER_ID),
        quote_id: Uuid::from_u128(QUOTE_ID),
        expected_quote_set_version,
    }
}

async fn new_store() -> (test_postgres::TestDatabase, Arc<PostgresStore>) {
    let test_database = test_postgres::create_isolated_database().await;
    let database_url = test_database.database_url.clone();
    let store = tokio::task::spawn_blocking(move || PostgresStore::connect(&database_url))
        .await
        .expect("FACTSTR connect task must finish")
        .expect("FACTSTR must initialize its schema in the isolated test database");
    (test_database, Arc::new(store))
}

async fn seed(store: Arc<PostgresStore>, status: SupplierStatus) -> u64 {
    let rfq = RfqOpenedFact {
        rfq_id: Uuid::from_u128(RFQ_ID),
        reference: "RFQ-ATOMIC-001".to_owned(),
        currency: "BRL".to_owned(),
        opened_at: Utc.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).unwrap(),
        items: vec![RfqItemFact {
            id: Uuid::from_u128(ITEM_ID),
            sku: "SKU-1".to_owned(),
            description: "Industrial bearing".to_owned(),
            required_quantity: 2,
        }],
    };
    let supplier = SupplierRegisteredFact {
        supplier_id: Uuid::from_u128(SUPPLIER_ID),
        reference: "SUP-1".to_owned(),
        name: "Supplier One".to_owned(),
        registered_at: Utc.with_ymd_and_hms(2026, 10, 1, 9, 0, 0).unwrap(),
    };
    let eligibility = SupplierEligibilityChangedFact {
        supplier_id: Uuid::from_u128(SUPPLIER_ID),
        status,
        changed_at: Utc.with_ymd_and_hms(2026, 10, 2, 9, 0, 0).unwrap(),
    };
    let quote = SupplierQuoteSubmittedFact {
        quote_id: Uuid::from_u128(QUOTE_ID),
        rfq_id: Uuid::from_u128(RFQ_ID),
        supplier_id: Uuid::from_u128(SUPPLIER_ID),
        currency: "BRL".to_owned(),
        total_amount_cents: 500,
        delivery_days: 5,
        valid_until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        submitted_at: Utc.with_ymd_and_hms(2026, 10, 3, 9, 0, 0).unwrap(),
        items: vec![QuotedItemFact {
            rfq_item_id: Uuid::from_u128(ITEM_ID),
            quantity: 2,
            unit_price_cents: 250,
            line_total_cents: 500,
        }],
    };
    tokio::task::spawn_blocking(move || {
        store
            .as_ref()
            .append(vec![
                NewEvent::new(RFQ_OPENED_V1, json!(rfq)),
                NewEvent::new(SUPPLIER_REGISTERED_V1, json!(supplier)),
                NewEvent::new(SUPPLIER_ELIGIBILITY_CHANGED_V1, json!(eligibility)),
                NewEvent::new(SUPPLIER_QUOTE_SUBMITTED_V1, json!(quote)),
            ])
            .expect("seed facts should append atomically");
        let query = EventQuery::all()
            .with_filters([EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1])
                .with_payload_predicates([json!({"rfq_id": Uuid::from_u128(RFQ_ID)})])]);
        store
            .as_ref()
            .query(&query)
            .expect("quote set should be queryable")
            .event_records
            .into_iter()
            .map(|record| record.sequence_number)
            .max()
            .expect("one quote must be present")
    })
    .await
    .expect("seed/query task must finish")
}

async fn finalization_event_types(store: Arc<PostgresStore>) -> Vec<String> {
    tokio::task::spawn_blocking(move || {
        let query = EventQuery::all().with_filters([
            EventFilter::for_event_types([RFQ_CLOSED_V1])
                .with_payload_predicates([json!({"rfq_id": Uuid::from_u128(RFQ_ID)})]),
            EventFilter::for_event_types([RFQ_AWARDED_V1])
                .with_payload_predicates([json!({"rfq_id": Uuid::from_u128(RFQ_ID)})]),
            EventFilter::for_event_types([PURCHASE_ORDER_CREATED_V1])
                .with_payload_predicates([json!({"rfq_id": Uuid::from_u128(RFQ_ID)})]),
            EventFilter::for_event_types([PURCHASE_ORDER_DISPATCH_REQUESTED_V1])
                .with_payload_predicates([json!({"rfq_id": Uuid::from_u128(RFQ_ID)})]),
        ]);
        let mut records = store
            .as_ref()
            .query(&query)
            .expect("finalization facts should be queryable")
            .event_records;
        records.sort_by_key(|record| record.sequence_number);
        records
            .into_iter()
            .map(|record| record.event_type)
            .collect()
    })
    .await
    .expect("query task must finish")
}

fn expected_finalization_event_types() -> Vec<String> {
    vec![
        RFQ_CLOSED_V1.to_owned(),
        RFQ_AWARDED_V1.to_owned(),
        PURCHASE_ORDER_CREATED_V1.to_owned(),
        PURCHASE_ORDER_DISPATCH_REQUESTED_V1.to_owned(),
    ]
}

fn flow(store: Arc<PostgresStore>) -> AtomicSourcingFlow {
    let state = Arc::new(FactstrAtomicSourcingStateProvider::new(store));
    AtomicSourcingFlow::new(
        state,
        Arc::new(FixedClock(now())),
        Arc::new(FixedIds::new()),
    )
}

#[tokio::test]
async fn successful_finalization_persists_the_four_consequences_as_one_batch() {
    let (_database, store) = new_store().await;
    let quote_set_version = seed(Arc::clone(&store), SupplierStatus::Eligible).await;
    let decision = tokio::task::spawn_blocking({
        let service = flow(Arc::clone(&store));
        move || service.finalize_rfq_sourcing(request(quote_set_version))
    })
    .await
    .unwrap()
    .expect("all sourcing checks should pass");

    assert_eq!(decision.closed.rfq_id, Uuid::from_u128(RFQ_ID));
    assert_eq!(decision.awarded.quote_id, Uuid::from_u128(QUOTE_ID));
    assert_eq!(
        decision.purchase_order.source_quote_id,
        Uuid::from_u128(QUOTE_ID)
    );
    assert_eq!(
        decision.dispatch_requested.purchase_order_id,
        decision.purchase_order.purchase_order_id
    );
    assert_eq!(
        finalization_event_types(Arc::clone(&store)).await,
        expected_finalization_event_types()
    );
}

#[tokio::test]
async fn rejected_business_decision_writes_none_of_the_four_finalization_facts() {
    let (_database, store) = new_store().await;
    let version = seed(Arc::clone(&store), SupplierStatus::Blocked).await;
    let result = tokio::task::spawn_blocking({
        let service = flow(Arc::clone(&store));
        move || service.finalize_rfq_sourcing(request(version))
    })
    .await
    .unwrap();
    assert!(matches!(
        result,
        Err(FinalizeRfqSourcingError::Rejected(
            SourcingRejection::SupplierNotEligible
        ))
    ));
    assert!(
        finalization_event_types(Arc::clone(&store))
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn buyer_selection_is_rejected_when_quote_set_token_is_stale() {
    let (_database, store) = new_store().await;
    let version = seed(Arc::clone(&store), SupplierStatus::Eligible).await;
    let result = tokio::task::spawn_blocking({
        let service = flow(Arc::clone(&store));
        move || service.finalize_rfq_sourcing(request(version.saturating_sub(1)))
    })
    .await
    .unwrap();
    assert!(matches!(
        result,
        Err(FinalizeRfqSourcingError::Rejected(
            SourcingRejection::QuoteSetChanged
        ))
    ));
    assert!(
        finalization_event_types(Arc::clone(&store))
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn ccc_conflict_after_prepare_leaves_no_partial_finalization() {
    let (_database, store) = new_store().await;
    let version = seed(Arc::clone(&store), SupplierStatus::Eligible).await;
    let prepared = {
        let provider = FactstrAtomicSourcingStateProvider::new(Arc::clone(&store));
        tokio::task::spawn_blocking(move || {
            prepare(
                &provider,
                request(version),
                now(),
                Uuid::from_u128(10),
                Uuid::from_u128(11),
                Uuid::from_u128(12),
            )
        })
        .await
        .unwrap()
        .expect("preparation should succeed without writing")
    };
    // The legacy eligibility RPU is deliberately not called; we append a competing fact to
    // simulate a concurrent, valid state change after our read and before atomic commit.
    let store_to_update = Arc::clone(&store);
    tokio::task::spawn_blocking(move || {
        store_to_update
            .as_ref()
            .append(vec![NewEvent::new(
                SUPPLIER_ELIGIBILITY_CHANGED_V1,
                json!(SupplierEligibilityChangedFact {
                    supplier_id: Uuid::from_u128(SUPPLIER_ID),
                    status: SupplierStatus::Blocked,
                    changed_at: Utc.with_ymd_and_hms(2026, 10, 29, 12, 1, 0).unwrap(),
                }),
            )])
            .expect("concurrent state change should append");
    })
    .await
    .unwrap();

    let result = {
        let provider = FactstrAtomicSourcingStateProvider::new(Arc::clone(&store));
        tokio::task::spawn_blocking(move || commit_prepared(&provider, prepared))
            .await
            .unwrap()
    };
    assert!(matches!(
        result,
        Err(FinalizeRfqSourcingError::Rejected(
            SourcingRejection::CommandContextChanged
        ))
    ));
    assert!(
        finalization_event_types(Arc::clone(&store))
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn concurrent_new_quote_after_prepare_conflicts_without_partial_finalization() {
    let (_database, store) = new_store().await;
    let version = seed(Arc::clone(&store), SupplierStatus::Eligible).await;
    let prepared = {
        let provider = FactstrAtomicSourcingStateProvider::new(Arc::clone(&store));
        tokio::task::spawn_blocking(move || {
            prepare(
                &provider,
                request(version),
                now(),
                Uuid::from_u128(30),
                Uuid::from_u128(31),
                Uuid::from_u128(32),
            )
        })
        .await
        .unwrap()
        .expect("preparation should succeed without writing")
    };

    // A competing supplier submits a quote for the same RFQ after buyer review.
    let competing_quote = SupplierQuoteSubmittedFact {
        quote_id: Uuid::from_u128(41),
        rfq_id: Uuid::from_u128(RFQ_ID),
        supplier_id: Uuid::from_u128(42),
        currency: "BRL".to_owned(),
        total_amount_cents: 600,
        delivery_days: 7,
        valid_until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        submitted_at: Utc.with_ymd_and_hms(2026, 10, 29, 12, 1, 0).unwrap(),
        items: vec![QuotedItemFact {
            rfq_item_id: Uuid::from_u128(ITEM_ID),
            quantity: 2,
            unit_price_cents: 300,
            line_total_cents: 600,
        }],
    };
    let store_to_update = Arc::clone(&store);
    tokio::task::spawn_blocking(move || {
        store_to_update
            .as_ref()
            .append(vec![NewEvent::new(
                SUPPLIER_QUOTE_SUBMITTED_V1,
                json!(competing_quote),
            )])
            .expect("competing quote should be appended");
    })
    .await
    .unwrap();

    let result = {
        let provider = FactstrAtomicSourcingStateProvider::new(Arc::clone(&store));
        tokio::task::spawn_blocking(move || commit_prepared(&provider, prepared))
            .await
            .unwrap()
    };
    assert!(matches!(
        result,
        Err(FinalizeRfqSourcingError::Rejected(
            SourcingRejection::CommandContextChanged
        ))
    ));
    assert!(
        finalization_event_types(Arc::clone(&store))
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn second_commit_of_same_prepared_context_conflicts_without_duplicate_batch() {
    let (_database, store) = new_store().await;
    let version = seed(Arc::clone(&store), SupplierStatus::Eligible).await;
    let prepare_one = {
        let provider = FactstrAtomicSourcingStateProvider::new(Arc::clone(&store));
        tokio::task::spawn_blocking(move || {
            prepare(
                &provider,
                request(version),
                now(),
                Uuid::from_u128(10),
                Uuid::from_u128(11),
                Uuid::from_u128(12),
            )
        })
        .await
        .unwrap()
        .unwrap()
    };
    let prepare_two = {
        let provider = FactstrAtomicSourcingStateProvider::new(Arc::clone(&store));
        tokio::task::spawn_blocking(move || {
            prepare(
                &provider,
                request(version),
                now(),
                Uuid::from_u128(20),
                Uuid::from_u128(21),
                Uuid::from_u128(22),
            )
        })
        .await
        .unwrap()
        .unwrap()
    };
    let first = {
        let provider = FactstrAtomicSourcingStateProvider::new(Arc::clone(&store));
        tokio::task::spawn_blocking(move || commit_prepared(&provider, prepare_one))
            .await
            .unwrap()
    };
    assert!(first.is_ok());
    let second = {
        let provider = FactstrAtomicSourcingStateProvider::new(Arc::clone(&store));
        tokio::task::spawn_blocking(move || commit_prepared(&provider, prepare_two))
            .await
            .unwrap()
    };
    assert!(matches!(
        second,
        Err(FinalizeRfqSourcingError::Rejected(
            SourcingRejection::CommandContextChanged
        ))
    ));
    assert_eq!(
        finalization_event_types(Arc::clone(&store)).await,
        expected_finalization_event_types()
    );
}
