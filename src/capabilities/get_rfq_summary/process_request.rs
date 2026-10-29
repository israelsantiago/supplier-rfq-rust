use thiserror::Error;
use uuid::Uuid;

use crate::projections::rfq_summary_reader::{RfqSummary, RfqSummaryReader};

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
    reader: &dyn RfqSummaryReader,
    request: GetRfqSummaryRequest,
) -> Result<RfqSummary, GetRfqSummaryError> {
    match load_context::load_context(reader, request.rfq_id).await {
        Ok(Some(summary)) => Ok(summary),
        Ok(None) => Err(GetRfqSummaryError::NotAvailableYet),
        Err(error) => Err(GetRfqSummaryError::Projection(error.to_string())),
    }
}

#[cfg(test)]
mod reader_port_tests {
    use super::*;
    use crate::projections::rfq_summary_reader::{
        RfqSummaryPage, RfqSummaryReadError, RfqSummaryReadFuture, RfqSummaryReader,
    };
    use chrono::{TimeZone, Utc};

    struct MissingSummaryReader;

    impl RfqSummaryReader for MissingSummaryReader {
        fn get(&self, _rfq_id: Uuid) -> RfqSummaryReadFuture<'_, Option<RfqSummary>> {
            Box::pin(async { Ok(None) })
        }

        fn list<'a>(
            &'a self,
            _status: Option<&'a str>,
            limit: i64,
            offset: i64,
        ) -> RfqSummaryReadFuture<'a, RfqSummaryPage> {
            Box::pin(async move {
                Ok(RfqSummaryPage {
                    items: vec![],
                    total: 0,
                    limit,
                    offset,
                })
            })
        }
    }

    #[tokio::test]
    async fn missing_summary_keeps_existing_not_available_yet_result_with_fake_reader() {
        let error = process(
            &MissingSummaryReader,
            GetRfqSummaryRequest {
                rfq_id: Uuid::nil(),
            },
        )
        .await
        .expect_err("missing projection row should remain a not-available result");
        assert!(matches!(error, GetRfqSummaryError::NotAvailableYet));
    }

    fn sample_summary() -> RfqSummary {
        let timestamp = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
        RfqSummary {
            rfq_id: Uuid::from_u128(7),
            reference: "RFQ-PORT-001".to_owned(),
            currency: "BRL".to_owned(),
            status: "open".to_owned(),
            opened_at: timestamp,
            closed_at: None,
            item_count: 2,
            quote_count: 3,
            lowest_quote_amount_cents: Some(1700),
            lowest_quote_supplier_id: Some(Uuid::from_u128(8)),
            last_event_sequence: 10,
            projected_at: timestamp,
        }
    }

    struct PresentSummaryReader {
        summary: RfqSummary,
    }

    impl RfqSummaryReader for PresentSummaryReader {
        fn get(&self, _rfq_id: Uuid) -> RfqSummaryReadFuture<'_, Option<RfqSummary>> {
            let summary = self.summary.clone();
            Box::pin(async move { Ok(Some(summary)) })
        }

        fn list<'a>(
            &'a self,
            _status: Option<&'a str>,
            limit: i64,
            offset: i64,
        ) -> RfqSummaryReadFuture<'a, RfqSummaryPage> {
            Box::pin(async move {
                Ok(RfqSummaryPage {
                    items: vec![],
                    total: 0,
                    limit,
                    offset,
                })
            })
        }
    }

    #[tokio::test]
    async fn found_summary_is_returned_unchanged_through_reader_port() {
        let expected = sample_summary();
        let result = process(
            &PresentSummaryReader {
                summary: expected.clone(),
            },
            GetRfqSummaryRequest {
                rfq_id: expected.rfq_id,
            },
        )
        .await
        .expect("present summary should be returned");
        assert_eq!(result.rfq_id, expected.rfq_id);
        assert_eq!(result.reference, expected.reference);
        assert_eq!(result.status, expected.status);
        assert_eq!(result.quote_count, expected.quote_count);
        assert_eq!(
            result.lowest_quote_amount_cents,
            expected.lowest_quote_amount_cents
        );
        assert_eq!(result.last_event_sequence, expected.last_event_sequence);
    }

    struct FailedReader;

    impl RfqSummaryReader for FailedReader {
        fn get(&self, _rfq_id: Uuid) -> RfqSummaryReadFuture<'_, Option<RfqSummary>> {
            Box::pin(async { Err(RfqSummaryReadError::new("synthetic reader diagnostic")) })
        }

        fn list<'a>(
            &'a self,
            _status: Option<&'a str>,
            limit: i64,
            offset: i64,
        ) -> RfqSummaryReadFuture<'a, RfqSummaryPage> {
            Box::pin(async move {
                Ok(RfqSummaryPage {
                    items: vec![],
                    total: 0,
                    limit,
                    offset,
                })
            })
        }
    }

    #[tokio::test]
    async fn reader_failure_keeps_existing_projection_error_category() {
        let error = process(
            &FailedReader,
            GetRfqSummaryRequest {
                rfq_id: Uuid::nil(),
            },
        )
        .await
        .expect_err("reader failure should be mapped to the existing query error");
        assert!(
            matches!(error, GetRfqSummaryError::Projection(message) if message.contains("synthetic reader diagnostic"))
        );
    }
}
