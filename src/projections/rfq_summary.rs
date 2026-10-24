//! PostgreSQL read projection for RFQ list/detail queries.
//!
//! FACTSTR remains authoritative. This projection is derived and can be rebuilt from
//! the durable `rfq-summary-projection-v1` stream. Batch writes are idempotent because
//! each RFQ row only applies events with a greater per-RFQ event sequence.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use factstr::{
    DurableStream, EventFilter, EventQuery, EventRecord, EventStore, EventStoreError, HandleStream,
    StreamHandlerError,
};
use factstr_postgres::PostgresStore;
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgPool, postgres::PgPoolOptions};
use thiserror::Error;
use uuid::Uuid;

use crate::facts::{
    RFQ_CLOSED_V1, RFQ_OPENED_V1, RfqClosedFact, RfqOpenedFact, SUPPLIER_QUOTE_SUBMITTED_V1,
    SupplierQuoteSubmittedFact,
};

const DURABLE_STREAM_ID: &str = "rfq-summary-projection-v1";
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RfqSummary {
    pub rfq_id: Uuid,
    pub reference: String,
    pub currency: String,
    pub status: String,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub item_count: i32,
    pub quote_count: i64,
    pub lowest_quote_amount_cents: Option<i64>,
    pub lowest_quote_supplier_id: Option<Uuid>,
    pub last_event_sequence: i64,
    pub projected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RfqSummaryPage {
    pub items: Vec<RfqSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Error)]
pub enum RfqSummaryProjectionError {
    #[error("PostgreSQL projection error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("PostgreSQL projection migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("FACTSTR stream subscription failed: {0}")]
    Store(#[from] EventStoreError),
    #[error("FACTSTR stream registration task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("invalid {event_type} fact payload: {message}")]
    InvalidFact {
        event_type: &'static str,
        message: String,
    },
    #[error("event sequence {sequence_number} cannot be represented by PostgreSQL BIGINT")]
    SequenceOutOfRange { sequence_number: u64 },
    #[error(
        "projection is missing RFQ {rfq_id} while applying {event_type} at sequence {sequence_number}; replay must start from RFQ_OPENED_V1"
    )]
    MissingRfq {
        rfq_id: Uuid,
        event_type: &'static str,
        sequence_number: u64,
    },
    #[error(
        "projection row for RFQ {rfq_id} did not advance to event sequence {sequence_number} as expected"
    )]
    UnexpectedSequence { rfq_id: Uuid, sequence_number: u64 },
    #[error("RFQ item count exceeds PostgreSQL INTEGER")]
    ItemCountOutOfRange,
}

/// Holds the PostgreSQL read model and the active durable FACTSTR subscription.
pub struct RfqSummaryProjection {
    pool: PgPool,
    // Keeping the stream alive keeps live event delivery registered. Drop unsubscribes.
    _stream: factstr::EventStream,
}

impl RfqSummaryProjection {
    /// Connect, apply application-owned migrations, and replay/subscribe durably.
    ///
    /// FACTSTR's synchronous stream registration is performed on a blocking thread;
    /// startup returns after the durable replay has caught up to the current history.
    pub async fn start(
        database_url: &str,
        store: Arc<PostgresStore>,
    ) -> Result<Self, RfqSummaryProjectionError> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(database_url)
            .await?;
        MIGRATOR.run(&pool).await?;

        let handler_pool = pool.clone();
        let stream = tokio::task::spawn_blocking(move || {
            let query = EventQuery::all().with_filters([
                EventFilter::for_event_types([RFQ_OPENED_V1]),
                EventFilter::for_event_types([RFQ_CLOSED_V1]),
                EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1]),
            ]);
            let durable_stream = DurableStream::new(DURABLE_STREAM_ID);
            store.stream_to_durable(
                &durable_stream,
                &query,
                HandleStream::new(move |records| {
                    let pool = handler_pool.clone();
                    async move {
                        match apply_event_batch(&pool, records).await {
                            Ok(()) => Ok(()),
                            Err(error) => {
                                tracing::error!(
                                    error = %error,
                                    "RFQ summary projection batch failed; durable cursor was not advanced"
                                );
                                Err(StreamHandlerError::new(error.to_string()))
                            }
                        }
                    }
                }),
            )
        })
        .await??;

        Ok(Self {
            pool,
            _stream: stream,
        })
    }

    pub async fn get(&self, rfq_id: Uuid) -> Result<Option<RfqSummary>, sqlx::Error> {
        sqlx::query_as::<_, RfqSummary>(
            r#"SELECT rfq_id, reference, currency, status, opened_at, closed_at,
                      item_count, quote_count, lowest_quote_amount_cents,
                      lowest_quote_supplier_id, last_event_sequence, projected_at
               FROM rfq_summary_projection
               WHERE rfq_id = $1"#,
        )
        .bind(rfq_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list(
        &self,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<RfqSummaryPage, sqlx::Error> {
        // Keep total and page rows from the same projection snapshot while consumers update it.
        let mut transaction = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *transaction)
            .await?;

        let total = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM rfq_summary_projection WHERE ($1::TEXT IS NULL OR status = $1)",
        )
        .bind(status)
        .fetch_one(&mut *transaction)
        .await?;

        let items = sqlx::query_as::<_, RfqSummary>(
            r#"SELECT rfq_id, reference, currency, status, opened_at, closed_at,
                      item_count, quote_count, lowest_quote_amount_cents,
                      lowest_quote_supplier_id, last_event_sequence, projected_at
               FROM rfq_summary_projection
               WHERE ($1::TEXT IS NULL OR status = $1)
               ORDER BY opened_at DESC, rfq_id
               LIMIT $2 OFFSET $3"#,
        )
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;

        Ok(RfqSummaryPage {
            items,
            total,
            limit,
            offset,
        })
    }
}

