//! PostgreSQL read projection for RFQ list/detail queries.
//!
//! FACTSTR remains authoritative. This projection is derived and can be rebuilt from
//! the durable `rfq-summary-projection-v1` stream. Batch writes are idempotent because
//! each RFQ row only applies events with a greater per-RFQ event sequence.

mod apply_events;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use factstr::{
    DurableStream, EventFilter, EventQuery, EventStore, EventStoreError, HandleStream,
    StreamHandlerError,
};
use factstr_postgres::PostgresStore;
use sqlx::{FromRow, PgPool, postgres::PgPoolOptions};
use thiserror::Error;
use uuid::Uuid;

use crate::events::{RFQ_CLOSED_V1, RFQ_OPENED_V1, SUPPLIER_QUOTE_SUBMITTED_V1};

const DURABLE_STREAM_ID: &str = "rfq-summary-projection-v1";
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

pub use crate::projections::rfq_summary_reader::{RfqSummary, RfqSummaryPage};
use crate::projections::rfq_summary_reader::{
    RfqSummaryReadError, RfqSummaryReadFuture, RfqSummaryReader,
};

#[derive(Debug, Clone, FromRow)]
struct RfqSummaryRow {
    rfq_id: Uuid,
    reference: String,
    currency: String,
    status: String,
    opened_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    item_count: i32,
    quote_count: i64,
    lowest_quote_amount_cents: Option<i64>,
    lowest_quote_supplier_id: Option<Uuid>,
    last_event_sequence: i64,
    projected_at: DateTime<Utc>,
}

impl From<RfqSummaryRow> for RfqSummary {
    fn from(row: RfqSummaryRow) -> Self {
        Self {
            rfq_id: row.rfq_id,
            reference: row.reference,
            currency: row.currency,
            status: row.status,
            opened_at: row.opened_at,
            closed_at: row.closed_at,
            item_count: row.item_count,
            quote_count: row.quote_count,
            lowest_quote_amount_cents: row.lowest_quote_amount_cents,
            lowest_quote_supplier_id: row.lowest_quote_supplier_id,
            last_event_sequence: row.last_event_sequence,
            projected_at: row.projected_at,
        }
    }
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
    read_pool: PgPool,
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
        // Keep HTTP reads independent from background projection writes.
        let read_pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(database_url)
            .await?;
        MIGRATOR.run(&read_pool).await?;

        // The durable stream uses a separate, bounded pool. A slow event
        // handler must not consume the connections reserved for API reads.
        let handler_pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(database_url)
            .await?;
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
                        match apply_events::apply_event_batch(&pool, records).await {
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
            read_pool,
            _stream: stream,
        })
    }

    pub async fn get(&self, rfq_id: Uuid) -> Result<Option<RfqSummary>, sqlx::Error> {
        sqlx::query_as::<_, RfqSummaryRow>(
            r#"SELECT rfq_id, reference, currency, status, opened_at, closed_at,
                      item_count, quote_count, lowest_quote_amount_cents,
                      lowest_quote_supplier_id, last_event_sequence, projected_at
               FROM rfq_summary_projection
               WHERE rfq_id = $1"#,
        )
        .bind(rfq_id)
        .fetch_optional(&self.read_pool)
        .await
        .map(|row| row.map(Into::into))
    }

    pub async fn list(
        &self,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<RfqSummaryPage, sqlx::Error> {
        // Keep total and page rows from the same projection snapshot while consumers update it.
        let mut transaction = self.read_pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *transaction)
            .await?;

        let total = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM rfq_summary_projection WHERE ($1::TEXT IS NULL OR status = $1)",
        )
        .bind(status)
        .fetch_one(&mut *transaction)
        .await?;

        let items = sqlx::query_as::<_, RfqSummaryRow>(
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
        .await?
        .into_iter()
        .map(Into::into)
        .collect::<Vec<RfqSummary>>();
        transaction.commit().await?;

        Ok(RfqSummaryPage {
            items,
            total,
            limit,
            offset,
        })
    }
}

