use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

pub use crate::domain::{
    procurement_budget::{BudgetReservationRejection, ProcurementBudgetContext},
    purchase_order::PurchaseOrderRejection,
    sourcing::{AwardedSupplierQuote, SourcingRejection},
};

use crate::{
    amri::{
        FactId, FactTypeId, InteractionDefinition, InteractionDefinitionId, ParticipantDefinition,
        ParticipantId, PreparedOutcome, ProposedFact,
    },
    domain::{
        procurement_budget::{BudgetReservationCommand, decide_reserve_procurement_budget},
        purchase_order::decide_issue_purchase_order,
        sourcing::{AwardQuoteSelection, SourcingContext, decide_award_supplier_quote},
    },
    events::{ProcurementBudgetReservedFact, PurchaseOrderCreatedV2Fact, RfqAwardedFact},
};

pub const PROCUREMENT_COMMIT_INTERACTION_ID: &str = "supply-chain.procurement-commit";
pub const PROCUREMENT_COMMIT_INTERACTION_VERSION: u32 = 1;
pub const AWARD_PARTICIPANT_ID: &str = "sourcing.award-supplier-quote";
pub const RESERVE_BUDGET_PARTICIPANT_ID: &str = "procurement.budget.reserve";
pub const ISSUE_PO_PARTICIPANT_ID: &str = "procurement.purchase-order.issue";

const AWARD_FACT_TYPE: &str = "supply-chain.rfq.awarded";
const BUDGET_RESERVED_FACT_TYPE: &str = "supply-chain.procurement-budget.reserved";
const PO_CREATED_FACT_TYPE: &str = "supply-chain.purchase-order.created.v2";

/// Buyer-reviewed input for one bounded procurement commit interaction.
///
/// `expected_amount_cents` and `currency` are the values the buyer confirmed from the quote
/// review. Award validates the quote itself; the independent budget participant reserves this
/// reviewed amount; Issue PO later proves both outputs agree exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcurementCommitRequest {
    selection: AwardQuoteSelection,
    budget_id: Uuid,
    expected_amount_cents: i64,
    currency: String,
}

impl ProcurementCommitRequest {
    pub fn new(
        selection: AwardQuoteSelection,
        budget_id: Uuid,
        expected_amount_cents: i64,
        currency: impl Into<String>,
    ) -> Self {
        Self {
            selection,
            budget_id,
            expected_amount_cents,
            currency: currency.into(),
        }
    }

    pub const fn selection(&self) -> &AwardQuoteSelection {
        &self.selection
    }
    pub const fn budget_id(&self) -> Uuid {
        self.budget_id
    }
    pub const fn expected_amount_cents(&self) -> i64 {
        self.expected_amount_cents
    }
    pub fn currency(&self) -> &str {
        &self.currency
    }
}

