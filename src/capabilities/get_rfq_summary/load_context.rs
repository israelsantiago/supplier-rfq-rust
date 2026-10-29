use uuid::Uuid;

use crate::projections::rfq_summary_reader::{RfqSummary, RfqSummaryReadError, RfqSummaryReader};

/// Shell de leitura: não interpreta a ausência do resumo como erro de negócio.
pub(super) async fn load_context(
    reader: &dyn RfqSummaryReader,
    rfq_id: Uuid,
) -> Result<Option<RfqSummary>, RfqSummaryReadError> {
    reader.get(rfq_id).await
}
