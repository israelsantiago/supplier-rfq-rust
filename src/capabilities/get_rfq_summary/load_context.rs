use uuid::Uuid;

use crate::projections::rfq_summary_projection::{RfqSummary, RfqSummaryProjection};

/// Shell de leitura: não interpreta a ausência do resumo como erro de negócio.
pub(super) async fn load_context(
    projection: &RfqSummaryProjection,
    rfq_id: Uuid,
) -> Result<Option<RfqSummary>, sqlx::Error> {
    projection.get(rfq_id).await
}