async fn apply_event_batch(
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use factstr::{EventStore, NewEvent};
    use factstr_postgres::PostgresStore;
    use testcontainers_modules::{
        postgres::Postgres,
        testcontainers::{ImageExt, runners::AsyncRunner},
    };

    use crate::facts::{QuotedItemFact, RFQ_OPENED_V1, RfqItemFact};

    #[tokio::test]
    async fn replaying_the_same_fact_batch_does_not_double_count_quotes() {
        let container = Postgres::default()
            .with_db_name("supplier_rfq")
            .with_user("rfq_user")
            .with_password("rfq_password")
            .with_tag("16-alpine")
            .start()
            .await
            .expect("PostgreSQL container must start; Docker must be available");
        let host = container
            .get_host()
            .await
            .expect("container host must exist");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("PostgreSQL port must be mapped");
        let database_url = format!("postgres://rfq_user:rfq_password@{host}:{port}/supplier_rfq");

        let store_url = database_url.clone();
        let store = tokio::task::spawn_blocking(move || PostgresStore::connect(&store_url))
            .await
            .expect("FACTSTR connection task must complete")
            .expect("FACTSTR schema must initialize");
        let rfq_id = Uuid::new_v4();
        let supplier_id = Uuid::new_v4();
        let opened = RfqOpenedFact {
            rfq_id,
            reference: "RFQ-PROJECTION-IDEMPOTENCY".to_owned(),
            currency: "BRL".to_owned(),
            opened_at: Utc::now(),
            items: vec![RfqItemFact {
                id: Uuid::new_v4(),
                sku: "SKU-1".to_owned(),
                description: "Item 1".to_owned(),
                required_quantity: 1,
            }],
        };
        let quote = SupplierQuoteSubmittedFact {
            quote_id: Uuid::new_v4(),
            rfq_id,
            supplier_id,
            currency: "BRL".to_owned(),
            total_amount_cents: 1250,
            delivery_days: 7,
            valid_until: NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
            submitted_at: Utc::now(),
            items: vec![QuotedItemFact {
                rfq_item_id: opened.items[0].id,
                quantity: 1,
                unit_price_cents: 1250,
                line_total_cents: 1250,
            }],
        };
        let open_payload = serde_json::to_value(opened).unwrap();
        let quote_payload = serde_json::to_value(quote).unwrap();
        let append_store = Arc::new(store);
        let append_store_for_thread = Arc::clone(&append_store);
        tokio::task::spawn_blocking(move || {
            append_store_for_thread
                .append(vec![
                    NewEvent::new(RFQ_OPENED_V1, open_payload),
                    NewEvent::new(SUPPLIER_QUOTE_SUBMITTED_V1, quote_payload),
                ])
                .expect("test facts must append");
        })
        .await
        .expect("append task must complete");

        let query_store = Arc::clone(&append_store);
        let records = tokio::task::spawn_blocking(move || {
            let query = EventQuery::all().with_filters([
                EventFilter::for_event_types([RFQ_OPENED_V1]),
                EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1]),
            ]);
            query_store
                .query(&query)
                .expect("facts must be queryable")
                .event_records
        })
        .await
        .expect("query task must complete");

        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("projection pool must connect");
        MIGRATOR
            .run(&pool)
            .await
            .expect("projection migration must run");
        apply_event_batch(&pool, records.clone())
            .await
            .expect("first projection application must succeed");
        apply_event_batch(&pool, records)
            .await
            .expect("replayed projection batch must succeed idempotently");

        let summary = sqlx::query_as::<_, RfqSummary>(
            "SELECT rfq_id, reference, currency, status, opened_at, closed_at, item_count, quote_count, lowest_quote_amount_cents, lowest_quote_supplier_id, last_event_sequence, projected_at FROM rfq_summary_projection WHERE rfq_id = $1",
        )
        .bind(rfq_id)
        .fetch_one(&pool)
        .await
        .expect("projection row must exist");
        assert_eq!(summary.quote_count, 1);
        assert_eq!(summary.lowest_quote_amount_cents, Some(1250));
        drop(pool);
        drop(append_store);
        drop(container);
    }
}
