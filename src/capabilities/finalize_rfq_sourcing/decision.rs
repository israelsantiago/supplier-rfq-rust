//! Deterministic domain decision logic for finalizing RFQ sourcing.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::events::{
    PurchaseOrderCreatedFact, PurchaseOrderDispatchRequestedFact, RfqAwardedFact, RfqClosedFact,
    RfqOpenedFact, SupplierQuoteSubmittedFact, SupplierStatus,
};

use super::FinalizeRfqSourcingRequest;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourcingRejection {
    #[error("RFQ was not found")]
    RfqNotFound,
    #[error("RFQ is already closed")]
    RfqAlreadyClosed,
    #[error("RFQ sourcing has already been finalized")]
    AlreadyFinalized,
    #[error("supplier has not been registered")]
    SupplierNotRegistered,
    #[error("supplier eligibility has not been set")]
    SupplierEligibilityNotSet,
    #[error("supplier is not eligible for award")]
    SupplierNotEligible,
    #[error(
        "the quote set changed after it was reviewed; reload quotes and confirm the selection again"
    )]
    QuoteSetChanged,
    #[error("the selected quote was not found for this RFQ and supplier")]
    QuoteNotFound,
    #[error("more than one quote exists for the selected supplier and RFQ")]
    DuplicateSupplierQuote,
    #[error("the selected quote has expired")]
    QuoteExpired,
    #[error("quote currency does not match RFQ currency")]
    CurrencyMismatch,
    #[error("quote delivery_days must be positive")]
    InvalidDeliveryDays,
    #[error("RFQ must contain at least one item")]
    EmptyRfq,
    #[error("selected quote must contain every RFQ item exactly once")]
    EmptyQuote,
    #[error("quoted item IDs do not match the RFQ item set")]
    ItemSetMismatch,
    #[error("an RFQ item is duplicated in the selected quote")]
    DuplicateQuotedItem,
    #[error("quoted quantity does not match the RFQ requested quantity")]
    QuantityMismatch,
    #[error("unit price must be positive")]
    InvalidUnitPrice,
    #[error("quote totals are inconsistent with quote line amounts")]
    InvalidQuoteTotals,
    #[error("quote amount overflowed the supported integer range")]
    AmountOverflow,
    #[error(
        "the command context changed before the atomic commit; reload and retry with a new idempotency key"
    )]
    CommandContextChanged,
}

/// Explicit values reconstructed from the single FACTSTR context query.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcingContext {
    pub rfq: Option<RfqOpenedFact>,
    pub closed: bool,
    pub supplier_registered: bool,
    pub supplier_status: Option<SupplierStatus>,
    /// Number of quote facts for this supplier/RFQ pair, including a quote whose ID
    /// does not match the selected ID. The legacy quote RPU guarantees this is <= 1.
    pub supplier_quote_count: usize,
    pub selected_quote: Option<SupplierQuoteSubmittedFact>,
    /// Highest FACTSTR sequence number among all quotes for this RFQ.
    pub latest_quote_sequence_number: Option<u64>,
    pub finalized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcingDecision {
    pub closed: RfqClosedFact,
    pub awarded: RfqAwardedFact,
    pub purchase_order: PurchaseOrderCreatedFact,
    pub dispatch_requested: PurchaseOrderDispatchRequestedFact,
}

