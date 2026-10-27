//! Runtime dependencies for HTTP delivery. FACTSTR remains authoritative; the RFQ summary is a derived read model.

use std::sync::Arc;

use factstr_postgres::PostgresStore;

use crate::{
    projections::rfq_summary_projection::RfqSummaryProjection,
    providers::{ClockProvider, IdProvider, SystemClockProvider, UuidV4Provider},
};

#[derive(Clone)]
pub struct ApiState {
    pub event_store: Arc<PostgresStore>,
    pub rfq_summary_projection: Arc<RfqSummaryProjection>,
    pub clock_provider: Arc<dyn ClockProvider>,
    pub id_provider: Arc<dyn IdProvider>,
}

impl ApiState {
    /// Compatibility constructor using the production providers.
    pub fn new(store: PostgresStore, rfq_summary_projection: RfqSummaryProjection) -> Self {
        Self::from_shared_event_store_with_providers(
            Arc::new(store),
            Arc::new(rfq_summary_projection),
            Arc::new(SystemClockProvider),
            Arc::new(UuidV4Provider),
        )
    }

    /// Compatibility constructor using the production providers.
    pub fn from_shared_event_store(
        store: Arc<PostgresStore>,
        rfq_summary_projection: Arc<RfqSummaryProjection>,
    ) -> Self {
        Self::from_shared_event_store_with_providers(
            store,
            rfq_summary_projection,
            Arc::new(SystemClockProvider),
            Arc::new(UuidV4Provider),
        )
    }

    /// Composition seam: injects runtime providers without exposing them to HTTP requests.
    pub fn from_shared_event_store_with_providers(
        store: Arc<PostgresStore>,
        rfq_summary_projection: Arc<RfqSummaryProjection>,
        clock_provider: Arc<dyn ClockProvider>,
        id_provider: Arc<dyn IdProvider>,
    ) -> Self {
        Self {
            event_store: store,
            rfq_summary_projection,
            clock_provider,
            id_provider,
        }
    }
}
