//! SQL owned by the ListSupplierQuotes query capability.
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::SupplierQuoteListing;

pub async fn list_quotes(
    pool: &sqlx::PgPool,
    rfq_id: Uuid,
) -> Result<Option<Vec<SupplierQuoteListing>>, sqlx::Error> {
    let exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM rfqs WHERE id = $1)")
        .bind(rfq_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Ok(None);
    }

    let rows = sqlx::query(
        "SELECT id, rfq_id, supplier_id, currency, total_amount_cents, delivery_days, valid_until, submitted_at FROM supplier_quotes WHERE rfq_id = $1 ORDER BY submitted_at, id",
    )
    .bind(rfq_id)
    .fetch_all(pool)
    .await?;
    let mut quotes = Vec::with_capacity(rows.len());
    for row in rows {
        quotes.push(SupplierQuoteListing {
            id: row.try_get("id")?,
            rfq_id: row.try_get("rfq_id")?,
            supplier_id: row.try_get("supplier_id")?,
            currency: row.try_get("currency")?,
            total_amount_cents: row.try_get("total_amount_cents")?,
            delivery_days: row.try_get("delivery_days")?,
            valid_until: row.try_get::<NaiveDate, _>("valid_until")?,
            submitted_at: row.try_get::<DateTime<Utc>, _>("submitted_at")?,
        });
    }
    Ok(Some(quotes))
}