/// Pure decision: one explicit request + its full consistency context + stable values in,
/// one all-or-nothing set of consequences out.
pub fn decide(
    request: &FinalizeRfqSourcingRequest,
    context: &SourcingContext,
    now: DateTime<Utc>,
    award_id: Uuid,
    purchase_order_id: Uuid,
    dispatch_id: Uuid,
) -> Result<SourcingDecision, SourcingRejection> {
    let rfq = context.rfq.as_ref().ok_or(SourcingRejection::RfqNotFound)?;

    if context.finalized {
        return Err(SourcingRejection::AlreadyFinalized);
    }
    if context.closed {
        return Err(SourcingRejection::RfqAlreadyClosed);
    }
    if context.latest_quote_sequence_number != Some(request.expected_quote_set_version) {
        return Err(SourcingRejection::QuoteSetChanged);
    }
    if !context.supplier_registered {
        return Err(SourcingRejection::SupplierNotRegistered);
    }
    match context.supplier_status {
        None => return Err(SourcingRejection::SupplierEligibilityNotSet),
        Some(SupplierStatus::Blocked) => return Err(SourcingRejection::SupplierNotEligible),
        Some(SupplierStatus::Eligible) => {}
    }
    if context.supplier_quote_count == 0 {
        return Err(SourcingRejection::QuoteNotFound);
    }
    if context.supplier_quote_count > 1 {
        return Err(SourcingRejection::DuplicateSupplierQuote);
    }
    let quote = context
        .selected_quote
        .as_ref()
        .ok_or(SourcingRejection::QuoteNotFound)?;

    if quote.rfq_id != request.rfq_id
        || quote.supplier_id != request.supplier_id
        || quote.quote_id != request.quote_id
    {
        return Err(SourcingRejection::QuoteNotFound);
    }
    if quote.valid_until < now.date_naive() {
        return Err(SourcingRejection::QuoteExpired);
    }
    if quote.currency != rfq.currency {
        return Err(SourcingRejection::CurrencyMismatch);
    }
    if quote.delivery_days <= 0 {
        return Err(SourcingRejection::InvalidDeliveryDays);
    }
    validate_quote_items(rfq, quote)?;

    Ok(SourcingDecision {
        closed: RfqClosedFact {
            rfq_id: request.rfq_id,
            closed_at: now,
        },
        awarded: RfqAwardedFact {
            award_id,
            rfq_id: request.rfq_id,
            quote_id: quote.quote_id,
            supplier_id: quote.supplier_id,
            awarded_at: now,
            currency: quote.currency.clone(),
            total_amount_cents: quote.total_amount_cents,
            delivery_days: quote.delivery_days,
            valid_until: quote.valid_until,
        },
        purchase_order: PurchaseOrderCreatedFact {
            purchase_order_id,
            rfq_id: request.rfq_id,
            supplier_id: quote.supplier_id,
            source_quote_id: quote.quote_id,
            currency: quote.currency.clone(),
            total_amount_cents: quote.total_amount_cents,
            delivery_days: quote.delivery_days,
            created_at: now,
            items: quote.items.clone(),
        },
        dispatch_requested: PurchaseOrderDispatchRequestedFact {
            dispatch_id,
            purchase_order_id,
            rfq_id: request.rfq_id,
            requested_at: now,
        },
    })
}

