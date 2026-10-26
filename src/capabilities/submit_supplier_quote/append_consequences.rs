use factstr::{EventQuery, EventStore, EventStoreError, NewEvent};

use super::process_request::CommitError;

/// Shell de escrita: tenta anexar a cotação somente contra o contexto observado.
pub(super) fn append_consequences(
    store: &impl EventStore,
    query: &EventQuery,
    expected_context_version: Option<u64>,
    event: NewEvent,
) -> Result<(), CommitError> {
    store
        .append_if(vec![event], query, expected_context_version)
        .map(|_| ())
        .map_err(|error| match error {
            EventStoreError::ConditionalAppendConflict { .. } => CommitError::ContextChanged,
            other => CommitError::Store(other.to_string()),
        })
}
