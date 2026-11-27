use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::events::{ProcurementBudgetAllocatedFact, ProcurementBudgetReservedFact};

/// Read model required by the budget-reservation decision.
///
/// `allocations` and `reservations` are projected from the same composite snapshot used
/// by the sourcing participant. V1 deliberately has no release, cancellation/refund or
/// adjustment fact. Therefore every historical reservation remains occupied and the
/// `(budget_id, rfq_id)` uniqueness rule is lifetime-scoped, not merely active-scoped.
/// This fail-closed pilot policy is not approved for production until the Finance/Procurement
/// owner defines the allocation producer and reservation lifecycle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcurementBudgetContext {
    pub allocations: Vec<ProcurementBudgetAllocatedFact>,
    pub reservations: Vec<ProcurementBudgetReservedFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BudgetReservationRejection {
    #[error("requested reservation amount must be positive")]
    InvalidRequestedAmount,
    #[error("budget allocation amount must be positive")]
    InvalidAllocationAmount,
    #[error("no allocation exists for the requested budget")]
    BudgetNotFound,
    #[error("more than one allocation fact exists for the same budget")]
    DuplicateBudgetAllocation,
    #[error("currency must be a three-letter uppercase code")]
    InvalidCurrency,
    #[error("budget currency does not match the reviewed quote currency")]
    CurrencyMismatch,
    #[error("an existing reservation has a non-positive amount")]
    InvalidExistingReservation,
    #[error("the generated reservation identifier already exists in the budget history")]
    ReservationIdentityAlreadyUsed,
    #[error("existing reservation total exceeds the supported integer range")]
    ReservationTotalOverflow,
    #[error("an existing reservation already exists for this RFQ and budget pair")]
    AlreadyReservedForRfq,
    #[error("existing reservations exceed the allocated budget")]
    BudgetAlreadyOverdrawn,
    #[error("available budget is insufficient for the requested reservation")]
    InsufficientFunds,
}

/// Immutable command values captured by the Application before preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetReservationCommand {
    pub budget_id: Uuid,
    pub rfq_id: Uuid,
    pub supplier_id: Uuid,
    pub requested_amount_cents: i64,
    pub requested_currency: String,
    pub reservation_id: Uuid,
    pub reserved_at: DateTime<Utc>,
}

/// Reserves the buyer-reviewed amount against one already allocated budget.
///
/// This function is a Functional Core decision: it reads only supplied values, performs
/// checked integer arithmetic in minor currency units, and returns a proposed fact. It
/// does not create/adjust an allocation, persist anything, or contact an ERP.
fn valid_currency_code(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase())
}

