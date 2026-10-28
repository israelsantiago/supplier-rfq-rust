use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, ContextVersion, StateQuery, StateWriteError,
};
use crate::events::RfqOpenedFact;
#[derive(Debug)]
pub(super) enum AppendConsequencesError {
    ContextChanged,
    Store(String),
}
pub(super) fn append_consequences(
    state: &dyn ApplicationStateProvider,
    query: &StateQuery,
    expected: &ContextVersion,
    fact: &RfqOpenedFact,
) -> Result<(), AppendConsequencesError> {
    state
        .append_if(
            query,
            expected,
            vec![ApplicationFact::RfqOpened(fact.clone())],
        )
        .map_err(|error| match error {
            StateWriteError::ContextChanged => AppendConsequencesError::ContextChanged,
            other => AppendConsequencesError::Store(other.to_string()),
        })
}
