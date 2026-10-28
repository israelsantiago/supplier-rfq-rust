use super::process_request::RegisterSupplierError;
use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, ContextVersion, StateQuery, StateReadError,
};
use crate::events::SupplierRegisteredFact;
use uuid::Uuid;
pub(super) struct LoadedSupplierContext {
    pub(super) existing_suppliers: Vec<SupplierRegisteredFact>,
    pub(super) current_context_version: ContextVersion,
}
pub(super) fn identity_context_query(supplier_id: Uuid, reference: &str) -> StateQuery {
    StateQuery::SupplierIdentity {
        supplier_id,
        reference: reference.to_owned(),
    }
}
pub(super) fn load_context(
    state: &dyn ApplicationStateProvider,
    query: &StateQuery,
) -> Result<LoadedSupplierContext, RegisterSupplierError> {
    let snapshot = state
        .read(query)
        .map_err(|error: StateReadError| RegisterSupplierError::Store(error.to_string()))?;
    let existing_suppliers = snapshot
        .facts
        .into_iter()
        .filter_map(|record| match record.fact {
            ApplicationFact::SupplierRegistered(fact) => Some(fact),
            _ => None,
        })
        .collect();
    Ok(LoadedSupplierContext {
        existing_suppliers,
        current_context_version: snapshot.version,
    })
}
