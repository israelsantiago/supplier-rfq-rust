//! Pure Functional Core for SubmitSupplierQuote: facts/context in, decision/fact out.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use crate::events::{QuotedItemFact, RfqOpenedFact, SupplierQuoteSubmittedFact, SupplierStatus};

use super::{build_context::RfqSubmissionContext, request::SubmitSupplierQuoteRequest};

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
        // A validação do conjunto de IDs acima prova que esta chave existe.
        let required_quantity = required_by_id[&item.rfq_item_id];
        if item.quantity != required_quantity {
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
    use proptest::prelude::*;

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

    fn fixed_now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 23, 12, 0, 0).unwrap()
    }

    #[test]
    fn rejects_missing_rfq() {
        let (request, mut context) = fixture();
        context.rfq = None;
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::RfqNotFound
        );
    }

    #[test]
    fn rejects_supplier_without_registration() {
        let (request, mut context) = fixture();
        context.supplier_registered = false;
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::SupplierNotRegistered
        );
    }

    #[test]
    fn rejects_supplier_without_eligibility() {
        let (request, mut context) = fixture();
        context.supplier_status = None;
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::SupplierEligibilityNotSet
        );
    }

    #[test]
    fn rejects_blocked_supplier() {
        let (request, mut context) = fixture();
        context.supplier_status = Some(SupplierStatus::Blocked);
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::SupplierNotEligible
        );
    }

    #[test]
    fn rejects_currency_mismatch_and_accepts_case_and_whitespace_normalization() {
        let (mut request, context) = fixture();
        request.currency = "USD".to_owned();
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::CurrencyMismatch
        );

        request.currency = " brl ".to_owned();
        assert!(decide(&request, &context, Uuid::from_u128(3), fixed_now()).is_ok());
    }

    #[test]
    fn rejects_non_positive_delivery_days() {
        let (mut request, context) = fixture();
        for days in [0, -1] {
            request.delivery_days = days;
            assert_eq!(
                decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
                SubmissionRejection::InvalidDeliveryDays
            );
        }
    }

    #[test]
    fn rejects_validity_before_today() {
        let (mut request, context) = fixture();
        request.valid_until = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::ValidityInPast
        );
    }

    #[test]
    fn rejects_empty_quote() {
        let (mut request, context) = fixture();
        request.items.clear();
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::EmptyQuote
        );
    }

    #[test]
    fn rejects_duplicate_quoted_item_ids() {
        let (mut request, context) = fixture();
        request.items[1].rfq_item_id = request.items[0].rfq_item_id;
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::DuplicateQuotedItem
        );
    }

    #[test]
    fn rejects_missing_or_unknown_rfq_item_ids() {
        let (mut request, context) = fixture();
        request.items.truncate(1);
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::ItemSetMismatch
        );

        let (mut request, context) = fixture();
        request.items[1].rfq_item_id = Uuid::from_u128(999);
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::ItemSetMismatch
        );
    }

    #[test]
    fn rejects_non_positive_unit_price() {
        let (mut request, context) = fixture();
        for price in [0, -1] {
            request.items[0].unit_price_cents = price;
            assert_eq!(
                decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
                SubmissionRejection::InvalidUnitPrice
            );
        }
    }

    #[test]
    fn rejects_non_positive_quantity_even_if_loaded_context_is_invalid() {
        let (mut request, mut context) = fixture();
        request.items[0].quantity = 0;
        context.rfq.as_mut().unwrap().items[0].required_quantity = 0;
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::QuantityMismatch
        );
    }

    #[test]
    fn rejects_line_total_overflow() {
        let (mut request, mut context) = fixture();
        request.items.truncate(1);
        let rfq = context.rfq.as_mut().unwrap();
        rfq.items.truncate(1);
        rfq.items[0].required_quantity = i64::MAX;
        request.items[0].quantity = i64::MAX;
        request.items[0].unit_price_cents = 2;
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::AmountOverflow
        );
    }

    #[test]
    fn rejects_sum_overflow_across_valid_line_totals() {
        let (mut request, mut context) = fixture();
        let high_price = i64::MAX / 2 + 1;
        for index in 0..2 {
            request.items[index].quantity = 1;
            request.items[index].unit_price_cents = high_price;
            context.rfq.as_mut().unwrap().items[index].required_quantity = 1;
        }
        assert_eq!(
            decide(&request, &context, Uuid::from_u128(3), fixed_now()).unwrap_err(),
            SubmissionRejection::AmountOverflow
        );
    }

    proptest! {
        #[test]
        fn property_quote_total_equals_sum_of_integer_line_totals(
            quantity_a in 1_i64..=10_000,
            price_a in 1_i64..=1_000_000,
            quantity_b in 1_i64..=10_000,
            price_b in 1_i64..=1_000_000,
        ) {
            let (mut request, mut context) = fixture();
            let rfq = context.rfq.as_mut().expect("fixture includes an RFQ");
            let values = [(quantity_a, price_a), (quantity_b, price_b)];
            for (index, (quantity, price)) in values.into_iter().enumerate() {
                rfq.items[index].required_quantity = quantity;
                request.items[index].quantity = quantity;
                request.items[index].unit_price_cents = price;
            }

            let quote = decide(&request, &context, Uuid::from_u128(3), fixed_now())
                .expect("positive bounded inputs must produce a quote");
            let expected = quantity_a * price_a + quantity_b * price_b;
            let sum_of_lines: i64 = quote.items.iter().map(|item| item.line_total_cents).sum();

            prop_assert_eq!(quote.total_amount_cents, expected);
            prop_assert_eq!(sum_of_lines, quote.total_amount_cents);
            prop_assert_eq!(quote.rfq_id, Uuid::from_u128(1));
            prop_assert_eq!(quote.supplier_id, Uuid::from_u128(2));
            prop_assert_eq!(quote.quote_id, Uuid::from_u128(3));
            prop_assert_eq!(quote.submitted_at, fixed_now());
        }
    }
}
