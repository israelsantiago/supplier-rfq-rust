use crate::projections::rfq_summary_reader::{RfqSummaryPage, RfqSummaryReader};

use super::build_context::NormalizedListRequest;
use super::process_request::ListRfqSummariesError;

/// Shell de leitura: executa a consulta à projeção após validar o contexto.
pub(super) async fn load_context(
    reader: &dyn RfqSummaryReader,
    context: &NormalizedListRequest,
) -> Result<RfqSummaryPage, ListRfqSummariesError> {
    reader
        .list(context.status.as_deref(), context.limit, context.offset)
        .await
        .map_err(|error| ListRfqSummariesError::Projection(error.to_string()))
}
