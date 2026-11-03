//! Deterministic composition of shared RFQ lifecycle, award and purchase-order decisions.
//!
//! This remains the legacy single-capability baseline. It composes pure domain decisions;
//! it does not call other capability shells and still returns the same four consequences.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::FinalizeRfqSourcingRequest;
use crate::domain::rfq_lifecycle::{RfqLifecycleContext, RfqLifecycleRejection, decide_close_rfq};
use crate::domain::sourcing::{
    AwardQuoteSelection, decide_award_supplier_quote, decide_create_purchase_order,
};
use crate::events::{
    PurchaseOrderCreatedFact, PurchaseOrderDispatchRequestedFact, RfqAwardedFact, RfqClosedFact,
};

pub use crate::domain::sourcing::{SourcingContext, SourcingRejection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcingDecision {
    pub closed: RfqClosedFact,
    pub awarded: RfqAwardedFact,
    pub purchase_order: PurchaseOrderCreatedFact,
    pub dispatch_requested: PurchaseOrderDispatchRequestedFact,
}

/// Preserve the original public signature and rejection order while composing the shared
/// Functional Core. Exactly one `decide` entry point remains in this module.
pub fn decide(
    request: &FinalizeRfqSourcingRequest,
    context: &SourcingContext,
    now: DateTime<Utc>,
    award_id: Uuid,
    purchase_order_id: Uuid,
    dispatch_id: Uuid,
) -> Result<SourcingDecision, SourcingRejection> {
    let selection = AwardQuoteSelection::new(
        request.rfq_id,
        request.supplier_id,
        request.quote_id,
        request.expected_quote_set_version,
    );
    let awarded_quote = decide_award_supplier_quote(&selection, context, now, award_id)?;

    let lifecycle = RfqLifecycleContext::new(context.rfq.is_some(), context.closed);
    let closed =
        decide_close_rfq(request.rfq_id, &lifecycle, now).map_err(|rejection| match rejection {
            RfqLifecycleRejection::RfqNotFound => SourcingRejection::RfqNotFound,
            RfqLifecycleRejection::RfqAlreadyClosed => SourcingRejection::RfqAlreadyClosed,
        })?;
    let purchase_order = decide_create_purchase_order(&awarded_quote, purchase_order_id, now);
    let dispatch_requested = PurchaseOrderDispatchRequestedFact {
        dispatch_id,
        purchase_order_id,
        rfq_id: request.rfq_id,
        requested_at: now,
    };

    Ok(SourcingDecision {
        closed,
        awarded: awarded_quote.award_fact().clone(),
        purchase_order,
        dispatch_requested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{
        QuotedItemFact, RfqItemFact, RfqOpenedFact, SupplierQuoteSubmittedFact, SupplierStatus,
    };
    use chrono::{NaiveDate, TimeZone, Utc};

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

    #[test]
    fn shared_award_and_purchase_order_decisions_match_the_composed_finalize_result() {
        let ctx = context();
        let command = request();
        let decision_at = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
        let award_id = Uuid::from_u128(10);
        let purchase_order_id = Uuid::from_u128(11);
        let dispatch_id = Uuid::from_u128(12);

        let composed = decide(
            &command,
            &ctx,
            decision_at,
            award_id,
            purchase_order_id,
            dispatch_id,
        )
        .unwrap();
        let selection = AwardQuoteSelection::new(
            command.rfq_id,
            command.supplier_id,
            command.quote_id,
            command.expected_quote_set_version,
        );
        let awarded = decide_award_supplier_quote(&selection, &ctx, decision_at, award_id)
            .expect("shared award decision must accept the same valid context");

        assert_eq!(awarded.award_fact(), &composed.awarded);
        assert_eq!(awarded.source_quote(), ctx.selected_quote.as_ref().unwrap());
        assert_eq!(
            decide_create_purchase_order(&awarded, purchase_order_id, decision_at),
            composed.purchase_order,
        );
    }
}
