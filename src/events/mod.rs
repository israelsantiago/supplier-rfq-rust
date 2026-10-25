//! Versioned fact names and payload contracts. Facts are append-only records;
//! new business information is represented by new facts, never by rewriting old ones.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const RFQ_OPENED_V1: &str = "rfq_opened.v1";
pub const RFQ_CLOSED_V1: &str = "rfq_closed.v1";
pub const SUPPLIER_REGISTERED_V1: &str = "supplier_registered.v1";
pub const SUPPLIER_ELIGIBILITY_CHANGED_V1: &str = "supplier_eligibility_changed.v1";
pub const SUPPLIER_QUOTE_SUBMITTED_V1: &str = "supplier_quote_submitted.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplierStatus {
    Eligible,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RfqItemFact {
    pub id: Uuid,
    pub sku: String,
    pub description: String,
    pub required_quantity: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfqOpenedFact {
    pub rfq_id: Uuid,
    pub reference: String,
    pub currency: String,
    pub opened_at: DateTime<Utc>,
    pub items: Vec<RfqItemFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfqClosedFact {
    pub rfq_id: Uuid,
    pub closed_at: DateTime<Utc>,
}

/// Identity is registered once; the natural reference does not change in this example.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplierRegisteredFact {
    pub supplier_id: Uuid,
    pub reference: String,
    pub name: String,
    pub registered_at: DateTime<Utc>,
}

/// Eligibility changes contain only the information that changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplierEligibilityChangedFact {
    pub supplier_id: Uuid,
    pub status: SupplierStatus,
    pub changed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotedItemFact {
    pub rfq_item_id: Uuid,
    pub quantity: i64,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplierQuoteSubmittedFact {
    pub quote_id: Uuid,
    pub rfq_id: Uuid,
    pub supplier_id: Uuid,
    pub currency: String,
    pub total_amount_cents: i64,
    pub delivery_days: i32,
    pub valid_until: NaiveDate,
    pub submitted_at: DateTime<Utc>,
    pub items: Vec<QuotedItemFact>,
}
