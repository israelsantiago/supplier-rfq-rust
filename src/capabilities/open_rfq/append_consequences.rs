use factstr::{EventQuery, EventStore, EventStoreError, NewEvent};
use serde_json::to_value;

use crate::events::{RFQ_OPENED_V1, RfqOpenedFact};

#[derive(Debug)]
pub(super) enum AppendConsequencesError {
    ContextChanged,
    Store(String),
}

/// Shell de persistência: append_if protege atomicamente o contexto da decisão.
pub(super) fn append_consequences(
    store: &impl EventStore,
    context_query: &EventQuery,
    expected_context_version: Option<u64>,
    fact: &RfqOpenedFact,
) -> Result<(), AppendConsequencesError> {
    let payload =
        to_value(fact).map_err(|error| AppendConsequencesError::Store(error.to_string()))?;
    let event = NewEvent::new(RFQ_OPENED_V1, payload);

    match store.append_if(vec![event], context_query, expected_context_version) {
        Ok(_) => Ok(()),
        Err(EventStoreError::ConditionalAppendConflict { .. }) => {
            Err(AppendConsequencesError::ContextChanged)
        }
        Err(error) => Err(AppendConsequencesError::Store(error.to_string())),
    }
}
