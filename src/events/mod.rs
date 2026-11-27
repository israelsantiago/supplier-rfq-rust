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
pub const RFQ_AWARDED_V1: &str = "rfq_awarded.v1";
pub const PURCHASE_ORDER_CREATED_V1: &str = "purchase_order_created.v1";
pub const PURCHASE_ORDER_DISPATCH_REQUESTED_V1: &str = "purchase_order_dispatch_requested.v1";

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

/// Buyer-approved award; immutable audit fact for the selected quote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RfqAwardedFact {
    pub award_id: Uuid,
    pub rfq_id: Uuid,
    pub quote_id: Uuid,
    pub supplier_id: Uuid,
    pub awarded_at: DateTime<Utc>,
    pub currency: String,
    pub total_amount_cents: i64,
    pub delivery_days: i32,
    pub valid_until: NaiveDate,
}

/// Purchase-order intent recorded with the award in the same atomic event batch.
/// This is a domain record, not proof that an external ERP accepted the order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurchaseOrderCreatedFact {
    pub purchase_order_id: Uuid,
    pub rfq_id: Uuid,
    pub supplier_id: Uuid,
    pub source_quote_id: Uuid,
    pub currency: String,
    pub total_amount_cents: i64,
    pub delivery_days: i32,
    pub created_at: DateTime<Utc>,
    pub items: Vec<QuotedItemFact>,
}

/// Durable integration intent. A downstream dispatcher publishes it after the commit;
/// retries/deduplication are required because the external system is outside FACTSTR's tx.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurchaseOrderDispatchRequestedFact {
    pub dispatch_id: Uuid,
    pub purchase_order_id: Uuid,
    pub rfq_id: Uuid,
    pub requested_at: DateTime<Utc>,
}

pub const PROCUREMENT_BUDGET_ALLOCATED_V1: &str = "procurement_budget_allocated.v1";
pub const PROCUREMENT_BUDGET_RESERVED_V1: &str = "procurement_budget_reserved.v1";
pub const PURCHASE_ORDER_CREATED_V2: &str = "purchase_order_created.v2";

/// Proposed input contract for the new budget-reservation capability.
/// Ownership/source of this fact must be approved by the Financial/Procurement domain owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcurementBudgetAllocatedFact {
    pub budget_id: Uuid,
    pub currency: String,
    pub allocated_amount_cents: i64,
    pub allocated_at: DateTime<Utc>,
}

/// Durable reservation fact proposed by the Reserve Procurement Budget capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcurementBudgetReservedFact {
    pub reservation_id: Uuid,
    pub budget_id: Uuid,
    pub rfq_id: Uuid,
    pub supplier_id: Uuid,
    pub currency: String,
    pub reserved_amount_cents: i64,
    pub reserved_at: DateTime<Utc>,
}

/// New PO schema. The legacy purchase_order_created.v1 contract is not modified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurchaseOrderCreatedV2Fact {
    pub purchase_order_id: Uuid,
    pub rfq_id: Uuid,
    pub supplier_id: Uuid,
    pub source_quote_id: Uuid,
    pub budget_id: Uuid,
    pub budget_reservation_id: Uuid,
    pub currency: String,
    pub total_amount_cents: i64,
    pub delivery_days: i32,
    pub created_at: DateTime<Utc>,
    pub items: Vec<QuotedItemFact>,
}
