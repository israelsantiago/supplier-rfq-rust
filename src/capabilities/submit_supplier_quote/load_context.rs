use factstr::{EventFilter, EventQuery, EventStore, EventStoreError};
use serde_json::json;
use uuid::Uuid;

use crate::events::{
    RFQ_CLOSED_V1, RFQ_OPENED_V1, RfqClosedFact, RfqOpenedFact, SUPPLIER_ELIGIBILITY_CHANGED_V1,
    SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
    SupplierQuoteSubmittedFact, SupplierRegisteredFact,
};

use super::build_context::SubmissionContextFact;

#[derive(Debug)]
pub(super) struct LoadedSubmissionContext {
    pub(super) query: EventQuery,
    pub(super) current_context_version: Option<u64>,
    pub(super) facts: Vec<SubmissionContextFact>,
}

pub(super) fn submission_context_query(rfq_id: Uuid, supplier_id: Uuid) -> EventQuery {
    EventQuery::all().with_filters([
        EventFilter::for_event_types([RFQ_OPENED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
        EventFilter::for_event_types([RFQ_CLOSED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
        EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
            .with_payload_predicates([json!({ "supplier_id": supplier_id })]),
        EventFilter::for_event_types([SUPPLIER_ELIGIBILITY_CHANGED_V1])
            .with_payload_predicates([json!({ "supplier_id": supplier_id })]),
        EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id, "supplier_id": supplier_id })]),
    ])
}

/// Shell de leitura: consulta FACTSTR e transforma os payloads em fatos tipados.
pub(super) fn load_context(
    store: &impl EventStore,
    rfq_id: Uuid,
    supplier_id: Uuid,
) -> Result<LoadedSubmissionContext, EventStoreError> {
    let query = submission_context_query(rfq_id, supplier_id);
    let result = store.query(&query)?;
    let mut facts = Vec::with_capacity(result.event_records.len());

    for record in result.event_records {
        match record.event_type.as_str() {
            RFQ_OPENED_V1 => {
                let fact: RfqOpenedFact =
                    serde_json::from_value(record.payload).map_err(|error| {
                        backend_error(format!("invalid {RFQ_OPENED_V1} fact: {error}"))
                    })?;
                facts.push(SubmissionContextFact::RfqOpened(fact));
            }
            RFQ_CLOSED_V1 => {
                let _fact: RfqClosedFact =
                    serde_json::from_value(record.payload).map_err(|error| {
                        backend_error(format!("invalid {RFQ_CLOSED_V1} fact: {error}"))
                    })?;
                facts.push(SubmissionContextFact::RfqClosed);
            }
            SUPPLIER_REGISTERED_V1 => {
                let _fact: SupplierRegisteredFact = serde_json::from_value(record.payload)
                    .map_err(|error| {
                        backend_error(format!("invalid {SUPPLIER_REGISTERED_V1} fact: {error}"))
                    })?;
                facts.push(SubmissionContextFact::SupplierRegistered);
            }
            SUPPLIER_ELIGIBILITY_CHANGED_V1 => {
                let fact: SupplierEligibilityChangedFact = serde_json::from_value(record.payload)
                    .map_err(|error| {
                    backend_error(format!(
                        "invalid {SUPPLIER_ELIGIBILITY_CHANGED_V1} fact: {error}"
                    ))
                })?;
                facts.push(SubmissionContextFact::SupplierEligibilityChanged(
                    fact.status,
                ));
            }
            SUPPLIER_QUOTE_SUBMITTED_V1 => {
                let _fact: SupplierQuoteSubmittedFact = serde_json::from_value(record.payload)
                    .map_err(|error| {
                        backend_error(format!(
                            "invalid {SUPPLIER_QUOTE_SUBMITTED_V1} fact: {error}"
                        ))
                    })?;
                facts.push(SubmissionContextFact::SupplierQuoteSubmitted);
            }
            _ => {}
        }
    }

    Ok(LoadedSubmissionContext {
        query,
        current_context_version: result.current_context_version,
        facts,
    })
}

fn backend_error(message: String) -> EventStoreError {
    EventStoreError::BackendFailure { message }
}
