use super::build_context::SubmissionContextFact;
use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, ContextVersion, StateQuery, StateReadError,
};
use uuid::Uuid;
#[derive(Debug)]
pub(super) struct LoadedSubmissionContext {
    pub(super) query: StateQuery,
    pub(super) current_context_version: ContextVersion,
    pub(super) facts: Vec<SubmissionContextFact>,
}
pub(super) fn submission_context_query(rfq_id: Uuid, supplier_id: Uuid) -> StateQuery {
    StateQuery::SupplierQuoteSubmission {
        rfq_id,
        supplier_id,
    }
}
pub(super) fn load_context(
    state: &dyn ApplicationStateProvider,
    rfq_id: Uuid,
    supplier_id: Uuid,
) -> Result<LoadedSubmissionContext, StateReadError> {
    let query = submission_context_query(rfq_id, supplier_id);
    let snapshot = state.read(&query)?;
    let facts = snapshot
        .facts
        .into_iter()
        .map(|record| match record.fact {
            ApplicationFact::RfqOpened(fact) => SubmissionContextFact::RfqOpened(fact),
            ApplicationFact::RfqClosed(_) => SubmissionContextFact::RfqClosed,
            ApplicationFact::SupplierRegistered(_) => SubmissionContextFact::SupplierRegistered,
            ApplicationFact::SupplierEligibilityChanged(fact) => {
                SubmissionContextFact::SupplierEligibilityChanged(fact.status)
            }
            ApplicationFact::SupplierQuoteSubmitted(_) => {
                SubmissionContextFact::SupplierQuoteSubmitted
            }
        })
        .collect();
    Ok(LoadedSubmissionContext {
        query,
        current_context_version: snapshot.version,
        facts,
    })
}
