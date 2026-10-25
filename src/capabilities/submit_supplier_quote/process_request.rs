//! RPU: query the decision context, run the pure decision, then conditionally append.

use factstr::{EventStore, EventStoreError, NewEvent};
use serde_json::to_value;
use thiserror::Error;
use uuid::Uuid;

use crate::events::{SUPPLIER_QUOTE_SUBMITTED_V1, SupplierQuoteSubmittedFact};

use super::{
    context::{LoadedSubmissionContext, load_context},
    decision::{SubmissionRejection, decide},
    request::SubmitSupplierQuoteRequest,
};

#[derive(Debug, Error)]
pub enum PrepareError {
    #[error(transparent)]
    Rejected(#[from] SubmissionRejection),
    #[error("event store error: {0}")]
    Store(String),
}

#[derive(Debug, Error)]
pub enum CommitError {
    #[error("command context changed after the decision was made")]
    ContextChanged,
    #[error("event store error: {0}")]
    Store(String),
}

#[derive(Debug)]
pub struct PreparedSubmission {
    context: LoadedSubmissionContext,
    event: NewEvent,
    pub quote: SupplierQuoteSubmittedFact,
}

impl PreparedSubmission {
    /// Context version observed by the RPU before its pure decision was made.
    /// Exposed to support deterministic CCC tests and operational diagnostics.
    pub fn expected_context_version(&self) -> Option<u64> {
        self.context.current_context_version
    }
}

pub fn prepare(
    store: &impl EventStore,
    rfq_id: Uuid,
    request: SubmitSupplierQuoteRequest,
) -> Result<PreparedSubmission, PrepareError> {
    let loaded = load_context(store, rfq_id, request.supplier_id)
        .map_err(|error| PrepareError::Store(error.to_string()))?;
    let quote = decide(
        &request,
        &loaded.context,
        Uuid::new_v4(),
        chrono::Utc::now(),
    )?;
    let payload = to_value(&quote).map_err(|error| PrepareError::Store(error.to_string()))?;
    let event = NewEvent::new(SUPPLIER_QUOTE_SUBMITTED_V1, payload);

    Ok(PreparedSubmission {
        context: loaded,
        event,
        quote,
    })
}

/// Exposed as a separate operation to make CCC's read/decide/conditional-append
/// boundary directly testable with two independent FACTSTR/PostgreSQL stores.
pub fn commit_prepared(
    store: &impl EventStore,
    prepared: PreparedSubmission,
) -> Result<SupplierQuoteSubmittedFact, CommitError> {
    store
        .append_if(
            vec![prepared.event],
            &prepared.context.query,
            prepared.context.current_context_version,
        )
        .map(|_| prepared.quote)
        .map_err(|error| match error {
            EventStoreError::ConditionalAppendConflict { .. } => CommitError::ContextChanged,
            other => CommitError::Store(other.to_string()),
        })
}

pub fn process(
    store: &impl EventStore,
    rfq_id: Uuid,
    request: SubmitSupplierQuoteRequest,
) -> Result<SupplierQuoteSubmittedFact, PrepareError> {
    // A single re-read after CCC conflict turns common races into the current
    // business rejection (duplicate or closed), without blind retry loops.
    for attempt in 0..2 {
        let prepared = prepare(store, rfq_id, request.clone())?;
        match commit_prepared(store, prepared) {
            Ok(quote) => return Ok(quote),
            Err(CommitError::ContextChanged) if attempt == 0 => continue,
            Err(CommitError::ContextChanged) => {
                return Err(PrepareError::Rejected(
                    SubmissionRejection::CommandContextChanged,
                ));
            }
            Err(CommitError::Store(message)) => return Err(PrepareError::Store(message)),
        }
    }
    Err(PrepareError::Rejected(
        SubmissionRejection::CommandContextChanged,
    ))
}
