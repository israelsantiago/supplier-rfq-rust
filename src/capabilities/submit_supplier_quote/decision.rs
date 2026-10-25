//! Pure Functional Core for SubmitSupplierQuote: facts/context in, decision/fact out.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use crate::events::{QuotedItemFact, RfqOpenedFact, SupplierQuoteSubmittedFact, SupplierStatus};

use super::{context::RfqSubmissionContext, request::SubmitSupplierQuoteRequest};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SubmissionRejection {
    #[error("RFQ was not found")]
    RfqNotFound,
    #[error("RFQ is closed")]
    RfqClosed,
    #[error("supplier has not been registered")]
    SupplierNotRegistered,
    #[error("supplier eligibility has not been set")]
    SupplierEligibilityNotSet,
    #[error("supplier is not eligible to submit a quote")]
    SupplierNotEligible,
    #[error("a quote from this supplier already exists for the RFQ")]
    DuplicateSupplierQuote,
    #[error("quote currency does not match RFQ currency")]
    CurrencyMismatch,
    #[error("delivery_days must be positive")]
    InvalidDeliveryDays,
    #[error("valid_until cannot be in the past")]
    ValidityInPast,
    #[error("quote must contain all requested RFQ items")]
    EmptyQuote,
    #[error("an RFQ item cannot be quoted more than once")]
    DuplicateQuotedItem,
    #[error("quoted items do not match the RFQ item set")]
    ItemSetMismatch,
    #[error("quoted quantity must match the requested quantity")]
    QuantityMismatch,
    #[error("unit price must be positive")]
    InvalidUnitPrice,
    #[error("quote amount overflowed the supported integer range")]
    AmountOverflow,
    #[error("the decision context changed repeatedly; retry the command")]
    CommandContextChanged,
}

pub fn decide(
    request: &SubmitSupplierQuoteRequest,
    context: &RfqSubmissionContext,
    quote_id: Uuid,
    now: chrono::DateTime<Utc>,
) -> Result<SupplierQuoteSubmittedFact, SubmissionRejection> {
    let rfq = context
        .rfq
        .as_ref()
        .ok_or(SubmissionRejection::RfqNotFound)?;
    if context.closed {
        return Err(SubmissionRejection::RfqClosed);
    }
    if !context.supplier_registered {
        return Err(SubmissionRejection::SupplierNotRegistered);
    }
    match context.supplier_status {
        None => return Err(SubmissionRejection::SupplierEligibilityNotSet),
        Some(SupplierStatus::Blocked) => return Err(SubmissionRejection::SupplierNotEligible),
        Some(SupplierStatus::Eligible) => {}
    }
    if context.already_submitted {
        return Err(SubmissionRejection::DuplicateSupplierQuote);
    }
    decide_for_rfq(request, rfq, quote_id, now)
}

