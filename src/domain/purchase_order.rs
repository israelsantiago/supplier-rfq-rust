use std::collections::HashSet;

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    domain::sourcing::AwardedSupplierQuote,
    events::{ProcurementBudgetReservedFact, PurchaseOrderCreatedV2Fact},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PurchaseOrderRejection {
    #[error("award and budget reservation refer to different RFQs")]
    RfqMismatch,
    #[error("award and budget reservation refer to different suppliers")]
    SupplierMismatch,
    #[error("award and budget reservation use different currencies")]
    CurrencyMismatch,
    #[error("award amount does not exactly match the budget reservation")]
    AmountMismatch,
    #[error("award amount must be positive")]
    InvalidAmount,
    #[error("purchase order cannot be issued without quote lines")]
    EmptyLines,
    #[error("a quote line has an invalid quantity or unit price")]
    InvalidLine,
    #[error("quote line arithmetic overflowed the supported integer range")]
    LineAmountOverflow,
    #[error("quote line total does not match quantity multiplied by unit price")]
    InvalidLineTotal,
    #[error("quote line identifiers are duplicated")]
    DuplicateLine,
    #[error("sum of quote lines does not match the award amount")]
    TotalMismatch,
}

/// Issues the domain PO fact v2 only when the validated award and budget reservation agree.
///
/// v1 is intentionally untouched: v2 adds explicit budget and reservation linkage. The
/// returned value is still only a proposal; persistence and outbox dispatch belong to later
/// stages and to the owning Application State adapter.
pub fn decide_issue_purchase_order(
    awarded: &AwardedSupplierQuote,
    reservation: &ProcurementBudgetReservedFact,
    purchase_order_id: Uuid,
    created_at: DateTime<Utc>,
) -> Result<PurchaseOrderCreatedV2Fact, PurchaseOrderRejection> {
    let award = awarded.award_fact();
    let quote = awarded.source_quote();

    if award.rfq_id != reservation.rfq_id {
        return Err(PurchaseOrderRejection::RfqMismatch);
    }
    if award.supplier_id != reservation.supplier_id {
        return Err(PurchaseOrderRejection::SupplierMismatch);
    }
    if award.currency != reservation.currency || quote.currency != reservation.currency {
        return Err(PurchaseOrderRejection::CurrencyMismatch);
    }
    if award.total_amount_cents <= 0 || reservation.reserved_amount_cents <= 0 {
        return Err(PurchaseOrderRejection::InvalidAmount);
    }
    if award.total_amount_cents != reservation.reserved_amount_cents {
        return Err(PurchaseOrderRejection::AmountMismatch);
    }
    if quote.rfq_id != award.rfq_id
        || quote.supplier_id != award.supplier_id
        || quote.quote_id != award.quote_id
        || quote.total_amount_cents != award.total_amount_cents
        || quote.delivery_days != award.delivery_days
        || quote.valid_until != award.valid_until
    {
        return Err(PurchaseOrderRejection::TotalMismatch);
    }
    if quote.items.is_empty() {
        return Err(PurchaseOrderRejection::EmptyLines);
    }

    let mut line_ids = HashSet::with_capacity(quote.items.len());
    let mut calculated_total = 0_i64;
    for item in &quote.items {
        if !line_ids.insert(item.rfq_item_id) {
            return Err(PurchaseOrderRejection::DuplicateLine);
        }
        if item.quantity <= 0 || item.unit_price_cents <= 0 {
            return Err(PurchaseOrderRejection::InvalidLine);
        }
        let calculated_line = item
            .quantity
            .checked_mul(item.unit_price_cents)
            .ok_or(PurchaseOrderRejection::LineAmountOverflow)?;
        if calculated_line != item.line_total_cents {
            return Err(PurchaseOrderRejection::InvalidLineTotal);
        }
        calculated_total = calculated_total
            .checked_add(calculated_line)
            .ok_or(PurchaseOrderRejection::LineAmountOverflow)?;
    }
    if calculated_total != award.total_amount_cents {
        return Err(PurchaseOrderRejection::TotalMismatch);
    }

    Ok(PurchaseOrderCreatedV2Fact {
        purchase_order_id,
        rfq_id: award.rfq_id,
        supplier_id: award.supplier_id,
        source_quote_id: award.quote_id,
        budget_id: reservation.budget_id,
        budget_reservation_id: reservation.reservation_id,
        currency: award.currency.clone(),
        total_amount_cents: award.total_amount_cents,
        delivery_days: award.delivery_days,
        created_at,
        items: quote.items.clone(),
    })
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};
    use uuid::Uuid;

    use crate::{
        domain::{
            purchase_order::{PurchaseOrderRejection, decide_issue_purchase_order},
            sourcing::{AwardQuoteSelection, SourcingContext, decide_award_supplier_quote},
        },
        events::{
            ProcurementBudgetReservedFact, QuotedItemFact, RfqItemFact, RfqOpenedFact,
            SupplierQuoteSubmittedFact, SupplierStatus,
        },
    };

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    fn award() -> crate::domain::sourcing::AwardedSupplierQuote {
        let now = Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap();
        let context = SourcingContext {
            rfq: Some(RfqOpenedFact {
                rfq_id: id(1),
                reference: "RFQ-1".into(),
                currency: "BRL".into(),
                opened_at: now,
                items: vec![RfqItemFact {
                    id: id(4),
                    sku: "SKU-1".into(),
                    description: "Item 1".into(),
                    required_quantity: 2,
                }],
            }),
            closed: false,
            supplier_registered: true,
            supplier_status: Some(SupplierStatus::Eligible),
            supplier_quote_count: 1,
            selected_quote: Some(SupplierQuoteSubmittedFact {
                quote_id: id(3),
                rfq_id: id(1),
                supplier_id: id(2),
                currency: "BRL".into(),
                total_amount_cents: 500,
                delivery_days: 4,
                valid_until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
                submitted_at: now,
                items: vec![QuotedItemFact {
                    rfq_item_id: id(4),
                    quantity: 2,
                    unit_price_cents: 250,
                    line_total_cents: 500,
                }],
            }),
            latest_quote_sequence_number: Some(10),
            finalized: false,
        };
        decide_award_supplier_quote(
            &AwardQuoteSelection::new(id(1), id(2), id(3), 10),
            &context,
            now,
            id(7),
        )
        .unwrap()
    }

    fn reservation() -> ProcurementBudgetReservedFact {
        ProcurementBudgetReservedFact {
            reservation_id: id(8),
            budget_id: id(9),
            rfq_id: id(1),
            supplier_id: id(2),
            currency: "BRL".into(),
            reserved_amount_cents: 500,
            reserved_at: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        }
    }

    #[test]
    fn issues_v2_with_budget_linkage_and_preserves_quote_lines() {
        let fact = decide_issue_purchase_order(
            &award(),
            &reservation(),
            id(11),
            Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(fact.budget_id, id(9));
        assert_eq!(fact.budget_reservation_id, id(8));
        assert_eq!(fact.total_amount_cents, 500);
        assert_eq!(fact.items.len(), 1);
    }

    #[test]
    fn rejects_reservation_that_does_not_match_awarded_amount() {
        let mut reserved = reservation();
        reserved.reserved_amount_cents = 501;
        assert_eq!(
            decide_issue_purchase_order(&award(), &reserved, id(11), Utc::now()),
            Err(PurchaseOrderRejection::AmountMismatch)
        );
    }

    #[test]
    fn rejects_supplier_or_currency_mismatch() {
        let mut reserved = reservation();
        reserved.supplier_id = id(99);
        assert_eq!(
            decide_issue_purchase_order(&award(), &reserved, id(11), Utc::now()),
            Err(PurchaseOrderRejection::SupplierMismatch)
        );
        reserved = reservation();
        reserved.currency = "USD".into();
        assert_eq!(
            decide_issue_purchase_order(&award(), &reserved, id(11), Utc::now()),
            Err(PurchaseOrderRejection::CurrencyMismatch)
        );
    }
}
