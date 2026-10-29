//! New composite business capability: finalizes sourcing with one atomic state write.
//!
//! This module intentionally does not call `CommandRpus` or any existing capability
//! shell. Existing per-command RPUs retain their contracts and individual CCC commits.

mod decision;
pub mod state;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::providers::{ClockProvider, IdProvider};

pub use decision::{SourcingContext, SourcingDecision, SourcingRejection, decide};
pub use state::{
    AtomicSourcingStateProvider, SourcingContextSnapshot, SourcingContextVersion,
    SourcingStateError,
};

/// Domain command. The quote-set version is the maximum `sequence_number` returned
/// by the quote list the buyer actually reviewed, not the store's global sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizeRfqSourcingRequest {
    pub rfq_id: Uuid,
    pub supplier_id: Uuid,
    pub quote_id: Uuid,
    pub expected_quote_set_version: u64,
}

#[derive(Debug, Error)]
pub enum FinalizeRfqSourcingError {
    #[error(transparent)]
    Rejected(#[from] SourcingRejection),
    #[error("FACTSTR sourcing-finalization operation failed: {0}")]
    Store(String),
}

/// Prepared command has no durable effects. Hidden from normal application APIs; the
/// feature-gated seam exists solely to force deterministic races in integration tests.
#[doc(hidden)]
pub struct PreparedSourcingCommit {
    request: FinalizeRfqSourcingRequest,
    expected_context_version: SourcingContextVersion,
    decision: SourcingDecision,
}

pub struct FinalizeRfqSourcing {
    state: Arc<dyn AtomicSourcingStateProvider>,
    clock: Arc<dyn ClockProvider>,
    ids: Arc<dyn IdProvider>,
}

impl FinalizeRfqSourcing {
    pub fn new(
        state: Arc<dyn AtomicSourcingStateProvider>,
        clock: Arc<dyn ClockProvider>,
        ids: Arc<dyn IdProvider>,
    ) -> Self {
        Self { state, clock, ids }
    }

    /// Execute one logical command. Clock and IDs are captured exactly once; a CCC
    /// conflict is returned to the caller so a human supplier selection is reviewed again.
    pub fn finalize(
        &self,
        request: FinalizeRfqSourcingRequest,
    ) -> Result<SourcingDecision, FinalizeRfqSourcingError> {
        let now = self.clock.now_utc();
        let award_id = self.ids.new_uuid();
        let purchase_order_id = self.ids.new_uuid();
        let dispatch_id = self.ids.new_uuid();
        let prepared = prepare_with_values(
            self.state.as_ref(),
            request,
            now,
            award_id,
            purchase_order_id,
            dispatch_id,
        )?;
        commit_prepared(self.state.as_ref(), prepared)
    }
}

fn prepare_with_values(
    state: &dyn AtomicSourcingStateProvider,
    request: FinalizeRfqSourcingRequest,
    now: DateTime<Utc>,
    award_id: Uuid,
    purchase_order_id: Uuid,
    dispatch_id: Uuid,
) -> Result<PreparedSourcingCommit, FinalizeRfqSourcingError> {
    let snapshot = state.read_context(&request).map_err(map_state_error)?;
    let decision = decide(
        &request,
        &snapshot.context,
        now,
        award_id,
        purchase_order_id,
        dispatch_id,
    )?;
    Ok(PreparedSourcingCommit {
        request,
        expected_context_version: snapshot.expected_version,
        decision,
    })
}

fn commit_prepared(
    state: &dyn AtomicSourcingStateProvider,
    prepared: PreparedSourcingCommit,
) -> Result<SourcingDecision, FinalizeRfqSourcingError> {
    state
        .append_if(
            &prepared.request,
            &prepared.expected_context_version,
            &prepared.decision,
        )
        .map_err(map_state_error)?;
    Ok(prepared.decision)
}

fn map_state_error(error: SourcingStateError) -> FinalizeRfqSourcingError {
    match error {
        SourcingStateError::ContextChanged => {
            FinalizeRfqSourcingError::Rejected(SourcingRejection::CommandContextChanged)
        }
        SourcingStateError::Store(message) => FinalizeRfqSourcingError::Store(message),
    }
}

/// Explicit preparation seam for PostgreSQL acceptance tests. The returned value has no
/// state effects until the one provider append succeeds.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    use super::*;

    pub fn prepare(
        state: &dyn AtomicSourcingStateProvider,
        request: FinalizeRfqSourcingRequest,
        now: DateTime<Utc>,
        award_id: Uuid,
        purchase_order_id: Uuid,
        dispatch_id: Uuid,
    ) -> Result<PreparedSourcingCommit, FinalizeRfqSourcingError> {
        super::prepare_with_values(
            state,
            request,
            now,
            award_id,
            purchase_order_id,
            dispatch_id,
        )
    }

    pub fn commit_prepared(
        state: &dyn AtomicSourcingStateProvider,
        prepared: PreparedSourcingCommit,
    ) -> Result<SourcingDecision, FinalizeRfqSourcingError> {
        super::commit_prepared(state, prepared)
    }
}
