use factstr::{EventFilter, EventQuery, EventStore};
use serde_json::json;
use uuid::Uuid;

use crate::events::{RFQ_CLOSED_V1, RFQ_OPENED_V1, RfqClosedFact, RfqOpenedFact};

use super::{build_context::RfqLifecycleFact, process_request::CloseRfqError};

pub(super) struct LoadedRfqLifecycle {
    pub(super) facts: Vec<RfqLifecycleFact>,
    pub(super) current_context_version: Option<u64>,
}

pub(super) fn rfq_lifecycle_query(rfq_id: Uuid) -> EventQuery {
    EventQuery::all().with_filters([
        EventFilter::for_event_types([RFQ_OPENED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
        EventFilter::for_event_types([RFQ_CLOSED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
    ])
}

pub(super) fn load_context(
    store: &impl EventStore,
    query: &EventQuery,
) -> Result<LoadedRfqLifecycle, CloseRfqError> {
    let result = store
        .query(query)
        .map_err(|error| CloseRfqError::Store(error.to_string()))?;
    let mut facts = Vec::with_capacity(result.event_records.len());

    for record in result.event_records {
        match record.event_type.as_str() {
            RFQ_OPENED_V1 => {
                let _fact: RfqOpenedFact =
                    serde_json::from_value(record.payload).map_err(|error| {
                        CloseRfqError::Store(format!("invalid RFQ opened fact: {error}"))
                    })?;
                facts.push(RfqLifecycleFact::Opened);
            }
            RFQ_CLOSED_V1 => {
                let _fact: RfqClosedFact =
                    serde_json::from_value(record.payload).map_err(|error| {
                        CloseRfqError::Store(format!("invalid RFQ closed fact: {error}"))
                    })?;
                facts.push(RfqLifecycleFact::Closed);
            }
            _ => {}
        }
    }

    Ok(LoadedRfqLifecycle {
        facts,
        current_context_version: result.current_context_version,
    })
}