/// Adaptador PostgreSQL/SQLx para a porta de leitura da aplicação.
impl RfqSummaryReader for RfqSummaryProjection {
    fn get(&self, rfq_id: Uuid) -> RfqSummaryReadFuture<'_, Option<RfqSummary>> {
        Box::pin(async move {
            RfqSummaryProjection::get(self, rfq_id)
                .await
                .map_err(|error| RfqSummaryReadError::new(error.to_string()))
        })
    }

    fn list<'a>(
        &'a self,
        status: Option<&'a str>,
        limit: i64,
        offset: i64,
    ) -> RfqSummaryReadFuture<'a, RfqSummaryPage> {
        Box::pin(async move {
            RfqSummaryProjection::list(self, status, limit, offset)
                .await
                .map_err(|error| RfqSummaryReadError::new(error.to_string()))
        })
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

    use crate::events::{
        QuotedItemFact, RFQ_OPENED_V1, RfqItemFact, RfqOpenedFact, SUPPLIER_QUOTE_SUBMITTED_V1,
        SupplierQuoteSubmittedFact,
    };

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
        apply_events::apply_event_batch(&pool, records.clone())
            .await
            .expect("first projection application must succeed");
        apply_events::apply_event_batch(&pool, records)
            .await
            .expect("replayed projection batch must succeed idempotently");

        let summary = sqlx::query_as::<_, RfqSummaryRow>(
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

    #[tokio::test]
    async fn failed_projection_batch_rolls_back_earlier_events() {
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
            .expect("mapped port must exist");
        let database_url = format!("postgres://rfq_user:rfq_password@{host}:{port}/supplier_rfq");

        let store_url = database_url.clone();
        let store = tokio::task::spawn_blocking(move || PostgresStore::connect(&store_url))
            .await
            .expect("FACTSTR connection task must complete")
            .expect("FACTSTR schema must initialize");

        let rfq_that_must_rollback = Uuid::from_u128(1701);
        let missing_rfq = Uuid::from_u128(1702);
        let opened = RfqOpenedFact {
            rfq_id: rfq_that_must_rollback,
            reference: "RFQ-PROJECTION-ROLLBACK".to_owned(),
            currency: "BRL".to_owned(),
            opened_at: Utc::now(),
            items: vec![RfqItemFact {
                id: Uuid::from_u128(1703),
                sku: "SKU-ROLLBACK".to_owned(),
                description: "Valid first event in a failing batch".to_owned(),
                required_quantity: 1,
            }],
        };
        let quote_for_missing_rfq = SupplierQuoteSubmittedFact {
            quote_id: Uuid::from_u128(1704),
            rfq_id: missing_rfq,
            supplier_id: Uuid::from_u128(1705),
            currency: "BRL".to_owned(),
            total_amount_cents: 500,
            delivery_days: 3,
            valid_until: NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
            submitted_at: Utc::now(),
            items: vec![QuotedItemFact {
                rfq_item_id: Uuid::from_u128(1706),
                quantity: 1,
                unit_price_cents: 500,
                line_total_cents: 500,
            }],
        };
        let open_payload = serde_json::to_value(opened).unwrap();
        let quote_payload = serde_json::to_value(quote_for_missing_rfq).unwrap();
        let append_store = Arc::new(store);
        let append_store_thread = Arc::clone(&append_store);
        tokio::task::spawn_blocking(move || {
            append_store_thread
                .append(vec![
                    NewEvent::new(RFQ_OPENED_V1, open_payload),
                    NewEvent::new(SUPPLIER_QUOTE_SUBMITTED_V1, quote_payload),
                ])
                .expect("test batch must append to the authoritative store");
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
        assert_eq!(records.len(), 2);

        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("projection pool must connect");
        MIGRATOR
            .run(&pool)
            .await
            .expect("projection migration must run");

        let error = apply_events::apply_event_batch(&pool, records)
            .await
            .expect_err("quote referring to an RFQ absent from the projection must fail");
        assert!(matches!(
            error,
            RfqSummaryProjectionError::MissingRfq { rfq_id, .. } if rfq_id == missing_rfq
        ));

        // The first RFQ_OPENED_V1 in the same transaction must have rolled back too.
        let persisted = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM rfq_summary_projection WHERE rfq_id = $1",
        )
        .bind(rfq_that_must_rollback)
        .fetch_one(&pool)
        .await
        .expect("projection row count must be queryable after rollback");
        assert_eq!(
            persisted, 0,
            "a failed batch must not partially persist earlier events"
        );

        drop(pool);
        drop(append_store);
        drop(container);
    }
}