/// IDs are generated once by the Application factory and retained with the logical execution.
/// Fact IDs are deterministically derived from the InteractionId, so a CCC retry can reuse the
/// same values without calling IdProvider again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcurementCommitIds {
    pub award_id: Uuid,
    pub reservation_id: Uuid,
    pub purchase_order_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcurementCommitOutput {
    AwardedSupplierQuote(Box<AwardedSupplierQuote>),
    BudgetReserved(Box<ProcurementBudgetReservedFact>),
    PurchaseOrderCreated(Box<PurchaseOrderCreatedV2Fact>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcurementCommitFact {
    RfqAwarded(Box<RfqAwardedFact>),
    BudgetReserved(Box<ProcurementBudgetReservedFact>),
    PurchaseOrderCreated(Box<PurchaseOrderCreatedV2Fact>),
}

pub type PreparedProcurementCommitOutcome =
    PreparedOutcome<ProcurementCommitOutput, ProcurementCommitFact>;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProcurementCommitParticipantRejection {
    #[error(transparent)]
    Award(#[from] SourcingRejection),
    #[error(transparent)]
    Budget(#[from] BudgetReservationRejection),
    #[error(transparent)]
    PurchaseOrder(#[from] PurchaseOrderRejection),
}

fn participant_id(value: &str) -> ParticipantId {
    ParticipantId::new(value).expect("participant IDs are static valid constants")
}

fn fact_type_id(value: &str) -> FactTypeId {
    FactTypeId::new(value).expect("fact type IDs are static valid constants")
}

fn fact_id(interaction_id: &str, suffix: &str) -> FactId {
    FactId::new(format!("{interaction_id}:{suffix}"))
        .expect("InteractionId and static suffix form a valid FactId")
}

pub fn procurement_commit_interaction_definition() -> InteractionDefinition {
    let award = participant_id(AWARD_PARTICIPANT_ID);
    let reserve = participant_id(RESERVE_BUDGET_PARTICIPANT_ID);
    let issue_po = participant_id(ISSUE_PO_PARTICIPANT_ID);

    InteractionDefinition::new(
        InteractionDefinitionId::new(PROCUREMENT_COMMIT_INTERACTION_ID)
            .expect("interaction definition ID is valid"),
        NonZeroU32::new(PROCUREMENT_COMMIT_INTERACTION_VERSION)
            .expect("interaction definition version is non-zero"),
        vec![
            ParticipantDefinition::new(award, Vec::new(), vec![fact_type_id(AWARD_FACT_TYPE)]),
            ParticipantDefinition::new(
                reserve,
                Vec::new(),
                vec![fact_type_id(BUDGET_RESERVED_FACT_TYPE)],
            ),
            ParticipantDefinition::new(
                issue_po,
                vec![
                    participant_id(AWARD_PARTICIPANT_ID),
                    participant_id(RESERVE_BUDGET_PARTICIPANT_ID),
                ],
                vec![fact_type_id(PO_CREATED_FACT_TYPE)],
            ),
        ],
    )
    .expect("static Procurement Commit participant graph must be valid")
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AwardSupplierQuoteInteractionParticipant;

impl AwardSupplierQuoteInteractionParticipant {
    pub fn id() -> ParticipantId {
        participant_id(AWARD_PARTICIPANT_ID)
    }

    pub fn prepare(
        &self,
        request: &ProcurementCommitRequest,
        context: &SourcingContext,
        awarded_at: DateTime<Utc>,
        ids: ProcurementCommitIds,
        interaction_id: &str,
    ) -> Result<PreparedProcurementCommitOutcome, ProcurementCommitParticipantRejection> {
        let awarded =
            decide_award_supplier_quote(request.selection(), context, awarded_at, ids.award_id)?;
        let award_fact = awarded.award_fact().clone();
        Ok(PreparedOutcome::new(
            Self::id(),
            ProcurementCommitOutput::AwardedSupplierQuote(Box::new(awarded)),
            vec![ProposedFact::new(
                fact_id(interaction_id, "rfq-awarded"),
                fact_type_id(AWARD_FACT_TYPE),
                ProcurementCommitFact::RfqAwarded(Box::new(award_fact)),
            )],
        ))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReserveProcurementBudgetParticipant;

impl ReserveProcurementBudgetParticipant {
    pub fn id() -> ParticipantId {
        participant_id(RESERVE_BUDGET_PARTICIPANT_ID)
    }

    pub fn prepare(
        &self,
        request: &ProcurementCommitRequest,
        context: &ProcurementBudgetContext,
        reserved_at: DateTime<Utc>,
        ids: ProcurementCommitIds,
        interaction_id: &str,
    ) -> Result<PreparedProcurementCommitOutcome, ProcurementCommitParticipantRejection> {
        let command = BudgetReservationCommand {
            budget_id: request.budget_id(),
            rfq_id: request.selection().rfq_id(),
            supplier_id: request.selection().supplier_id(),
            requested_amount_cents: request.expected_amount_cents(),
            requested_currency: request.currency().to_owned(),
            reservation_id: ids.reservation_id,
            reserved_at,
        };
        let reservation = decide_reserve_procurement_budget(&command, context)?;
        Ok(PreparedOutcome::new(
            Self::id(),
            ProcurementCommitOutput::BudgetReserved(Box::new(reservation.clone())),
            vec![ProposedFact::new(
                fact_id(interaction_id, "budget-reserved"),
                fact_type_id(BUDGET_RESERVED_FACT_TYPE),
                ProcurementCommitFact::BudgetReserved(Box::new(reservation)),
            )],
        ))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IssuePurchaseOrderParticipant;

impl IssuePurchaseOrderParticipant {
    pub fn id() -> ParticipantId {
        participant_id(ISSUE_PO_PARTICIPANT_ID)
    }

    pub fn prepare(
        &self,
        awarded: &AwardedSupplierQuote,
        reservation: &ProcurementBudgetReservedFact,
        purchase_order_id: Uuid,
        created_at: DateTime<Utc>,
        interaction_id: &str,
    ) -> Result<PreparedProcurementCommitOutcome, ProcurementCommitParticipantRejection> {
        let purchase_order =
            decide_issue_purchase_order(awarded, reservation, purchase_order_id, created_at)?;
        Ok(PreparedOutcome::new(
            Self::id(),
            ProcurementCommitOutput::PurchaseOrderCreated(Box::new(purchase_order.clone())),
            vec![ProposedFact::new(
                fact_id(interaction_id, "purchase-order-created-v2"),
                fact_type_id(PO_CREATED_FACT_TYPE),
                ProcurementCommitFact::PurchaseOrderCreated(Box::new(purchase_order)),
            )],
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        AWARD_PARTICIPANT_ID, ISSUE_PO_PARTICIPANT_ID, RESERVE_BUDGET_PARTICIPANT_ID,
        procurement_commit_interaction_definition,
    };

    #[test]
    fn graph_has_two_independent_preparations_and_po_depends_on_both() {
        let definition = procurement_commit_interaction_definition();
        assert_eq!(definition.execution_order().len(), 3);
        let award = definition
            .participant(&super::participant_id(AWARD_PARTICIPANT_ID))
            .unwrap();
        let reserve = definition
            .participant(&super::participant_id(RESERVE_BUDGET_PARTICIPANT_ID))
            .unwrap();
        let po = definition
            .participant(&super::participant_id(ISSUE_PO_PARTICIPANT_ID))
            .unwrap();
        assert!(award.dependencies().is_empty());
        assert!(reserve.dependencies().is_empty());
        assert_eq!(po.dependencies().len(), 2);
        assert_eq!(
            definition
                .execution_order()
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .len(),
            3,
        );
    }

    #[test]
    fn stable_topological_order_places_independent_budget_and_award_before_po() {
        let order = procurement_commit_interaction_definition()
            .execution_order()
            .to_vec();
        assert_eq!(order[0].as_str(), RESERVE_BUDGET_PARTICIPANT_ID);
        assert_eq!(order[1].as_str(), AWARD_PARTICIPANT_ID);
        assert_eq!(order[2].as_str(), ISSUE_PO_PARTICIPANT_ID);
    }
}