fn validate_quote_items(
    rfq: &RfqOpenedFact,
    quote: &SupplierQuoteSubmittedFact,
) -> Result<(), SourcingRejection> {
    if rfq.items.is_empty() {
        return Err(SourcingRejection::EmptyRfq);
    }
    if quote.items.is_empty() {
        return Err(SourcingRejection::EmptyQuote);
    }

    let required_by_id: HashMap<Uuid, i64> = rfq
        .items
        .iter()
        .map(|item| (item.id, item.required_quantity))
        .collect();
    if required_by_id.len() != rfq.items.len() {
        return Err(SourcingRejection::ItemSetMismatch);
    }

    let mut seen = HashSet::with_capacity(quote.items.len());
    for item in &quote.items {
        if !seen.insert(item.rfq_item_id) {
            return Err(SourcingRejection::DuplicateQuotedItem);
        }
    }
    if quote.items.len() != required_by_id.len()
        || !seen
            .iter()
            .all(|item_id| required_by_id.contains_key(item_id))
    {
        return Err(SourcingRejection::ItemSetMismatch);
    }

    let mut calculated_total = 0_i64;
    for item in &quote.items {
        let required_quantity = required_by_id[&item.rfq_item_id];
        if item.quantity != required_quantity || item.quantity <= 0 {
            return Err(SourcingRejection::QuantityMismatch);
        }
        if item.unit_price_cents <= 0 {
            return Err(SourcingRejection::InvalidUnitPrice);
        }
        let calculated_line_total = item
            .quantity
            .checked_mul(item.unit_price_cents)
            .ok_or(SourcingRejection::AmountOverflow)?;
        if item.line_total_cents != calculated_line_total {
            return Err(SourcingRejection::InvalidQuoteTotals);
        }
        calculated_total = calculated_total
            .checked_add(calculated_line_total)
            .ok_or(SourcingRejection::AmountOverflow)?;
    }
    if calculated_total != quote.total_amount_cents {
        return Err(SourcingRejection::InvalidQuoteTotals);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{QuotedItemFact, RfqItemFact};
    use chrono::{NaiveDate, TimeZone};

    fn request() -> FinalizeRfqSourcingRequest {
        FinalizeRfqSourcingRequest {
            rfq_id: Uuid::from_u128(1),
            supplier_id: Uuid::from_u128(2),
            quote_id: Uuid::from_u128(3),
            expected_quote_set_version: 40,
        }
    }

    fn rfq() -> RfqOpenedFact {
        RfqOpenedFact {
            rfq_id: Uuid::from_u128(1),
            reference: "RFQ-TEST-1".to_owned(),
            currency: "BRL".to_owned(),
            opened_at: Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap(),
            items: vec![RfqItemFact {
                id: Uuid::from_u128(4),
                sku: "SKU-1".to_owned(),
                description: "Item 1".to_owned(),
                required_quantity: 2,
            }],
        }
    }

    fn quote() -> SupplierQuoteSubmittedFact {
        SupplierQuoteSubmittedFact {
            quote_id: Uuid::from_u128(3),
            rfq_id: Uuid::from_u128(1),
            supplier_id: Uuid::from_u128(2),
            currency: "BRL".to_owned(),
            total_amount_cents: 500,
            delivery_days: 5,
            valid_until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            submitted_at: Utc.with_ymd_and_hms(2026, 1, 2, 10, 0, 0).unwrap(),
            items: vec![QuotedItemFact {
                rfq_item_id: Uuid::from_u128(4),
                quantity: 2,
                unit_price_cents: 250,
                line_total_cents: 500,
            }],
        }
    }

    fn context() -> SourcingContext {
        SourcingContext {
            rfq: Some(rfq()),
            closed: false,
            supplier_registered: true,
            supplier_status: Some(SupplierStatus::Eligible),
            supplier_quote_count: 1,
            selected_quote: Some(quote()),
            latest_quote_sequence_number: Some(40),
            finalized: false,
        }
    }

    fn decide_for(
        context: &SourcingContext,
        request: &FinalizeRfqSourcingRequest,
    ) -> Result<SourcingDecision, SourcingRejection> {
        decide(
            request,
            context,
            Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap(),
            Uuid::from_u128(10),
            Uuid::from_u128(11),
            Uuid::from_u128(12),
        )
    }

    #[test]
    fn valid_context_produces_all_four_consequences_from_one_decision() {
        let result = decide_for(&context(), &request()).unwrap();
        assert_eq!(result.closed.rfq_id, Uuid::from_u128(1));
        assert_eq!(result.awarded.quote_id, Uuid::from_u128(3));
        assert_eq!(result.purchase_order.purchase_order_id, Uuid::from_u128(11));
        assert_eq!(
            result.dispatch_requested.purchase_order_id,
            Uuid::from_u128(11)
        );
        assert_eq!(result.purchase_order.items, quote().items);
    }

    #[test]
    fn rejects_missing_rfq() {
        let mut ctx = context();
        ctx.rfq = None;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::RfqNotFound
        );
    }

    #[test]
    fn rejects_already_finalized_before_closed_state() {
        let mut ctx = context();
        ctx.finalized = true;
        ctx.closed = true;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::AlreadyFinalized
        );
    }

    #[test]
    fn rejects_already_closed_rfq() {
        let mut ctx = context();
        ctx.closed = true;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::RfqAlreadyClosed
        );
    }

    #[test]
    fn rejects_stale_quote_set_token() {
        let mut ctx = context();
        ctx.latest_quote_sequence_number = Some(41);
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuoteSetChanged
        );
    }

    #[test]
    fn rejects_unregistered_supplier() {
        let mut ctx = context();
        ctx.supplier_registered = false;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::SupplierNotRegistered
        );
    }

    #[test]
    fn rejects_missing_eligibility() {
        let mut ctx = context();
        ctx.supplier_status = None;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::SupplierEligibilityNotSet
        );
    }

    #[test]
    fn rejects_blocked_supplier() {
        let mut ctx = context();
        ctx.supplier_status = Some(SupplierStatus::Blocked);
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::SupplierNotEligible
        );
    }

    #[test]
    fn rejects_missing_or_duplicate_supplier_quote() {
        let mut ctx = context();
        ctx.supplier_quote_count = 0;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuoteNotFound
        );
        ctx.supplier_quote_count = 2;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::DuplicateSupplierQuote
        );
        ctx.supplier_quote_count = 1;
        ctx.selected_quote = None;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuoteNotFound
        );
    }

    #[test]
    fn rejects_selected_quote_with_wrong_identity() {
        let mut ctx = context();
        ctx.selected_quote.as_mut().unwrap().quote_id = Uuid::from_u128(99);
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuoteNotFound
        );
        ctx.selected_quote = Some(quote());
        ctx.selected_quote.as_mut().unwrap().supplier_id = Uuid::from_u128(98);
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuoteNotFound
        );
        ctx.selected_quote = Some(quote());
        ctx.selected_quote.as_mut().unwrap().rfq_id = Uuid::from_u128(97);
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuoteNotFound
        );
    }

    #[test]
    fn accepts_quote_valid_until_decision_date_and_produces_all_four_facts() {
        let mut ctx = context();
        let decision_at = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
        let decision_date = decision_at.date_naive();
        ctx.selected_quote
            .as_mut()
            .expect("valid fixture contains a selected quote")
            .valid_until = decision_date;

        let result = decide_for(&ctx, &request())
            .expect("a quote is valid through its inclusive validity date");

        // The pure decision returns all four required domain consequences.
        assert_eq!(result.closed.rfq_id, Uuid::from_u128(1));
        assert_eq!(result.closed.closed_at, decision_at);
        assert_eq!(result.awarded.award_id, Uuid::from_u128(10));
        assert_eq!(result.awarded.quote_id, Uuid::from_u128(3));
        assert_eq!(result.awarded.valid_until, decision_date);
        assert_eq!(result.awarded.awarded_at, decision_at);
        assert_eq!(result.purchase_order.purchase_order_id, Uuid::from_u128(11));
        assert_eq!(result.purchase_order.source_quote_id, Uuid::from_u128(3));
        assert_eq!(result.purchase_order.created_at, decision_at);
        assert_eq!(result.dispatch_requested.dispatch_id, Uuid::from_u128(12));
        assert_eq!(
            result.dispatch_requested.purchase_order_id,
            Uuid::from_u128(11)
        );
        assert_eq!(result.dispatch_requested.requested_at, decision_at);
    }

    #[test]
    fn rejects_expired_quote_and_currency_mismatch() {
        let mut ctx = context();
        ctx.selected_quote.as_mut().unwrap().valid_until =
            NaiveDate::from_ymd_opt(2026, 10, 28).unwrap();
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuoteExpired
        );
        ctx.selected_quote = Some(quote());
        ctx.selected_quote.as_mut().unwrap().currency = "USD".to_owned();
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::CurrencyMismatch
        );
        ctx = context();
        ctx.selected_quote.as_mut().unwrap().delivery_days = 0;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::InvalidDeliveryDays
        );
    }

    #[test]
    fn rejects_empty_rfq_or_quote_items() {
        let mut ctx = context();
        ctx.rfq.as_mut().unwrap().items.clear();
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::EmptyRfq
        );
        ctx.rfq = Some(rfq());
        ctx.selected_quote.as_mut().unwrap().items.clear();
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::EmptyQuote
        );
    }

    #[test]
    fn rejects_duplicate_rfq_item_ids_and_quote_item_ids() {
        let mut ctx = context();
        let mut duplicate = rfq().items[0].clone();
        duplicate.description = "duplicate".to_owned();
        ctx.rfq.as_mut().unwrap().items.push(duplicate);
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::ItemSetMismatch
        );
        ctx = context();
        ctx.selected_quote
            .as_mut()
            .unwrap()
            .items
            .push(quote().items[0].clone());
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::DuplicateQuotedItem
        );
    }

    #[test]
    fn rejects_quote_item_set_mismatch_and_nonpositive_quantity() {
        let mut ctx = context();
        ctx.selected_quote.as_mut().unwrap().items[0].rfq_item_id = Uuid::from_u128(500);
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::ItemSetMismatch
        );
        ctx = context();
        ctx.rfq.as_mut().unwrap().items.push(RfqItemFact {
            id: Uuid::from_u128(501),
            sku: "SKU-MISSING".to_owned(),
            description: "A requested item omitted from the quote".to_owned(),
            required_quantity: 1,
        });
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::ItemSetMismatch
        );
        ctx = context();
        ctx.selected_quote.as_mut().unwrap().items[0].quantity = 1;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuantityMismatch
        );
        ctx.selected_quote.as_mut().unwrap().items[0].quantity = 0;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuantityMismatch
        );
        // Cover the `quantity <= 0` branch independently from the mismatch branch.
        ctx = context();
        ctx.rfq.as_mut().unwrap().items[0].required_quantity = 0;
        let item = &mut ctx.selected_quote.as_mut().unwrap().items[0];
        item.quantity = 0;
        item.line_total_cents = 0;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuantityMismatch
        );
    }

    #[test]
    fn rejects_nonpositive_unit_price() {
        let mut ctx = context();
        ctx.selected_quote.as_mut().unwrap().items[0].unit_price_cents = 0;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::InvalidUnitPrice
        );
    }

    #[test]
    fn rejects_line_total_and_quote_total_mismatch() {
        let mut ctx = context();
        ctx.selected_quote.as_mut().unwrap().items[0].line_total_cents = 499;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::InvalidQuoteTotals
        );
        ctx = context();
        ctx.selected_quote.as_mut().unwrap().total_amount_cents = 499;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::InvalidQuoteTotals
        );
    }

    #[test]
    fn rejects_line_multiplication_overflow() {
        let mut ctx = context();
        let item = &mut ctx.selected_quote.as_mut().unwrap().items[0];
        item.quantity = i64::MAX;
        item.unit_price_cents = 2;
        item.line_total_cents = 1;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::QuantityMismatch
        );
        // Keep quantity consistent with the RFQ and exercise checked multiplication by
        // constructing a second RFQ line whose required quantity is i64::MAX.
        ctx = context();
        ctx.rfq.as_mut().unwrap().items[0].required_quantity = i64::MAX;
        let item = &mut ctx.selected_quote.as_mut().unwrap().items[0];
        item.quantity = i64::MAX;
        item.unit_price_cents = 2;
        item.line_total_cents = i64::MAX;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::AmountOverflow
        );
    }

    #[test]
    fn rejects_total_sum_overflow() {
        let mut ctx = context();
        let second_id = Uuid::from_u128(5);
        ctx.rfq.as_mut().unwrap().items.push(RfqItemFact {
            id: second_id,
            sku: "SKU-2".to_owned(),
            description: "Item 2".to_owned(),
            required_quantity: 1,
        });
        let first = &mut ctx.selected_quote.as_mut().unwrap().items[0];
        first.quantity = 2;
        first.unit_price_cents = i64::MAX;
        first.line_total_cents = i64::MAX;
        // Force each line to be representable, but the sum of the lines not to be.
        first.quantity = 1;
        ctx.rfq.as_mut().unwrap().items[0].required_quantity = 1;
        ctx.selected_quote
            .as_mut()
            .unwrap()
            .items
            .push(QuotedItemFact {
                rfq_item_id: second_id,
                quantity: 1,
                unit_price_cents: i64::MAX,
                line_total_cents: i64::MAX,
            });
        ctx.selected_quote.as_mut().unwrap().total_amount_cents = i64::MAX;
        assert_eq!(
            decide_for(&ctx, &request()).unwrap_err(),
            SourcingRejection::AmountOverflow
        );
    }
}
