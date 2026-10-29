//! FACTSTR/PostgreSQL adapter for the separate atomic sourcing state port.
//!
//! The legacy `FactstrApplicationStateProvider` and `ApplicationStateProvider` contract
//! are untouched. This adapter guarantees the same context query is reconstructed for read
//! and append_if, and serializes the entire consequence set in one FACTSTR batch.

use std::sync::Arc;

use factstr::{EventFilter, EventQuery, EventStore, EventStoreError, NewEvent};
use factstr_postgres::PostgresStore;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::{
    capabilities::finalize_rfq_sourcing::{
        AtomicSourcingStateProvider, FinalizeRfqSourcingRequest, SourcingContext,
        SourcingContextSnapshot, SourcingContextVersion, SourcingDecision, SourcingStateError,
    },
    events::{
        PURCHASE_ORDER_CREATED_V1, PURCHASE_ORDER_DISPATCH_REQUESTED_V1, PurchaseOrderCreatedFact,
        PurchaseOrderDispatchRequestedFact, RFQ_AWARDED_V1, RFQ_CLOSED_V1, RFQ_OPENED_V1,
        RfqAwardedFact, RfqClosedFact, RfqOpenedFact, SUPPLIER_ELIGIBILITY_CHANGED_V1,
        SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
        SupplierQuoteSubmittedFact, SupplierRegisteredFact, SupplierStatus,
    },
};

pub struct FactstrAtomicSourcingStateProvider {
    store: Arc<PostgresStore>,
}

impl FactstrAtomicSourcingStateProvider {
    pub fn new(store: Arc<PostgresStore>) -> Self {
        Self { store }
    }
}

fn sourcing_context_query(request: &FinalizeRfqSourcingRequest) -> EventQuery {
    // All filters are part of ONE EventQuery. FACTSTR validates this exact query version
    // atomically with append_if. Splitting this into multiple appends would break atomicity.
    EventQuery::all().with_filters([
        EventFilter::for_event_types([RFQ_OPENED_V1])
            .with_payload_predicates([json!({ "rfq_id": request.rfq_id })]),
        EventFilter::for_event_types([RFQ_CLOSED_V1])
            .with_payload_predicates([json!({ "rfq_id": request.rfq_id })]),
        EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1])
            .with_payload_predicates([json!({ "rfq_id": request.rfq_id })]),
        EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
            .with_payload_predicates([json!({ "supplier_id": request.supplier_id })]),
        EventFilter::for_event_types([SUPPLIER_ELIGIBILITY_CHANGED_V1])
            .with_payload_predicates([json!({ "supplier_id": request.supplier_id })]),
        EventFilter::for_event_types([RFQ_AWARDED_V1])
            .with_payload_predicates([json!({ "rfq_id": request.rfq_id })]),
        EventFilter::for_event_types([PURCHASE_ORDER_CREATED_V1])
            .with_payload_predicates([json!({ "rfq_id": request.rfq_id })]),
        EventFilter::for_event_types([PURCHASE_ORDER_DISPATCH_REQUESTED_V1])
            .with_payload_predicates([json!({ "rfq_id": request.rfq_id })]),
    ])
}

fn decode<T: DeserializeOwned>(event_type: &str, payload: Value) -> Result<T, SourcingStateError> {
    serde_json::from_value(payload).map_err(|error| {
        SourcingStateError::Store(format!("invalid {event_type} payload: {error}"))
    })
}

