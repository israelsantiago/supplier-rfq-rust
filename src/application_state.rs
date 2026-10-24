//! Application state: FACTSTR is authoritative; the RFQ summary is a derived read model.

use std::sync::Arc;

use factstr_postgres::PostgresStore;

use crate::projections::rfq_summary::RfqSummaryProjection;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<PostgresStore>,
    pub rfq_summary_projection: Arc<RfqSummaryProjection>,
}

impl AppState {
    pub fn new(store: PostgresStore, rfq_summary_projection: RfqSummaryProjection) -> Self {
        Self {
            store: Arc::new(store),
            rfq_summary_projection: Arc::new(rfq_summary_projection),
        }
    }

    pub fn from_shared_store(
        store: Arc<PostgresStore>,
        rfq_summary_projection: Arc<RfqSummaryProjection>,
    ) -> Self {
        Self {
            store,
            rfq_summary_projection,
        }
    }
}
