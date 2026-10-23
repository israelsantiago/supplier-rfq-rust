//! Capability-owned SQL. The RPU owns the transaction boundary; this module does not begin or commit transactions.
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use super::{
    context::{
        RequiredItem, RfqStatus, RfqSubmissionContext, SubmitSupplierQuoteContext, SupplierStatus,
    },
    result::QuoteSubmitted,
};

#[derive(Debug, thiserror::Error)]
pub enum StateAccessError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid stored value for {field}: {value}")]
    InvalidStoredValue { field: &'static str, value: String },
}

/// Load facts needed for this command, locking them until the RPU commits or rolls back.
/// Lock order is RFQ -> supplier -> RFQ items. Other command RPUs touching these facts
/// must follow a compatible order to reduce deadlock risk.
pub async fn load_submission_context(
    tx: &mut Transaction<'_, Postgres>,
    rfq_id: Uuid,
    supplier_id: Uuid,
) -> Result<SubmitSupplierQuoteContext, StateAccessError> {
    let rfq_row = sqlx::query("SELECT id, status, currency FROM rfqs WHERE id = $1 FOR SHARE")
        .bind(rfq_id)
        .fetch_optional(&mut **tx)
        .await?;

    let rfq_header = if let Some(row) = rfq_row {
        let status_text: String = row.try_get("status")?;
        let status = match status_text.as_str() {
            "draft" => RfqStatus::Draft,
            "open" => RfqStatus::Open,
            "closed" => RfqStatus::Closed,
            _ => {
                return Err(StateAccessError::InvalidStoredValue {
                    field: "rfqs.status",
                    value: status_text,
                });
            }
        };
        let id: Uuid = row.try_get("id")?;
        let currency: String = row.try_get("currency")?;
        Some((id, status, currency))
    } else {
        None
    };

    // Keep a deterministic lock order: RFQ row, supplier row, then RFQ item rows.
    let supplier_row = sqlx::query("SELECT status FROM suppliers WHERE id = $1 FOR SHARE")
        .bind(supplier_id)
        .fetch_optional(&mut **tx)
        .await?;
    let supplier_status = match supplier_row {
        None => None,
        Some(row) => {
            let status_text: String = row.try_get("status")?;
            Some(match status_text.as_str() {
                "eligible" => SupplierStatus::Eligible,
                "blocked" => SupplierStatus::Blocked,
                _ => {
                    return Err(StateAccessError::InvalidStoredValue {
                        field: "suppliers.status",
                        value: status_text,
                    });
                }
            })
        }
    };

    let rfq = if let Some((id, status, currency)) = rfq_header {
        let item_rows = sqlx::query(
            "SELECT id, required_quantity FROM rfq_items WHERE rfq_id = $1 ORDER BY id FOR SHARE",
        )
        .bind(rfq_id)
        .fetch_all(&mut **tx)
        .await?;
        let mut required_items = Vec::with_capacity(item_rows.len());
        for item_row in item_rows {
            required_items.push(RequiredItem {
                id: item_row.try_get("id")?,
                required_quantity: item_row.try_get("required_quantity")?,
            });
        }
        Some(RfqSubmissionContext {
            id,
            status,
            currency,
            required_items,
        })
    } else {
        None
    };

    Ok(SubmitSupplierQuoteContext {
        rfq,
        supplier_status,
    })
}

pub async fn record_quote(
    tx: &mut Transaction<'_, Postgres>,
    quote: &QuoteSubmitted,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO supplier_quotes (id, rfq_id, supplier_id, currency, total_amount_cents, delivery_days, valid_until, submitted_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(quote.id)
    .bind(quote.rfq_id)
    .bind(quote.supplier_id)
    .bind(&quote.currency)
    .bind(quote.total_amount_cents)
    .bind(quote.delivery_days)
    .bind(quote.valid_until)
    .bind(quote.submitted_at)
    .execute(&mut **tx)
    .await?;

    for (item, line_total_cents) in quote.items.iter().zip(&quote.line_totals_cents) {
        sqlx::query(
            "INSERT INTO supplier_quote_items (quote_id, rfq_id, rfq_item_id, quantity, unit_price_cents, line_total_cents) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(quote.id)
        .bind(quote.rfq_id)
        .bind(item.rfq_item_id)
        .bind(item.quantity)
        .bind(item.unit_price_cents)
        .bind(line_total_cents)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
