use super::process_request::CommitError;
use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, ContextVersion, StateQuery,
};
use crate::events::SupplierQuoteSubmittedFact;
pub(super) fn append_consequences(
    state: &dyn ApplicationStateProvider,
    query: &StateQuery,
    expected: &ContextVersion,
    quote: &SupplierQuoteSubmittedFact,
) -> Result<(), CommitError> {
    state
        .append_if(
            query,
            expected,
            vec![ApplicationFact::SupplierQuoteSubmitted(quote.clone())],
        )
        .map_err(CommitError::from)
}
