//! Functional Core: deterministic business decision over an explicit context snapshot.
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{
    context::{RfqStatus, SubmitSupplierQuoteContext, SupplierStatus},
    request::SubmitSupplierQuote,
    result::{QuoteSubmitted, SubmissionRejection},
};

pub fn decide(
    request: &SubmitSupplierQuote,
    context: &SubmitSupplierQuoteContext,
    quote_id: Uuid,
    now: DateTime<Utc>,
) -> Result<QuoteSubmitted, SubmissionRejection> {
    let rfq = context
        .rfq
        .as_ref()
        .ok_or(SubmissionRejection::RfqNotFound)?;
    let supplier_status = context
        .supplier_status
        .ok_or(SubmissionRejection::SupplierNotFound)?;

    match rfq.status {
        RfqStatus::Open => {}
        RfqStatus::Closed => return Err(SubmissionRejection::RfqClosed),
        RfqStatus::Draft => return Err(SubmissionRejection::RfqNotOpen),
    }
    if supplier_status != SupplierStatus::Eligible {
        return Err(SubmissionRejection::SupplierNotEligible);
    }
    if request.currency.trim().to_ascii_uppercase() != rfq.currency {
        return Err(SubmissionRejection::CurrencyMismatch);
    }
    if request.delivery_days <= 0 {
        return Err(SubmissionRejection::InvalidDeliveryDays);
    }
    if request.valid_until < now.date_naive() {
        return Err(SubmissionRejection::QuoteValidityInPast);
    }
    if request.items.is_empty() {
        return Err(SubmissionRejection::EmptyQuote);
    }

    let mut requested_ids = HashSet::with_capacity(request.items.len());
    for item in &request.items {
        if !requested_ids.insert(item.rfq_item_id) {
            return Err(SubmissionRejection::DuplicateRfqItem);
        }
    }

    let required_by_id: HashMap<Uuid, i64> = rfq
        .required_items
        .iter()
        .map(|item| (item.id, item.required_quantity))
        .collect();
    if requested_ids.len() != required_by_id.len()
        || !requested_ids
            .iter()
            .all(|item_id| required_by_id.contains_key(item_id))
    {
        return Err(SubmissionRejection::ItemSetMismatch);
    }

    let mut total_amount_cents = 0_i64;
    let mut line_totals_cents = Vec::with_capacity(request.items.len());
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
        let line_total = item
            .quantity
            .checked_mul(item.unit_price_cents)
            .ok_or(SubmissionRejection::AmountOverflow)?;
        total_amount_cents = total_amount_cents
            .checked_add(line_total)
            .ok_or(SubmissionRejection::AmountOverflow)?;
        line_totals_cents.push(line_total);
    }

    Ok(QuoteSubmitted {
        id: quote_id,
        rfq_id: rfq.id,
        supplier_id: request.supplier_id,
        currency: rfq.currency.clone(),
        total_amount_cents,
        delivery_days: request.delivery_days,
        valid_until: request.valid_until,
        submitted_at: now,
        items: request.items.clone(),
        line_totals_cents,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone};

    use super::*;
    use crate::capabilities::submit_supplier_quote::context::{RequiredItem, RfqSubmissionContext};
    use crate::capabilities::submit_supplier_quote::request::QuotedItemInput;

    fn fixture() -> (SubmitSupplierQuote, SubmitSupplierQuoteContext) {
        let rfq_id = Uuid::from_u128(1);
        let item_a = Uuid::from_u128(11);
        let item_b = Uuid::from_u128(12);
        let supplier_id = Uuid::from_u128(2);
        let request = SubmitSupplierQuote {
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
        let context = SubmitSupplierQuoteContext {
            rfq: Some(RfqSubmissionContext {
                id: rfq_id,
                status: RfqStatus::Open,
                currency: "BRL".to_owned(),
                required_items: vec![
                    RequiredItem {
                        id: item_a,
                        required_quantity: 10,
                    },
                    RequiredItem {
                        id: item_b,
                        required_quantity: 5,
                    },
                ],
            }),
            supplier_status: Some(SupplierStatus::Eligible),
        };
        (request, context)
    }

    #[test]
    fn computes_total_in_integer_minor_units() {
        let (request, context) = fixture();
        let now = Utc.with_ymd_and_hms(2026, 10, 23, 12, 0, 0).unwrap();
        let result = decide(&request, &context, Uuid::from_u128(3), now).unwrap();
        assert_eq!(result.total_amount_cents, 27_500);
        assert_eq!(result.line_totals_cents, vec![12_500, 15_000]);
    }

    #[test]
    fn rejects_a_closed_rfq() {
        let (request, mut context) = fixture();
        context.rfq.as_mut().unwrap().status = RfqStatus::Closed;
        let result = decide(&request, &context, Uuid::new_v4(), Utc::now());
        assert_eq!(result.unwrap_err(), SubmissionRejection::RfqClosed);
    }

    #[test]
    fn rejects_a_draft_rfq() {
        let (request, mut context) = fixture();
        context.rfq.as_mut().unwrap().status = RfqStatus::Draft;
        let result = decide(&request, &context, Uuid::new_v4(), Utc::now());
        assert_eq!(result.unwrap_err(), SubmissionRejection::RfqNotOpen);
    }

    #[test]
    fn rejects_quantity_that_differs_from_the_rfq() {
        let (mut request, context) = fixture();
        request.items[0].quantity = 9;
        let result = decide(&request, &context, Uuid::new_v4(), Utc::now());
        assert_eq!(result.unwrap_err(), SubmissionRejection::QuantityMismatch);
    }

    #[test]
    fn rejects_a_blocked_supplier() {
        let (request, mut context) = fixture();
        context.supplier_status = Some(SupplierStatus::Blocked);
        let result = decide(&request, &context, Uuid::new_v4(), Utc::now());
        assert_eq!(
            result.unwrap_err(),
            SubmissionRejection::SupplierNotEligible
        );
    }
}
