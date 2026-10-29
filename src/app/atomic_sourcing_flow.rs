//! Composition facade for the separate atomic sourcing-finalization flow.
//! It intentionally has no dependency on the legacy `CommandRpus` facade.

use std::sync::Arc;

use crate::capabilities::finalize_rfq_sourcing::{
    AtomicSourcingStateProvider, FinalizeRfqSourcing, FinalizeRfqSourcingError,
    FinalizeRfqSourcingRequest, SourcingDecision,
};
use crate::providers::{ClockProvider, IdProvider};

pub struct AtomicSourcingFlow {
    finalize: FinalizeRfqSourcing,
}

impl AtomicSourcingFlow {
    pub fn new(
        state: Arc<dyn AtomicSourcingStateProvider>,
        clock: Arc<dyn ClockProvider>,
        ids: Arc<dyn IdProvider>,
    ) -> Self {
        Self {
            finalize: FinalizeRfqSourcing::new(state, clock, ids),
        }
    }

    pub fn finalize_rfq_sourcing(
        &self,
        request: FinalizeRfqSourcingRequest,
    ) -> Result<SourcingDecision, FinalizeRfqSourcingError> {
        self.finalize.finalize(request)
    }
}