fn build_context(
    request: &FinalizeRfqSourcingRequest,
    records: Vec<factstr::EventRecord>,
) -> Result<SourcingContext, SourcingStateError> {
    let mut records = records;
    records.sort_by_key(|record| record.sequence_number);

    let mut context = SourcingContext::default();
    let mut latest_supplier_eligibility: Option<(u64, SupplierStatus)> = None;

    for record in records {
        let sequence = record.sequence_number;
        match record.event_type.as_str() {
            RFQ_OPENED_V1 => {
                let fact: RfqOpenedFact = decode(RFQ_OPENED_V1, record.payload)?;
                if fact.rfq_id == request.rfq_id {
                    context.rfq = Some(fact);
                }
            }
            RFQ_CLOSED_V1 => {
                let fact: RfqClosedFact = decode(RFQ_CLOSED_V1, record.payload)?;
                if fact.rfq_id == request.rfq_id {
                    context.closed = true;
                }
            }
            SUPPLIER_REGISTERED_V1 => {
                let fact: SupplierRegisteredFact = decode(SUPPLIER_REGISTERED_V1, record.payload)?;
                if fact.supplier_id == request.supplier_id {
                    context.supplier_registered = true;
                }
            }
            SUPPLIER_ELIGIBILITY_CHANGED_V1 => {
                let fact: SupplierEligibilityChangedFact =
                    decode(SUPPLIER_ELIGIBILITY_CHANGED_V1, record.payload)?;
                if fact.supplier_id == request.supplier_id
                    && latest_supplier_eligibility.is_none_or(|(previous, _)| sequence > previous)
                {
                    latest_supplier_eligibility = Some((sequence, fact.status));
                }
            }
            SUPPLIER_QUOTE_SUBMITTED_V1 => {
                let fact: SupplierQuoteSubmittedFact =
                    decode(SUPPLIER_QUOTE_SUBMITTED_V1, record.payload)?;
                if fact.rfq_id == request.rfq_id {
                    if context
                        .latest_quote_sequence_number
                        .is_none_or(|previous| sequence > previous)
                    {
                        context.latest_quote_sequence_number = Some(sequence);
                    }
                    if fact.supplier_id == request.supplier_id {
                        context.supplier_quote_count += 1;
                        if fact.quote_id == request.quote_id {
                            context.selected_quote = Some(fact);
                        }
                    }
                }
            }
            RFQ_AWARDED_V1 => {
                let fact: RfqAwardedFact = decode(RFQ_AWARDED_V1, record.payload)?;
                if fact.rfq_id == request.rfq_id {
                    context.finalized = true;
                }
            }
            PURCHASE_ORDER_CREATED_V1 => {
                let fact: PurchaseOrderCreatedFact =
                    decode(PURCHASE_ORDER_CREATED_V1, record.payload)?;
                if fact.rfq_id == request.rfq_id {
                    context.finalized = true;
                }
            }
            PURCHASE_ORDER_DISPATCH_REQUESTED_V1 => {
                let fact: PurchaseOrderDispatchRequestedFact =
                    decode(PURCHASE_ORDER_DISPATCH_REQUESTED_V1, record.payload)?;
                if fact.rfq_id == request.rfq_id {
                    context.finalized = true;
                }
            }
            _ => {}
        }
    }

    context.supplier_status = latest_supplier_eligibility.map(|(_, status)| status);
    Ok(context)
}

impl AtomicSourcingStateProvider for FactstrAtomicSourcingStateProvider {
    fn read_context(
        &self,
        request: &FinalizeRfqSourcingRequest,
    ) -> Result<SourcingContextSnapshot, SourcingStateError> {
        let query = sourcing_context_query(request);
        let snapshot = self
            .store
            .query(&query)
            .map_err(|error| SourcingStateError::Store(error.to_string()))?;
        let context = build_context(request, snapshot.event_records)?;
        Ok(SourcingContextSnapshot {
            context,
            expected_version: SourcingContextVersion::from_factstr(
                snapshot.current_context_version,
            ),
        })
    }

    fn append_if(
        &self,
        request: &FinalizeRfqSourcingRequest,
        expected_version: &SourcingContextVersion,
        decision: &SourcingDecision,
    ) -> Result<(), SourcingStateError> {
        let query = sourcing_context_query(request);
        let events = vec![
            NewEvent::new(RFQ_CLOSED_V1, json!(&decision.closed)),
            NewEvent::new(RFQ_AWARDED_V1, json!(&decision.awarded)),
            NewEvent::new(PURCHASE_ORDER_CREATED_V1, json!(&decision.purchase_order)),
            NewEvent::new(
                PURCHASE_ORDER_DISPATCH_REQUESTED_V1,
                json!(&decision.dispatch_requested),
            ),
        ];
        let expected_factstr_version = expected_version.to_factstr()?;
        self.store
            .append_if(events, &query, expected_factstr_version)
            .map(|_| ())
            .map_err(|error| match error {
                EventStoreError::ConditionalAppendConflict { .. } => {
                    SourcingStateError::ContextChanged
                }
                other => SourcingStateError::Store(other.to_string()),
            })
    }
}
