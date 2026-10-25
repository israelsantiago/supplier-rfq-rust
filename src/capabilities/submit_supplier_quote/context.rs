use factstr::{EventFilter, EventQuery, EventStore, EventStoreError, QueryResult};
use serde_json::json;
use uuid::Uuid;

use crate::events::{
    RFQ_CLOSED_V1, RFQ_OPENED_V1, RfqClosedFact, RfqOpenedFact, SUPPLIER_ELIGIBILITY_CHANGED_V1,
    SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
    SupplierQuoteSubmittedFact, SupplierRegisteredFact, SupplierStatus,
};

#[derive(Debug, Clone)]
pub struct RfqSubmissionContext {
    pub rfq: Option<RfqOpenedFact>,
    pub closed: bool,
    pub supplier_registered: bool,
    pub supplier_status: Option<SupplierStatus>,
    pub already_submitted: bool,
}

#[derive(Debug)]
pub struct LoadedSubmissionContext {
    pub query: EventQuery,
    pub current_context_version: Option<u64>,
    pub context: RfqSubmissionContext,
}

pub fn submission_context_query(rfq_id: Uuid, supplier_id: Uuid) -> EventQuery {
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

pub fn load_context(
    store: &impl EventStore,
    rfq_id: Uuid,
    supplier_id: Uuid,
) -> Result<LoadedSubmissionContext, EventStoreError> {
    let query = submission_context_query(rfq_id, supplier_id);
    let result = store.query(&query)?;
    let context = build_context(&result)?;
    Ok(LoadedSubmissionContext {
        query,
        current_context_version: result.current_context_version,
        context,
    })
}

fn build_context(result: &QueryResult) -> Result<RfqSubmissionContext, EventStoreError> {
    let mut context = RfqSubmissionContext {
        rfq: None,
        closed: false,
        supplier_registered: false,
        supplier_status: None,
        already_submitted: false,
    };

    for record in &result.event_records {
        match record.event_type.as_str() {
            RFQ_OPENED_V1 => {
                context.rfq = Some(
                    serde_json::from_value::<RfqOpenedFact>(record.payload.clone()).map_err(
                        |error| backend_error(format!("invalid {RFQ_OPENED_V1} fact: {error}")),
                    )?,
                );
            }
            RFQ_CLOSED_V1 => {
                let _fact: RfqClosedFact =
                    serde_json::from_value(record.payload.clone()).map_err(|error| {
                        backend_error(format!("invalid {RFQ_CLOSED_V1} fact: {error}"))
                    })?;
                context.closed = true;
            }
            SUPPLIER_REGISTERED_V1 => {
                let _fact: SupplierRegisteredFact = serde_json::from_value(record.payload.clone())
                    .map_err(|error| {
                        backend_error(format!("invalid {SUPPLIER_REGISTERED_V1} fact: {error}"))
                    })?;
                context.supplier_registered = true;
            }
            SUPPLIER_ELIGIBILITY_CHANGED_V1 => {
                let fact: SupplierEligibilityChangedFact =
                    serde_json::from_value(record.payload.clone()).map_err(|error| {
                        backend_error(format!(
                            "invalid {SUPPLIER_ELIGIBILITY_CHANGED_V1} fact: {error}"
                        ))
                    })?;
                context.supplier_status = Some(fact.status);
            }
            SUPPLIER_QUOTE_SUBMITTED_V1 => {
                let _fact: SupplierQuoteSubmittedFact =
                    serde_json::from_value(record.payload.clone()).map_err(|error| {
                        backend_error(format!(
                            "invalid {SUPPLIER_QUOTE_SUBMITTED_V1} fact: {error}"
                        ))
                    })?;
                context.already_submitted = true;
            }
            _ => {}
        }
    }

    Ok(context)
}

fn backend_error(message: String) -> EventStoreError {
    EventStoreError::BackendFailure { message }
}
