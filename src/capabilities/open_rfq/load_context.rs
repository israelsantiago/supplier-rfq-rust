use super::process_request::OpenRfqError;
use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, ContextVersion, StateQuery, StateReadError,
    StoredFact,
};
use uuid::Uuid;

pub(super) struct LoadedRfqContext {
    pub(super) facts: Vec<StoredFact>,
    pub(super) current_context_version: ContextVersion,
}
pub(super) fn context_query(rfq_id: Uuid, reference: &str) -> StateQuery {
    StateQuery::RfqIdentity {
        rfq_id,
        reference: reference.to_owned(),
    }
}
pub(super) fn load_context(
    state: &dyn ApplicationStateProvider,
    query: &StateQuery,
) -> Result<LoadedRfqContext, OpenRfqError> {
    let snapshot = state
        .read(query)
        .map_err(|error: StateReadError| OpenRfqError::Store(error.to_string()))?;
    let facts = snapshot
        .facts
        .into_iter()
        .filter(|record| matches!(&record.fact, ApplicationFact::RfqOpened(_)))
        .collect();
    Ok(LoadedRfqContext {
        facts,
        current_context_version: snapshot.version,
    })
}
