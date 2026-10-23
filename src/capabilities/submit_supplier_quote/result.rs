use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::request::QuotedItemInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteSubmitted {
    pub id: Uuid,
    pub rfq_id: Uuid,
    pub supplier_id: Uuid,
    pub currency: String,
    pub total_amount_cents: i64,
    pub delivery_days: i32,
    pub valid_until: NaiveDate,
    pub submitted_at: DateTime<Utc>,
    pub items: Vec<QuotedItemInput>,
    pub line_totals_cents: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionRejection {
    RfqNotFound,
    SupplierNotFound,
    RfqClosed,
    RfqNotOpen,
    SupplierNotEligible,
    CurrencyMismatch,
    InvalidDeliveryDays,
    QuoteValidityInPast,
    EmptyQuote,
    DuplicateRfqItem,
    ItemSetMismatch,
    QuantityMismatch,
    InvalidUnitPrice,
    AmountOverflow,
    DuplicateSupplierQuote,
}

impl SubmissionRejection {
    pub fn code(&self) -> &'static str {
        match self {
            Self::RfqNotFound => "rfq_not_found",
            Self::SupplierNotFound => "supplier_not_found",
            Self::RfqClosed => "rfq_closed",
            Self::RfqNotOpen => "rfq_not_open",
            Self::SupplierNotEligible => "supplier_not_eligible",
            Self::CurrencyMismatch => "currency_mismatch",
            Self::InvalidDeliveryDays => "invalid_delivery_days",
            Self::QuoteValidityInPast => "quote_validity_in_past",
            Self::EmptyQuote => "empty_quote",
            Self::DuplicateRfqItem => "duplicate_rfq_item",
            Self::ItemSetMismatch => "item_set_mismatch",
            Self::QuantityMismatch => "quantity_mismatch",
            Self::InvalidUnitPrice => "invalid_unit_price",
            Self::AmountOverflow => "amount_overflow",
            Self::DuplicateSupplierQuote => "duplicate_supplier_quote",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::RfqNotFound => "RFQ was not found.",
            Self::SupplierNotFound => "Supplier was not found.",
            Self::RfqClosed => "The RFQ is closed and no longer accepts quotes.",
            Self::RfqNotOpen => "The RFQ is not open for supplier quotes.",
            Self::SupplierNotEligible => "The supplier is not eligible to quote.",
            Self::CurrencyMismatch => "Quote currency must match the RFQ currency.",
            Self::InvalidDeliveryDays => "Delivery days must be greater than zero.",
            Self::QuoteValidityInPast => "Quote validity date cannot be in the past.",
            Self::EmptyQuote => "A quote must contain at least one item.",
            Self::DuplicateRfqItem => "An RFQ item can only appear once in a quote.",
            Self::ItemSetMismatch => "The quote must contain exactly the RFQ's requested items.",
            Self::QuantityMismatch => "Each quoted quantity must match the requested quantity.",
            Self::InvalidUnitPrice => "Unit prices must be greater than zero.",
            Self::AmountOverflow => "The quote total exceeds the supported integer range.",
            Self::DuplicateSupplierQuote => {
                "This supplier has already submitted a quote for this RFQ."
            }
        }
    }

    pub fn http_status(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::RfqNotFound | Self::SupplierNotFound => StatusCode::NOT_FOUND,
            Self::CurrencyMismatch
            | Self::InvalidDeliveryDays
            | Self::QuoteValidityInPast
            | Self::EmptyQuote
            | Self::DuplicateRfqItem
            | Self::ItemSetMismatch
            | Self::QuantityMismatch
            | Self::InvalidUnitPrice
            | Self::AmountOverflow => StatusCode::UNPROCESSABLE_ENTITY,
            Self::RfqClosed
            | Self::RfqNotOpen
            | Self::SupplierNotEligible
            | Self::DuplicateSupplierQuote => StatusCode::CONFLICT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitSupplierQuoteOutcome {
    Accepted(QuoteSubmitted),
    Rejected(SubmissionRejection),
}
