//! Query RPU: ListRfqSummaries.
//! Valida os filtros e consulta uma página do resumo projetado.

use thiserror::Error;

use crate::projections::rfq_summary_projection::{RfqSummaryPage, RfqSummaryProjection};

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedListRequest {
    status: Option<String>,
    limit: i64,
    offset: i64,
}

fn normalize_request(
    request: ListRfqSummariesRequest,
) -> Result<NormalizedListRequest, ListRfqSummariesError> {
    let limit = request.limit.unwrap_or(50);
    let offset = request.offset.unwrap_or(0);

    if !(1..=100).contains(&limit) {
        return Err(ListRfqSummariesError::InvalidLimit);
    }

    if offset < 0 {
        return Err(ListRfqSummariesError::InvalidOffset);
    }

    let status = match request.status.as_deref() {
        None => None,
        Some("open") => Some("open".to_owned()),
        Some("closed") => Some("closed".to_owned()),
        Some(_) => return Err(ListRfqSummariesError::InvalidStatus),
    };

    Ok(NormalizedListRequest {
        status,
        limit,
        offset,
    })
}

pub async fn process(
    projection: &RfqSummaryProjection,
    request: ListRfqSummariesRequest,
) -> Result<RfqSummaryPage, ListRfqSummariesError> {
    let normalized = normalize_request(request)?;

    projection
        .list(
            normalized.status.as_deref(),
            normalized.limit,
            normalized.offset,
        )
        .await
        .map_err(|error| ListRfqSummariesError::Projection(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        status: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> ListRfqSummariesRequest {
        ListRfqSummariesRequest {
            status: status.map(str::to_owned),
            limit,
            offset,
        }
    }

    #[test]
    fn defaults_are_limit_50_offset_0_without_status_filter() {
        assert_eq!(
            normalize_request(request(None, None, None)).unwrap(),
            NormalizedListRequest {
                status: None,
                limit: 50,
                offset: 0,
            }
        );
    }

    #[test]
    fn accepts_only_open_and_closed_statuses() {
        assert_eq!(
            normalize_request(request(Some("open"), None, None))
                .unwrap()
                .status
                .as_deref(),
            Some("open")
        );

        assert_eq!(
            normalize_request(request(Some("closed"), None, None))
                .unwrap()
                .status
                .as_deref(),
            Some("closed")
        );

        assert!(matches!(
            normalize_request(request(Some("cancelled"), None, None)),
            Err(ListRfqSummariesError::InvalidStatus)
        ));
    }

    #[test]
    fn rejects_limits_outside_one_to_one_hundred() {
        for limit in [0, -1, 101] {
            assert!(matches!(
                normalize_request(request(None, Some(limit), None)),
                Err(ListRfqSummariesError::InvalidLimit)
            ));
        }
    }

    #[test]
    fn rejects_negative_offset() {
        assert!(matches!(
            normalize_request(request(None, None, Some(-1))),
            Err(ListRfqSummariesError::InvalidOffset)
        ));
    }
}
