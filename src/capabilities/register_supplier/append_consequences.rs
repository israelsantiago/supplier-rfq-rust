use factstr::{EventQuery, EventStore, EventStoreError, NewEvent};
use serde_json::to_value;

use crate::events::{SUPPLIER_REGISTERED_V1, SupplierRegisteredFact};

#[derive(Debug)]
pub(super) enum AppendConsequencesError {
    ContextChanged,
    Store(String),
}

/// Shell de escrita: a condição CCC vincula o append ao contexto carregado.
pub(super) fn append_consequences(
    store: &impl EventStore,
    query: &EventQuery,
    expected_context_version: Option<u64>,
    fact: &SupplierRegisteredFact,
) -> Result<(), AppendConsequencesError> {
    let payload =
        to_value(fact).map_err(|error| AppendConsequencesError::Store(error.to_string()))?;
    let event = NewEvent::new(SUPPLIER_REGISTERED_V1, payload);

    match store.append_if(vec![event], query, expected_context_version) {
        Ok(_) => Ok(()),
        Err(EventStoreError::ConditionalAppendConflict { .. }) => {
            Err(AppendConsequencesError::ContextChanged)
        }
        Err(error) => Err(AppendConsequencesError::Store(error.to_string())),
    }
}
