use thiserror::Error;

use crate::projections::rfq_summary_projection::{RfqSummaryPage, RfqSummaryProjection};

use super::{build_context, load_context};

/// Parâmetros da consulta de resumos de RFQ.
#[derive(Debug, Clone)]
pub struct ListRfqSummariesRequest {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Error)]
pub enum ListRfqSummariesError {
    #[error("limit must be between 1 and 100")]
    InvalidLimit,
    #[error("offset must be zero or greater")]
    InvalidOffset,
    #[error("status must be either open or closed")]
    InvalidStatus,
    #[error("RFQ summary projection error: {0}")]
    Projection(String),
}

/// Shell do RPU: normaliza a request, carrega a projeção e devolve a página.
pub async fn process(
    projection: &RfqSummaryProjection,
    request: ListRfqSummariesRequest,
) -> Result<RfqSummaryPage, ListRfqSummariesError> {
    let context = build_context::build_context(request)?;
    load_context::load_context(projection, &context).await
}
