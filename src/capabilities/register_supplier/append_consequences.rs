use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, ContextVersion, StateQuery, StateWriteError,
};
use crate::events::SupplierRegisteredFact;
#[derive(Debug)]
pub(super) enum AppendConsequencesError {
    ContextChanged,
    Store(String),
}
pub(super) fn append_consequences(
    state: &dyn ApplicationStateProvider,
    query: &StateQuery,
    expected: &ContextVersion,
    fact: &SupplierRegisteredFact,
) -> Result<(), AppendConsequencesError> {
    state
        .append_if(
            query,
            expected,
            vec![ApplicationFact::SupplierRegistered(fact.clone())],
        )
        .map_err(|error| match error {
            StateWriteError::ContextChanged => AppendConsequencesError::ContextChanged,
            other => AppendConsequencesError::Store(other.to_string()),
        })
}
