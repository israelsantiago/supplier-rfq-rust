//! Runtime dependencies for HTTP delivery. FACTSTR remains authoritative; the RFQ summary is a derived read model.

use std::sync::Arc;

use factstr_postgres::PostgresStore;

use crate::projections::rfq_summary_projection::RfqSummaryProjection;

#[derive(Clone)]
pub struct ApiState {
    pub event_store: Arc<PostgresStore>,
    pub rfq_summary_projection: Arc<RfqSummaryProjection>,
}

impl ApiState {
    pub fn new(store: PostgresStore, rfq_summary_projection: RfqSummaryProjection) -> Self {
        Self {
            event_store: Arc::new(store),
            rfq_summary_projection: Arc::new(rfq_summary_projection),
        }
    }

    pub fn from_shared_event_store(
        store: Arc<PostgresStore>,
        rfq_summary_projection: Arc<RfqSummaryProjection>,
    ) -> Self {
        Self {
            event_store: store,
            rfq_summary_projection,
        }
    }
}
