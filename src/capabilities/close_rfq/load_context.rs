use super::{build_context::RfqLifecycleFact, process_request::CloseRfqError};
use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, ContextVersion, StateQuery,
};
use uuid::Uuid;
pub(super) struct LoadedRfqLifecycle {
    pub(super) facts: Vec<RfqLifecycleFact>,
    pub(super) current_context_version: ContextVersion,
}
pub(super) fn rfq_lifecycle_query(rfq_id: Uuid) -> StateQuery {
    StateQuery::RfqLifecycle { rfq_id }
}
pub(super) fn load_context(
    state: &dyn ApplicationStateProvider,
    query: &StateQuery,
) -> Result<LoadedRfqLifecycle, CloseRfqError> {
    let snapshot = state
        .read(query)
        .map_err(|error| CloseRfqError::Store(error.to_string()))?;
    let facts = snapshot
        .facts
        .into_iter()
        .filter_map(|record| match record.fact {
            ApplicationFact::RfqOpened(_) => Some(RfqLifecycleFact::Opened),
            ApplicationFact::RfqClosed(_) => Some(RfqLifecycleFact::Closed),
            _ => None,
        })
        .collect();
    Ok(LoadedRfqLifecycle {
        facts,
        current_context_version: snapshot.version,
    })
}
