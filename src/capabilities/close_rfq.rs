//! Domain Capability / RPU: CloseRfq.

use factstr::{EventFilter, EventQuery, EventStore, EventStoreError, NewEvent};
use serde_json::{json, to_value};
use thiserror::Error;
use uuid::Uuid;

use crate::facts::{RFQ_CLOSED_V1, RFQ_OPENED_V1, RfqClosedFact, RfqOpenedFact};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CloseRfqRejection {
    #[error("RFQ was not found")]
    RfqNotFound,
    #[error("RFQ is already closed")]
    RfqAlreadyClosed,
    #[error("RFQ context changed concurrently; retry the command")]
    CommandContextChanged,
}

#[derive(Debug, Error)]
pub enum CloseRfqError {
    #[error(transparent)]
    Rejected(#[from] CloseRfqRejection),
    #[error("event store error: {0}")]
    Store(String),
}

pub fn process(store: &impl EventStore, rfq_id: Uuid) -> Result<RfqClosedFact, CloseRfqError> {
    let query = EventQuery::all().with_filters([
        EventFilter::for_event_types([RFQ_OPENED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
        EventFilter::for_event_types([RFQ_CLOSED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
    ]);

    for attempt in 0..2 {
        let result = store
            .query(&query)
            .map_err(|error| CloseRfqError::Store(error.to_string()))?;
        let mut opened = false;
        let mut closed = false;
        for record in &result.event_records {
            match record.event_type.as_str() {
                RFQ_OPENED_V1 => {
                    let _fact: RfqOpenedFact = serde_json::from_value(record.payload.clone())
                        .map_err(|error| {
                            CloseRfqError::Store(format!("invalid RFQ opened fact: {error}"))
                        })?;
                    opened = true;
                }
                RFQ_CLOSED_V1 => {
                    let _fact: RfqClosedFact = serde_json::from_value(record.payload.clone())
                        .map_err(|error| {
                            CloseRfqError::Store(format!("invalid RFQ closed fact: {error}"))
                        })?;
                    closed = true;
                }
                _ => {}
            }
        }
        if !opened {
            return Err(CloseRfqRejection::RfqNotFound.into());
        }
        if closed {
            return Err(CloseRfqRejection::RfqAlreadyClosed.into());
        }

        let fact = RfqClosedFact {
            rfq_id,
            closed_at: chrono::Utc::now(),
        };
        let event = NewEvent::new(
            RFQ_CLOSED_V1,
            to_value(&fact).map_err(|error| CloseRfqError::Store(error.to_string()))?,
        );
        match store.append_if(vec![event], &query, result.current_context_version) {
            Ok(_) => return Ok(fact),
            Err(EventStoreError::ConditionalAppendConflict { .. }) if attempt == 0 => continue,
            Err(EventStoreError::ConditionalAppendConflict { .. }) => {
                return Err(CloseRfqRejection::CommandContextChanged.into());
            }
            Err(error) => return Err(CloseRfqError::Store(error.to_string())),
        }
    }

    Err(CloseRfqRejection::CommandContextChanged.into())
}
