//! Aplicação transacional dos lotes recebidos da assinatura FACTSTR.

use factstr::EventRecord;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::events::{
    RFQ_CLOSED_V1, RFQ_OPENED_V1, RfqClosedFact, RfqOpenedFact, SUPPLIER_QUOTE_SUBMITTED_V1,
    SupplierQuoteSubmittedFact,
};

use super::RfqSummaryProjectionError;

pub(super) async fn apply_event_batch(
    pool: &PgPool,
    records: Vec<EventRecord>,
) -> Result<(), RfqSummaryProjectionError> {
    if records.is_empty() {
        return Ok(());
    }

    // The projection rows for every event in a committed FACTSTR batch update atomically.
    // If PostgreSQL fails, the handler returns Err and FACTSTR must not advance its cursor.
    let mut transaction = pool.begin().await?;
    for record in records {
        let sequence_number = i64::try_from(record.sequence_number).map_err(|_| {
            RfqSummaryProjectionError::SequenceOutOfRange {
                sequence_number: record.sequence_number,
            }
        })?;

        match record.event_type.as_str() {
            RFQ_OPENED_V1 => {
                let fact: RfqOpenedFact = decode_fact(RFQ_OPENED_V1, record.payload)?;
                let item_count = i32::try_from(fact.items.len())
                    .map_err(|_| RfqSummaryProjectionError::ItemCountOutOfRange)?;
                sqlx::query(
                    r#"INSERT INTO rfq_summary_projection (
                           rfq_id, reference, currency, status, opened_at, closed_at,
                           item_count, quote_count, lowest_quote_amount_cents,
                           lowest_quote_supplier_id, last_event_sequence, projected_at
                       ) VALUES ($1, $2, $3, 'open', $4, NULL, $5, 0, NULL, NULL, $6, now())
                       ON CONFLICT (rfq_id) DO UPDATE SET
                           reference = EXCLUDED.reference,
                           currency = EXCLUDED.currency,
                           status = 'open',
                           opened_at = EXCLUDED.opened_at,
                           closed_at = NULL,
                           item_count = EXCLUDED.item_count,
                           quote_count = 0,
                           lowest_quote_amount_cents = NULL,
                           lowest_quote_supplier_id = NULL,
                           last_event_sequence = EXCLUDED.last_event_sequence,
                           projected_at = now()
                       WHERE rfq_summary_projection.last_event_sequence < EXCLUDED.last_event_sequence"#,
                )
                .bind(fact.rfq_id)
                .bind(fact.reference)
                .bind(fact.currency)
                .bind(fact.opened_at)
                .bind(item_count)
                .bind(sequence_number)
                .execute(&mut *transaction)
                .await?;
            }
            RFQ_CLOSED_V1 => {
                let fact: RfqClosedFact = decode_fact(RFQ_CLOSED_V1, record.payload)?;
                let result = sqlx::query(
                    r#"UPDATE rfq_summary_projection
                       SET status = 'closed', closed_at = $2,
                           last_event_sequence = $3, projected_at = now()
                       WHERE rfq_id = $1 AND last_event_sequence < $3"#,
                )
                .bind(fact.rfq_id)
                .bind(fact.closed_at)
                .bind(sequence_number)
                .execute(&mut *transaction)
                .await?;
                if result.rows_affected() == 0 {
                    ensure_replay_is_idempotent_or_fail(
                        &mut transaction,
                        fact.rfq_id,
                        sequence_number,
                        RFQ_CLOSED_V1,
                        record.sequence_number,
                    )
                    .await?;
                }
            }
            SUPPLIER_QUOTE_SUBMITTED_V1 => {
                let fact: SupplierQuoteSubmittedFact =
                    decode_fact(SUPPLIER_QUOTE_SUBMITTED_V1, record.payload)?;
                let result = sqlx::query(
                    r#"UPDATE rfq_summary_projection
                       SET quote_count = quote_count + 1,
                           lowest_quote_supplier_id = CASE
                               WHEN lowest_quote_amount_cents IS NULL
                                    OR $2 < lowest_quote_amount_cents
                               THEN $3 ELSE lowest_quote_supplier_id END,
                           lowest_quote_amount_cents = CASE
                               WHEN lowest_quote_amount_cents IS NULL
                                    OR $2 < lowest_quote_amount_cents
                               THEN $2 ELSE lowest_quote_amount_cents END,
                           last_event_sequence = $4,
                           projected_at = now()
                       WHERE rfq_id = $1 AND last_event_sequence < $4"#,
                )
                .bind(fact.rfq_id)
                .bind(fact.total_amount_cents)
                .bind(fact.supplier_id)
                .bind(sequence_number)
                .execute(&mut *transaction)
                .await?;
                if result.rows_affected() == 0 {
                    ensure_replay_is_idempotent_or_fail(
                        &mut transaction,
                        fact.rfq_id,
                        sequence_number,
                        SUPPLIER_QUOTE_SUBMITTED_V1,
                        record.sequence_number,
                    )
                    .await?;
                }
            }
            // Other facts don't affect the RFQ summary. The durable stream is filtered,
            // but ignoring unknown records is forward-compatible if filtering evolves.
            _ => {}
        }
    }
    transaction.commit().await?;
    Ok(())
}

fn decode_fact<T: serde::de::DeserializeOwned>(
    event_type: &'static str,
    payload: Value,
) -> Result<T, RfqSummaryProjectionError> {
    serde_json::from_value(payload).map_err(|error| RfqSummaryProjectionError::InvalidFact {
        event_type,
        message: error.to_string(),
    })
}

async fn ensure_replay_is_idempotent_or_fail(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rfq_id: Uuid,
    sequence_number: i64,
    event_type: &'static str,
    original_sequence_number: u64,
) -> Result<(), RfqSummaryProjectionError> {
    let current_sequence = sqlx::query_scalar::<_, i64>(
        "SELECT last_event_sequence FROM rfq_summary_projection WHERE rfq_id = $1",
    )
    .bind(rfq_id)
    .fetch_optional(&mut **transaction)
    .await?;

    match current_sequence {
        Some(current) if current >= sequence_number => Ok(()),
        None => Err(RfqSummaryProjectionError::MissingRfq {
            rfq_id,
            event_type,
            sequence_number: original_sequence_number,
        }),
        Some(_) => Err(RfqSummaryProjectionError::UnexpectedSequence {
            rfq_id,
            sequence_number: original_sequence_number,
        }),
    }
}
