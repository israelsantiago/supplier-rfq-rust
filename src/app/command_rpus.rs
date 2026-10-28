//! Public application facade for command RPUs.
//!
//! Delivery mechanisms call these request/response methods. Runtime providers and
//! EventStore plumbing stay behind this boundary; tests can still inject providers.
use std::sync::Arc;

use factstr::EventStore;
use uuid::Uuid;

use crate::{
    capabilities::{
        close_rfq::{self, CloseRfqError, CloseRfqRequest},
        open_rfq::{self, OpenRfqError, OpenRfqRequest},
        register_supplier::{self, RegisterSupplierError, RegisterSupplierRequest},
        set_supplier_eligibility::{
            self, SetSupplierEligibilityError, SetSupplierEligibilityRequest,
        },
        submit_supplier_quote::{self, PrepareError, SubmitSupplierQuoteRequest},
    },
    events::{
        RfqClosedFact, RfqOpenedFact, SupplierEligibilityChangedFact, SupplierQuoteSubmittedFact,
        SupplierRegisteredFact,
    },
    providers::{ClockProvider, IdProvider},
};

pub struct CommandRpus<S>
where
    S: EventStore + Send + Sync + 'static,
{
    store: Arc<S>,
    clock: Arc<dyn ClockProvider>,
    ids: Arc<dyn IdProvider>,
}

impl<S> CommandRpus<S>
where
    S: EventStore + Send + Sync + 'static,
{
    /// Composition seam. Delivery handlers do not receive or inspect Providers.
    pub fn new(store: Arc<S>, clock: Arc<dyn ClockProvider>, ids: Arc<dyn IdProvider>) -> Self {
        Self { store, clock, ids }
    }

    pub fn open_rfq(&self, request: OpenRfqRequest) -> Result<RfqOpenedFact, OpenRfqError> {
        open_rfq::process_with_providers(
            self.store.as_ref(),
            request,
            self.clock.as_ref(),
            self.ids.as_ref(),
        )
    }

    pub fn register_supplier(
        &self,
        request: RegisterSupplierRequest,
    ) -> Result<SupplierRegisteredFact, RegisterSupplierError> {
        register_supplier::process_with_providers(
            self.store.as_ref(),
            request,
            self.clock.as_ref(),
            self.ids.as_ref(),
        )
    }

    pub fn set_supplier_eligibility(
        &self,
        supplier_id: Uuid,
        request: SetSupplierEligibilityRequest,
    ) -> Result<SupplierEligibilityChangedFact, SetSupplierEligibilityError> {
        set_supplier_eligibility::process_with_providers(
            self.store.as_ref(),
            supplier_id,
            request,
            self.clock.as_ref(),
        )
    }

    pub fn submit_supplier_quote(
        &self,
        rfq_id: Uuid,
        request: SubmitSupplierQuoteRequest,
    ) -> Result<SupplierQuoteSubmittedFact, PrepareError> {
        submit_supplier_quote::process_with_providers(
            self.store.as_ref(),
            rfq_id,
            request,
            self.clock.as_ref(),
            self.ids.as_ref(),
        )
    }

    pub fn close_rfq(&self, request: CloseRfqRequest) -> Result<RfqClosedFact, CloseRfqError> {
        close_rfq::process_with_providers(self.store.as_ref(), request, self.clock.as_ref())
    }
}
