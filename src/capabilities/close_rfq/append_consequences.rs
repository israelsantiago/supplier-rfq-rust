use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, ContextVersion, StateQuery, StateWriteError,
};
use crate::events::RfqClosedFact;
#[derive(Debug)]
pub(super) enum AppendConsequencesError {
    ContextChanged,
    Store(String),
}
pub(super) fn append_consequences(
    state: &dyn ApplicationStateProvider,
    query: &StateQuery,
    expected: &ContextVersion,
    fact: &RfqClosedFact,
) -> Result<(), AppendConsequencesError> {
    state
        .append_if(
            query,
            expected,
            vec![ApplicationFact::RfqClosed(fact.clone())],
        )
        .map_err(|error| match error {
            StateWriteError::ContextChanged => AppendConsequencesError::ContextChanged,
            other => AppendConsequencesError::Store(other.to_string()),
        })
}
