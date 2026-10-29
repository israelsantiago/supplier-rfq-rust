use thiserror::Error;

use crate::projections::rfq_summary_reader::{RfqSummaryPage, RfqSummaryReader};

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
    reader: &dyn RfqSummaryReader,
    request: ListRfqSummariesRequest,
) -> Result<RfqSummaryPage, ListRfqSummariesError> {
    let context = build_context::build_context(request)?;
    load_context::load_context(reader, &context).await
}

#[cfg(test)]
mod reader_port_tests {
    use super::*;
    use crate::projections::rfq_summary_reader::{
        RfqSummary, RfqSummaryPage, RfqSummaryReadError, RfqSummaryReadFuture, RfqSummaryReader,
    };

    struct RecordingReader;

    impl RfqSummaryReader for RecordingReader {
        fn get(&self, _rfq_id: uuid::Uuid) -> RfqSummaryReadFuture<'_, Option<RfqSummary>> {
            Box::pin(async { Ok(None) })
        }

        fn list<'a>(
            &'a self,
            status: Option<&'a str>,
            limit: i64,
            offset: i64,
        ) -> RfqSummaryReadFuture<'a, RfqSummaryPage> {
            let status = status.map(str::to_owned);
            Box::pin(async move {
                if status.as_deref() != Some("open") {
                    return Err(RfqSummaryReadError::new(
                        "normalized status was not forwarded",
                    ));
                }
                Ok(RfqSummaryPage {
                    items: vec![],
                    total: 23,
                    limit,
                    offset,
                })
            })
        }
    }

    #[tokio::test]
    async fn valid_list_request_uses_reader_port_with_normalized_pagination() {
        let page = process(
            &RecordingReader,
            ListRfqSummariesRequest {
                status: Some("open".to_owned()),
                limit: Some(17),
                offset: Some(4),
            },
        )
        .await
        .expect("valid query should be executed through the reader port");
        assert_eq!(page.total, 23);
        assert_eq!(page.limit, 17);
        assert_eq!(page.offset, 4);
        assert!(page.items.is_empty());
    }
}
