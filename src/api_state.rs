//! Composition root for HTTP delivery: only facades and the separate idempotency ledger escape.
use std::sync::Arc;

use factstr_postgres::PostgresStore;

use crate::{
    app::{CommandRpus, QueryRpus, atomic_sourcing_flow::AtomicSourcingFlow},
    application_state::{
        ApplicationStateProvider, atomic_sourcing_factstr::FactstrAtomicSourcingStateProvider,
        factstr::FactstrApplicationStateProvider,
    },
    http::idempotency::HttpIdempotencyStore,
    projections::rfq_summary_projection::RfqSummaryProjection,
    providers::{ClockProvider, IdProvider, SystemClockProvider, UuidV4Provider},
};

#[derive(Clone)]
pub struct ApiState {
    pub commands: Arc<CommandRpus>,
    pub queries: Arc<QueryRpus>,
    /// Present in the production composition root; absent only when tests inject
    /// an abstract ApplicationStateProvider without a concrete FACTSTR store.
    pub atomic_sourcing: Option<Arc<AtomicSourcingFlow>>,
    pub http_idempotency_store: HttpIdempotencyStore,
}

impl ApiState {
    pub fn new(
        store: PostgresStore,
        projection: RfqSummaryProjection,
        idempotency_store: HttpIdempotencyStore,
    ) -> Self {
        Self::from_shared_event_store(Arc::new(store), Arc::new(projection), idempotency_store)
    }

    pub fn from_shared_event_store(
        store: Arc<PostgresStore>,
        projection: Arc<RfqSummaryProjection>,
        idempotency_store: HttpIdempotencyStore,
    ) -> Self {
        Self::from_shared_event_store_with_providers(
            store,
            projection,
            Arc::new(SystemClockProvider),
            Arc::new(UuidV4Provider),
            idempotency_store,
        )
    }

    pub fn from_shared_event_store_with_providers(
        store: Arc<PostgresStore>,
        projection: Arc<RfqSummaryProjection>,
        clock_provider: Arc<dyn ClockProvider>,
        id_provider: Arc<dyn IdProvider>,
        idempotency_store: HttpIdempotencyStore,
    ) -> Self {
        let application_state: Arc<dyn ApplicationStateProvider> =
            Arc::new(FactstrApplicationStateProvider::new(Arc::clone(&store)));
        let commands = Arc::new(CommandRpus::new(
            Arc::clone(&application_state),
            Arc::clone(&clock_provider),
            Arc::clone(&id_provider),
        ));
        let queries = Arc::new(QueryRpus::new(application_state, projection));
        let atomic_sourcing_state = Arc::new(FactstrAtomicSourcingStateProvider::new(store));
        let atomic_sourcing = Arc::new(AtomicSourcingFlow::new(
            atomic_sourcing_state,
            clock_provider,
            id_provider,
        ));
        Self {
            commands,
            queries,
            atomic_sourcing: Some(atomic_sourcing),
            http_idempotency_store: idempotency_store,
        }
    }

    /// Injection seam for alternate state providers in adapter/contract tests.
    pub fn from_application_state_provider(
        application_state: Arc<dyn ApplicationStateProvider>,
        projection: Arc<RfqSummaryProjection>,
        clock_provider: Arc<dyn ClockProvider>,
        id_provider: Arc<dyn IdProvider>,
        idempotency_store: HttpIdempotencyStore,
    ) -> Self {
        let commands = Arc::new(CommandRpus::new(
            Arc::clone(&application_state),
            clock_provider,
            id_provider,
        ));
        let queries = Arc::new(QueryRpus::new(application_state, projection));
        Self {
            commands,
            queries,
            atomic_sourcing: None,
            http_idempotency_store: idempotency_store,
        }
    }
}
