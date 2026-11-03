use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::events::{
    PurchaseOrderCreatedFact, RfqAwardedFact, RfqOpenedFact, SupplierQuoteSubmittedFact,
    SupplierStatus,
};

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

/// Explicit read values needed to make the supplier-award decision.
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

/// Domain input shared by the standalone award participant and FinalizeRfqSourcing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AwardQuoteSelection {
    rfq_id: Uuid,
    supplier_id: Uuid,
    quote_id: Uuid,
    expected_quote_set_version: u64,
}

impl AwardQuoteSelection {
    pub const fn new(
        rfq_id: Uuid,
        supplier_id: Uuid,
        quote_id: Uuid,
        expected_quote_set_version: u64,
    ) -> Self {
        Self {
            rfq_id,
            supplier_id,
            quote_id,
            expected_quote_set_version,
        }
    }

    pub const fn rfq_id(&self) -> Uuid {
        self.rfq_id
    }

    pub const fn supplier_id(&self) -> Uuid {
        self.supplier_id
    }

    pub const fn quote_id(&self) -> Uuid {
        self.quote_id
    }

    pub const fn expected_quote_set_version(&self) -> u64 {
        self.expected_quote_set_version
    }
}

/// Sealed, immutable hand-off from AwardSupplierQuote to CreatePurchaseOrder.
///
/// The award fact does not contain quote line items. Keeping the validated source quote
/// alongside the award preserves the exact commercial lines required by PO creation.
/// Fields are private: only the successful award decision can create this contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwardedSupplierQuote {
    award_fact: RfqAwardedFact,
    source_quote: SupplierQuoteSubmittedFact,
}

impl AwardedSupplierQuote {
    fn new(award_fact: RfqAwardedFact, source_quote: SupplierQuoteSubmittedFact) -> Self {
        Self {
            award_fact,
            source_quote,
        }
    }

    pub fn award_fact(&self) -> &RfqAwardedFact {
        &self.award_fact
    }
    pub fn source_quote(&self) -> &SupplierQuoteSubmittedFact {
        &self.source_quote
    }
}

/// Pure award decision shared by autonomous and prepared execution paths.
pub fn decide_award_supplier_quote(
    selection: &AwardQuoteSelection,
    context: &SourcingContext,
    now: DateTime<Utc>,
    award_id: Uuid,
) -> Result<AwardedSupplierQuote, SourcingRejection> {
    let rfq = context.rfq.as_ref().ok_or(SourcingRejection::RfqNotFound)?;

    // Keep the legacy rejection precedence: already-finalized precedes closed RFQ,
    // followed by quote-set version, supplier eligibility and quote validation.
    if context.finalized {
        return Err(SourcingRejection::AlreadyFinalized);
    }
    if context.closed {
        return Err(SourcingRejection::RfqAlreadyClosed);
    }
    if context.latest_quote_sequence_number != Some(selection.expected_quote_set_version()) {
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

    if quote.rfq_id != selection.rfq_id()
        || quote.supplier_id != selection.supplier_id()
        || quote.quote_id != selection.quote_id()
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

    let award_fact = RfqAwardedFact {
        award_id,
        rfq_id: selection.rfq_id(),
        quote_id: quote.quote_id,
        supplier_id: quote.supplier_id,
        awarded_at: now,
        currency: quote.currency.clone(),
        total_amount_cents: quote.total_amount_cents,
        delivery_days: quote.delivery_days,
        valid_until: quote.valid_until,
    };

    Ok(AwardedSupplierQuote::new(award_fact, quote.clone()))
}

/// Pure PO decision consumes only a validated, sealed award hand-off and stable values.
pub fn decide_create_purchase_order(
    awarded: &AwardedSupplierQuote,
    purchase_order_id: Uuid,
    created_at: DateTime<Utc>,
) -> PurchaseOrderCreatedFact {
    let award = awarded.award_fact();
    PurchaseOrderCreatedFact {
        purchase_order_id,
        rfq_id: award.rfq_id,
        supplier_id: award.supplier_id,
        source_quote_id: award.quote_id,
        currency: award.currency.clone(),
        total_amount_cents: award.total_amount_cents,
        delivery_days: award.delivery_days,
        created_at,
        items: awarded.source_quote().items.clone(),
    }
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
    use chrono::{NaiveDate, TimeZone, Utc};
    use uuid::Uuid;

    use crate::events::{
        QuotedItemFact, RfqItemFact, RfqOpenedFact, SupplierQuoteSubmittedFact, SupplierStatus,
    };

    use super::{
        AwardQuoteSelection, SourcingContext, SourcingRejection, decide_award_supplier_quote,
        decide_create_purchase_order,
    };

    fn request() -> AwardQuoteSelection {
        AwardQuoteSelection::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            40,
        )
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

    #[test]
    fn awarded_quote_contract_keeps_award_and_source_quote_together() {
        let now = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
        let awarded =
            decide_award_supplier_quote(&request(), &context(), now, Uuid::from_u128(10)).unwrap();
        assert_eq!(awarded.award_fact().quote_id, Uuid::from_u128(3));
        assert_eq!(awarded.source_quote(), &quote());
    }

    #[test]
    fn purchase_order_is_derived_from_the_validated_award_contract() {
        let now = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
        let awarded =
            decide_award_supplier_quote(&request(), &context(), now, Uuid::from_u128(10)).unwrap();
        let purchase_order = decide_create_purchase_order(&awarded, Uuid::from_u128(11), now);
        assert_eq!(purchase_order.purchase_order_id, Uuid::from_u128(11));
        assert_eq!(purchase_order.rfq_id, awarded.award_fact().rfq_id);
        assert_eq!(purchase_order.supplier_id, awarded.award_fact().supplier_id);
        assert_eq!(
            purchase_order.source_quote_id,
            awarded.award_fact().quote_id
        );
        assert_eq!(purchase_order.currency, awarded.award_fact().currency);
        assert_eq!(
            purchase_order.total_amount_cents,
            awarded.award_fact().total_amount_cents
        );
        assert_eq!(
            purchase_order.delivery_days,
            awarded.award_fact().delivery_days
        );
        assert_eq!(purchase_order.created_at, now);
        assert_eq!(purchase_order.items, quote().items);
    }

    #[test]
    fn award_rejections_preserve_the_baseline_precondition_order() {
        let now = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
        let mut ctx = context();
        ctx.rfq = None;
        assert_eq!(
            decide_award_supplier_quote(&request(), &ctx, now, Uuid::from_u128(10)),
            Err(SourcingRejection::RfqNotFound)
        );
        ctx = context();
        ctx.finalized = true;
        ctx.closed = true;
        assert_eq!(
            decide_award_supplier_quote(&request(), &ctx, now, Uuid::from_u128(10)),
            Err(SourcingRejection::AlreadyFinalized)
        );
        ctx = context();
        ctx.closed = true;
        assert_eq!(
            decide_award_supplier_quote(&request(), &ctx, now, Uuid::from_u128(10)),
            Err(SourcingRejection::RfqAlreadyClosed)
        );
    }
}
