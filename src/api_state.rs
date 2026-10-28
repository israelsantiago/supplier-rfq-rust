//! Runtime dependencies for HTTP delivery. FACTSTR is authoritative; the summary is a read model.

use std::sync::Arc;

use factstr_postgres::PostgresStore;

use crate::{
    app::CommandRpus,
    projections::rfq_summary_projection::RfqSummaryProjection,
    providers::{ClockProvider, IdProvider, SystemClockProvider, UuidV4Provider},
};

#[derive(Clone)]
pub struct ApiState {
    pub event_store: Arc<PostgresStore>,
    pub rfq_summary_projection: Arc<RfqSummaryProjection>,
    /// Public delivery state contains the command facade, not raw Providers.
    pub commands: Arc<CommandRpus<PostgresStore>>,
}

impl ApiState {
    /// Compatibility constructor using production Providers.
    pub fn new(store: PostgresStore, rfq_summary_projection: RfqSummaryProjection) -> Self {
        Self::from_shared_event_store_with_providers(
            Arc::new(store),
            Arc::new(rfq_summary_projection),
            Arc::new(SystemClockProvider),
            Arc::new(UuidV4Provider),
        )
    }

    /// Compatibility constructor using production Providers.
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

    /// Composition root: Providers are injected once and hidden from delivery handlers.
    pub fn from_shared_event_store_with_providers(
        store: Arc<PostgresStore>,
        rfq_summary_projection: Arc<RfqSummaryProjection>,
        clock_provider: Arc<dyn ClockProvider>,
        id_provider: Arc<dyn IdProvider>,
    ) -> Self {
        let commands = Arc::new(CommandRpus::new(
            Arc::clone(&store),
            clock_provider,
            id_provider,
        ));
        Self {
            event_store: store,
            rfq_summary_projection,
            commands,
        }
    }
}