fn decide_for_rfq(
    request: &SubmitSupplierQuoteRequest,
    rfq: &RfqOpenedFact,
    quote_id: Uuid,
    now: chrono::DateTime<Utc>,
) -> Result<SupplierQuoteSubmittedFact, SubmissionRejection> {
    if request.currency.trim().to_ascii_uppercase() != rfq.currency {
        return Err(SubmissionRejection::CurrencyMismatch);
    }
    if request.delivery_days <= 0 {
        return Err(SubmissionRejection::InvalidDeliveryDays);
    }
    if request.valid_until < now.date_naive() {
        return Err(SubmissionRejection::ValidityInPast);
    }
    if request.items.is_empty() {
        return Err(SubmissionRejection::EmptyQuote);
    }

    let mut seen_ids = HashSet::with_capacity(request.items.len());
    for item in &request.items {
        if !seen_ids.insert(item.rfq_item_id) {
            return Err(SubmissionRejection::DuplicateQuotedItem);
        }
    }

    let required_by_id: HashMap<Uuid, i64> = rfq
        .items
        .iter()
        .map(|item| (item.id, item.required_quantity))
        .collect();
    if request.items.len() != required_by_id.len()
        || !seen_ids
            .iter()
            .all(|item_id| required_by_id.contains_key(item_id))
    {
        return Err(SubmissionRejection::ItemSetMismatch);
    }

    let mut total_amount_cents = 0_i64;
    let mut quoted_items = Vec::with_capacity(request.items.len());
    for item in &request.items {
        if item.unit_price_cents <= 0 {
            return Err(SubmissionRejection::InvalidUnitPrice);
        }
        let Some(required_quantity) = required_by_id.get(&item.rfq_item_id) else {
            return Err(SubmissionRejection::ItemSetMismatch);
        };
        if item.quantity != *required_quantity {
            return Err(SubmissionRejection::QuantityMismatch);
        }
        if item.quantity <= 0 {
            return Err(SubmissionRejection::QuantityMismatch);
        }
        let line_total_cents = item
            .quantity
            .checked_mul(item.unit_price_cents)
            .ok_or(SubmissionRejection::AmountOverflow)?;
        total_amount_cents = total_amount_cents
            .checked_add(line_total_cents)
            .ok_or(SubmissionRejection::AmountOverflow)?;
        quoted_items.push(QuotedItemFact {
            rfq_item_id: item.rfq_item_id,
            quantity: item.quantity,
            unit_price_cents: item.unit_price_cents,
            line_total_cents,
        });
    }

    Ok(SupplierQuoteSubmittedFact {
        quote_id,
        rfq_id: rfq.rfq_id,
        supplier_id: request.supplier_id,
        currency: rfq.currency.clone(),
        total_amount_cents,
        delivery_days: request.delivery_days,
        valid_until: request.valid_until,
        submitted_at: now,
        items: quoted_items,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone};

    use super::*;
    use crate::capabilities::submit_supplier_quote::request::QuotedItemInput;
    use crate::events::RfqItemFact;

    fn fixture() -> (SubmitSupplierQuoteRequest, RfqSubmissionContext) {
        let rfq_id = Uuid::from_u128(1);
        let item_a = Uuid::from_u128(11);
        let item_b = Uuid::from_u128(12);
        let supplier_id = Uuid::from_u128(2);
        let request = SubmitSupplierQuoteRequest {
            supplier_id,
            currency: "BRL".to_owned(),
            delivery_days: 7,
            valid_until: NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
            items: vec![
                QuotedItemInput {
                    rfq_item_id: item_a,
                    quantity: 10,
                    unit_price_cents: 1250,
                },
                QuotedItemInput {
                    rfq_item_id: item_b,
                    quantity: 5,
                    unit_price_cents: 3000,
                },
            ],
        };
        let context = RfqSubmissionContext {
            rfq: Some(RfqOpenedFact {
                rfq_id,
                reference: "RFQ-UNIT-001".to_owned(),
                currency: "BRL".to_owned(),
                opened_at: Utc.with_ymd_and_hms(2026, 10, 23, 12, 0, 0).unwrap(),
                items: vec![
                    RfqItemFact {
                        id: item_a,
                        sku: "SKU-A".to_owned(),
                        description: "A".to_owned(),
                        required_quantity: 10,
                    },
                    RfqItemFact {
                        id: item_b,
                        sku: "SKU-B".to_owned(),
                        description: "B".to_owned(),
                        required_quantity: 5,
                    },
                ],
            }),
            closed: false,
            supplier_registered: true,
            supplier_status: Some(SupplierStatus::Eligible),
            already_submitted: false,
        };
        (request, context)
    }

    #[test]
    fn calculates_quote_total_with_integer_minor_units() {
        let (request, context) = fixture();
        let now = Utc.with_ymd_and_hms(2026, 10, 23, 12, 0, 0).unwrap();
        let quote = decide(&request, &context, Uuid::from_u128(3), now).unwrap();
        assert_eq!(quote.total_amount_cents, 27_500);
        assert_eq!(
            quote
                .items
                .iter()
                .map(|item| item.line_total_cents)
                .collect::<Vec<_>>(),
            vec![12_500, 15_000]
        );
    }

    #[test]
    fn rejects_quantity_that_differs_from_rfq() {
        let (mut request, context) = fixture();
        request.items[0].quantity = 9;
        let result = decide(&request, &context, Uuid::new_v4(), Utc::now());
        assert_eq!(result.unwrap_err(), SubmissionRejection::QuantityMismatch);
    }

    #[test]
    fn rejects_closed_rfq() {
        let (request, mut context) = fixture();
        context.closed = true;
        assert_eq!(
            decide(&request, &context, Uuid::new_v4(), Utc::now()).unwrap_err(),
            SubmissionRejection::RfqClosed
        );
    }

    #[test]
    fn rejects_duplicate_submission() {
        let (request, mut context) = fixture();
        context.already_submitted = true;
        assert_eq!(
            decide(&request, &context, Uuid::new_v4(), Utc::now()).unwrap_err(),
            SubmissionRejection::DuplicateSupplierQuote
        );
    }
}
