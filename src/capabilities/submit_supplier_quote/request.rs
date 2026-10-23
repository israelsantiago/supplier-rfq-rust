use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Command request received by the Submit Supplier Quote capability.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitSupplierQuote {
    pub supplier_id: Uuid,
    pub currency: String,
    pub delivery_days: i32,
    pub valid_until: NaiveDate,
    pub items: Vec<QuotedItemInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuotedItemInput {
    pub rfq_item_id: Uuid,
    pub quantity: i64,
    /// Integer minor currency units (centavos for BRL); float money is intentionally avoided.
    pub unit_price_cents: i64,
}