pub fn decide_reserve_procurement_budget(
    command: &BudgetReservationCommand,
    context: &ProcurementBudgetContext,
) -> Result<ProcurementBudgetReservedFact, BudgetReservationRejection> {
    if command.requested_amount_cents <= 0 {
        return Err(BudgetReservationRejection::InvalidRequestedAmount);
    }
    if !valid_currency_code(&command.requested_currency) {
        return Err(BudgetReservationRejection::InvalidCurrency);
    }

    let mut allocations = context
        .allocations
        .iter()
        .filter(|allocation| allocation.budget_id == command.budget_id);
    let allocation = allocations
        .next()
        .ok_or(BudgetReservationRejection::BudgetNotFound)?;
    if allocations.next().is_some() {
        return Err(BudgetReservationRejection::DuplicateBudgetAllocation);
    }
    if allocation.allocated_amount_cents <= 0 {
        return Err(BudgetReservationRejection::InvalidAllocationAmount);
    }
    if !valid_currency_code(&allocation.currency) {
        return Err(BudgetReservationRejection::InvalidCurrency);
    }
    if allocation.currency != command.requested_currency {
        return Err(BudgetReservationRejection::CurrencyMismatch);
    }

    let mut reserved_total_cents = 0_i64;
    for reservation in context
        .reservations
        .iter()
        .filter(|reservation| reservation.budget_id == command.budget_id)
    {
        if reservation.reservation_id == command.reservation_id {
            return Err(BudgetReservationRejection::ReservationIdentityAlreadyUsed);
        }
        if reservation.reserved_amount_cents <= 0 {
            return Err(BudgetReservationRejection::InvalidExistingReservation);
        }
        if reservation.currency != allocation.currency {
            return Err(BudgetReservationRejection::CurrencyMismatch);
        }
        if reservation.rfq_id == command.rfq_id {
            return Err(BudgetReservationRejection::AlreadyReservedForRfq);
        }
        reserved_total_cents = reserved_total_cents
            .checked_add(reservation.reserved_amount_cents)
            .ok_or(BudgetReservationRejection::ReservationTotalOverflow)?;
    }

    let available_cents = allocation
        .allocated_amount_cents
        .checked_sub(reserved_total_cents)
        .ok_or(BudgetReservationRejection::BudgetAlreadyOverdrawn)?;
    if available_cents < 0 {
        return Err(BudgetReservationRejection::BudgetAlreadyOverdrawn);
    }
    if command.requested_amount_cents > available_cents {
        return Err(BudgetReservationRejection::InsufficientFunds);
    }

    Ok(ProcurementBudgetReservedFact {
        reservation_id: command.reservation_id,
        budget_id: command.budget_id,
        rfq_id: command.rfq_id,
        supplier_id: command.supplier_id,
        currency: allocation.currency.clone(),
        reserved_amount_cents: command.requested_amount_cents,
        reserved_at: command.reserved_at,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    use crate::events::{ProcurementBudgetAllocatedFact, ProcurementBudgetReservedFact};

    use super::{
        BudgetReservationRejection, ProcurementBudgetContext, decide_reserve_procurement_budget,
    };

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    fn context() -> ProcurementBudgetContext {
        ProcurementBudgetContext {
            allocations: vec![ProcurementBudgetAllocatedFact {
                budget_id: id(1),
                currency: "BRL".to_owned(),
                allocated_amount_cents: 100_000,
                allocated_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            }],
            reservations: vec![ProcurementBudgetReservedFact {
                reservation_id: id(10),
                budget_id: id(1),
                rfq_id: id(90),
                supplier_id: id(91),
                currency: "BRL".to_owned(),
                reserved_amount_cents: 25_000,
                reserved_at: Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(),
            }],
        }
    }

    fn reserve(
        amount: i64,
        currency: &str,
        current: &ProcurementBudgetContext,
    ) -> Result<ProcurementBudgetReservedFact, BudgetReservationRejection> {
        decide_reserve_procurement_budget(
            &super::BudgetReservationCommand {
                budget_id: id(1),
                rfq_id: id(2),
                supplier_id: id(3),
                requested_amount_cents: amount,
                requested_currency: currency.to_owned(),
                reservation_id: id(4),
                reserved_at: Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap(),
            },
            current,
        )
    }

    #[test]
    fn reserves_only_the_available_amount_and_emits_a_typed_fact() {
        let result = reserve(75_000, "BRL", &context()).unwrap();
        assert_eq!(result.budget_id, id(1));
        assert_eq!(result.rfq_id, id(2));
        assert_eq!(result.reserved_amount_cents, 75_000);
    }

    #[test]
    fn rejects_insufficient_available_balance() {
        assert_eq!(
            reserve(75_001, "BRL", &context()),
            Err(BudgetReservationRejection::InsufficientFunds)
        );
    }

    #[test]
    fn rejects_missing_allocation_and_duplicate_allocations() {
        let mut missing = context();
        missing.allocations.clear();
        assert_eq!(
            reserve(1, "BRL", &missing),
            Err(BudgetReservationRejection::BudgetNotFound)
        );

        let mut duplicate = context();
        duplicate.allocations.push(duplicate.allocations[0].clone());
        assert_eq!(
            reserve(1, "BRL", &duplicate),
            Err(BudgetReservationRejection::DuplicateBudgetAllocation)
        );
    }

    #[test]
    fn rejects_currency_mismatch_and_historical_duplicate_even_for_another_supplier() {
        assert_eq!(
            reserve(1, "USD", &context()),
            Err(BudgetReservationRejection::CurrencyMismatch)
        );

        let mut duplicate = context();
        duplicate.reservations[0].rfq_id = id(2);
        duplicate.reservations[0].supplier_id = id(999);
        assert_eq!(
            reserve(1, "BRL", &duplicate),
            Err(BudgetReservationRejection::AlreadyReservedForRfq)
        );
    }

    #[test]
    fn rejects_invalid_currency_and_reused_reservation_id() {
        assert_eq!(
            reserve(1, "brl", &context()),
            Err(BudgetReservationRejection::InvalidCurrency)
        );
        let mut reused = context();
        reused.reservations[0].reservation_id = id(4);
        assert_eq!(
            reserve(1, "BRL", &reused),
            Err(BudgetReservationRejection::ReservationIdentityAlreadyUsed)
        );
    }

    #[test]
    fn checked_sum_rejects_overflow_and_overdrawn_context() {
        let mut overflow = context();
        overflow.reservations = vec![
            ProcurementBudgetReservedFact {
                reservation_id: id(10),
                budget_id: id(1),
                rfq_id: id(91),
                supplier_id: id(92),
                currency: "BRL".to_owned(),
                reserved_amount_cents: i64::MAX,
                reserved_at: Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(),
            },
            ProcurementBudgetReservedFact {
                reservation_id: id(11),
                budget_id: id(1),
                rfq_id: id(92),
                supplier_id: id(93),
                currency: "BRL".to_owned(),
                reserved_amount_cents: 1,
                reserved_at: Utc.with_ymd_and_hms(2026, 1, 3, 0, 0, 0).unwrap(),
            },
        ];
        assert_eq!(
            reserve(1, "BRL", &overflow),
            Err(BudgetReservationRejection::ReservationTotalOverflow)
        );

        let mut overdrawn = context();
        overdrawn.reservations[0].reserved_amount_cents = 100_001;
        assert_eq!(
            reserve(1, "BRL", &overdrawn),
            Err(BudgetReservationRejection::BudgetAlreadyOverdrawn)
        );
    }
}
