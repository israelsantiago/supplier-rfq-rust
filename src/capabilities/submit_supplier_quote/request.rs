use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitSupplierQuoteRequest {
    pub supplier_id: Uuid,
    pub currency: String,
    pub delivery_days: i32,
    pub valid_until: NaiveDate,
    pub items: Vec<QuotedItemInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotedItemInput {
    pub rfq_item_id: Uuid,
    pub quantity: i64,
    pub unit_price_cents: i64,
}
