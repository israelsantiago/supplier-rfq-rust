use thiserror::Error;
use uuid::Uuid;

use crate::projections::rfq_summary_projection::{RfqSummary, RfqSummaryProjection};

use super::load_context;

#[derive(Debug, Clone, Copy)]
pub struct GetRfqSummaryRequest {
    pub rfq_id: Uuid,
}

#[derive(Debug, Error)]
pub enum GetRfqSummaryError {
    #[error("RFQ summary is not available in the read projection yet")]
    NotAvailableYet,
    #[error("RFQ summary projection error: {0}")]
    Projection(String),
}

/// Shell do RPU: carrega a projeção e converte o resultado em resposta de query.
pub async fn process(
    projection: &RfqSummaryProjection,
    request: GetRfqSummaryRequest,
) -> Result<RfqSummary, GetRfqSummaryError> {
    match load_context::load_context(projection, request.rfq_id).await {
        Ok(Some(summary)) => Ok(summary),
        Ok(None) => Err(GetRfqSummaryError::NotAvailableYet),
        Err(error) => Err(GetRfqSummaryError::Projection(error.to_string())),
    }
}
