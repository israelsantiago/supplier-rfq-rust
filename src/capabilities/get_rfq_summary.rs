//! Query RPU: GetRfqSummary.
//! Consulta o resumo projetado de um RFQ.

use thiserror::Error;
use uuid::Uuid;

use crate::projections::rfq_summary::{RfqSummary, RfqSummaryProjection};

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

pub async fn process(
    projection: &RfqSummaryProjection,
    request: GetRfqSummaryRequest,
) -> Result<RfqSummary, GetRfqSummaryError> {
    match projection.get(request.rfq_id).await {
        Ok(Some(summary)) => Ok(summary),
        Ok(None) => Err(GetRfqSummaryError::NotAvailableYet),
        Err(error) => Err(GetRfqSummaryError::Projection(error.to_string())),
    }
}
