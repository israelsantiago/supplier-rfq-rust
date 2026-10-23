use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RfqStatus {
    Draft,
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplierStatus {
    Eligible,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredItem {
    pub id: Uuid,
    pub required_quantity: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RfqSubmissionContext {
    pub id: Uuid,
    pub status: RfqStatus,
    pub currency: String,
    pub required_items: Vec<RequiredItem>,
}

/// Snapshot of the facts used by the decision. The RPU loads this inside a transaction
/// while protecting the rows that represent the command's consistency context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitSupplierQuoteContext {
    pub rfq: Option<RfqSubmissionContext>,
    pub supplier_status: Option<SupplierStatus>,
}
