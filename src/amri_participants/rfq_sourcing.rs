use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::amri::{
    FactId, FactTypeId, InteractionDefinition, InteractionDefinitionId, ParticipantDefinition,
    ParticipantId, PreparedOutcome, ProposedFact,
};
use crate::capabilities::close_rfq::CloseRfqRequest;
use crate::domain::rfq_lifecycle::decide_close_rfq;
pub use crate::domain::rfq_lifecycle::{RfqLifecycleContext, RfqLifecycleRejection};
pub use crate::domain::sourcing::{
    AwardQuoteSelection, AwardedSupplierQuote, SourcingContext, SourcingRejection,
};
use crate::domain::sourcing::{decide_award_supplier_quote, decide_create_purchase_order};
use crate::events::{PurchaseOrderCreatedFact, RfqAwardedFact, RfqClosedFact};

const CLOSE_PARTICIPANT_ID: &str = "rfq.close";
const AWARD_PARTICIPANT_ID: &str = "rfq.award-supplier-quote";
const CREATE_PO_PARTICIPANT_ID: &str = "rfq.create-purchase-order";
// Stable AMRI semantic identifiers. Stage 7's FACTSTR adapter must map these explicitly
// to the versioned storage event types; these strings are not serialized FACTSTR names.
const CLOSE_FACT_TYPE: &str = "supply-chain.rfq.closed";
const AWARD_FACT_TYPE: &str = "supply-chain.rfq.awarded";
const CREATE_PO_FACT_TYPE: &str = "supply-chain.purchase-order.created";
const INTERACTION_DEFINITION_ID: &str = "supply-chain.rfq-sourcing";
const INTERACTION_DEFINITION_VERSION: u32 = 1;

/// Typed payloads proposed by the three Supply Chain participants.
/// These are in-memory proposals, not persisted events or serialization envelopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RfqSourcingFact {
    RfqClosed(RfqClosedFact),
    RfqAwarded(RfqAwardedFact),
    PurchaseOrderCreated(PurchaseOrderCreatedFact),
}

/// Typed hand-off returned by each participant.
/// Only the award output is consumed by another participant in Stage 4B.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RfqParticipantOutput {
    RfqClosed(RfqClosedFact),
    AwardedSupplierQuote(AwardedSupplierQuote),
    PurchaseOrderCreated(PurchaseOrderCreatedFact),
}

pub type PreparedRfqOutcome = PreparedOutcome<RfqParticipantOutput, RfqSourcingFact>;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PreparedParticipantRejection {
    #[error("RFQ lifecycle decision rejected: {0:?}")]
    RfqLifecycle(RfqLifecycleRejection),
    #[error(transparent)]
    Sourcing(#[from] SourcingRejection),
}

fn participant_id(value: &str) -> ParticipantId {
    ParticipantId::new(value).expect("the participant identifiers are compile-time constants")
}

fn fact_type_id(value: &str) -> FactTypeId {
    FactTypeId::new(value).expect("the fact type identifiers are compile-time constants")
}

fn close_participant_id() -> ParticipantId {
    participant_id(CLOSE_PARTICIPANT_ID)
}

fn award_participant_id() -> ParticipantId {
    participant_id(AWARD_PARTICIPANT_ID)
}

fn create_po_participant_id() -> ParticipantId {
    participant_id(CREATE_PO_PARTICIPANT_ID)
}

/// Declarative Stage 4B participant graph. This is metadata only; Stage 5 owns execution.
/// Close and Award both decide from the same pre-interaction snapshot and are independent.
/// Create PO consumes the sealed hand-off returned by Award and therefore depends on it.
pub fn interaction_definition() -> InteractionDefinition {
    let close_id = close_participant_id();
    let award_id = award_participant_id();
    let create_po_id = create_po_participant_id();

    InteractionDefinition::new(
        InteractionDefinitionId::new(INTERACTION_DEFINITION_ID)
            .expect("the interaction definition identifier is valid"),
        NonZeroU32::new(INTERACTION_DEFINITION_VERSION)
            .expect("the interaction definition version is non-zero"),
        vec![
            ParticipantDefinition::new(close_id, Vec::new(), vec![fact_type_id(CLOSE_FACT_TYPE)]),
            ParticipantDefinition::new(
                award_id.clone(),
                Vec::new(),
                vec![fact_type_id(AWARD_FACT_TYPE)],
            ),
            ParticipantDefinition::new(
                create_po_id,
                vec![award_id],
                vec![fact_type_id(CREATE_PO_FACT_TYPE)],
            ),
        ],
    )
    .expect("the static Stage 4B participant graph must be valid")
}

/// Prepared adapter for the autonomous CloseRfq capability's shared Functional Core.
/// The context and timestamp are supplied by the caller; this adapter has no providers.
#[derive(Debug, Clone, Copy, Default)]
pub struct CloseRfqParticipant;

impl CloseRfqParticipant {
    pub fn id() -> ParticipantId {
        close_participant_id()
    }

    pub fn prepare(
        &self,
        request: CloseRfqRequest,
        context: &RfqLifecycleContext,
        closed_at: DateTime<Utc>,
        fact_id: FactId,
    ) -> Result<PreparedRfqOutcome, PreparedParticipantRejection> {
        let fact = decide_close_rfq(request.rfq_id, context, closed_at)
            .map_err(PreparedParticipantRejection::RfqLifecycle)?;

        Ok(PreparedOutcome::new(
            Self::id(),
            RfqParticipantOutput::RfqClosed(fact.clone()),
            vec![ProposedFact::new(
                fact_id,
                fact_type_id(CLOSE_FACT_TYPE),
                RfqSourcingFact::RfqClosed(fact),
            )],
        ))
    }
}

/// Prepared adapter for the supplier-award capability's shared Functional Core.
/// It returns a sealed hand-off containing the award and the validated source quote.
#[derive(Debug, Clone, Copy, Default)]
pub struct AwardSupplierQuoteParticipant;

impl AwardSupplierQuoteParticipant {
    pub fn id() -> ParticipantId {
        award_participant_id()
    }

    pub fn prepare(
        &self,
        selection: &AwardQuoteSelection,
        context: &SourcingContext,
        awarded_at: DateTime<Utc>,
        award_id: Uuid,
        fact_id: FactId,
    ) -> Result<PreparedRfqOutcome, PreparedParticipantRejection> {
        let awarded = decide_award_supplier_quote(selection, context, awarded_at, award_id)?;
        let award_fact = awarded.award_fact().clone();

        Ok(PreparedOutcome::new(
            Self::id(),
            RfqParticipantOutput::AwardedSupplierQuote(awarded),
            vec![ProposedFact::new(
                fact_id,
                fact_type_id(AWARD_FACT_TYPE),
                RfqSourcingFact::RfqAwarded(award_fact),
            )],
        ))
    }
}

/// Prepared adapter for purchase-order creation. It accepts only the sealed award hand-off.
#[derive(Debug, Clone, Copy, Default)]
pub struct CreatePurchaseOrderParticipant;

impl CreatePurchaseOrderParticipant {
    pub fn id() -> ParticipantId {
        create_po_participant_id()
    }

    pub fn prepare(
        &self,
        awarded: &AwardedSupplierQuote,
        purchase_order_id: Uuid,
        created_at: DateTime<Utc>,
        fact_id: FactId,
    ) -> PreparedRfqOutcome {
        let fact = decide_create_purchase_order(awarded, purchase_order_id, created_at);

        PreparedOutcome::new(
            Self::id(),
            RfqParticipantOutput::PurchaseOrderCreated(fact.clone()),
            vec![ProposedFact::new(
                fact_id,
                fact_type_id(CREATE_PO_FACT_TYPE),
                RfqSourcingFact::PurchaseOrderCreated(fact),
            )],
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};
    use uuid::Uuid;

    use crate::amri::{FactId, FactTypeId, ParticipantId};
    use crate::capabilities::close_rfq::CloseRfqRequest;
    use crate::domain::rfq_lifecycle::{RfqLifecycleContext, RfqLifecycleRejection};
    use crate::domain::sourcing::{AwardQuoteSelection, SourcingContext, SourcingRejection};
    use crate::events::{
        QuotedItemFact, RfqItemFact, RfqOpenedFact, SupplierQuoteSubmittedFact, SupplierStatus,
    };

    use super::{
        AwardSupplierQuoteParticipant, CloseRfqParticipant, CreatePurchaseOrderParticipant,
        PreparedParticipantRejection, RfqParticipantOutput, RfqSourcingFact,
        interaction_definition,
    };

    fn fact_id(value: &str) -> FactId {
        FactId::new(value).unwrap()
    }

    fn decision_time() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap()
    }

    fn selection() -> AwardQuoteSelection {
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
            reference: "RFQ-STAGE4B-1".to_owned(),
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

    fn sourcing_context() -> SourcingContext {
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
    fn close_participant_prepares_one_typed_fact_and_has_no_write_port() {
        let rfq_id = Uuid::from_u128(1);
        let now = decision_time();
        let expected_fact_id = fact_id("fact.close.1");
        let outcome = CloseRfqParticipant
            .prepare(
                CloseRfqRequest { rfq_id },
                &RfqLifecycleContext::new(true, false),
                now,
                expected_fact_id.clone(),
            )
            .unwrap();

        assert_eq!(outcome.participant_id(), &CloseRfqParticipant::id());
        assert_eq!(outcome.facts().len(), 1);
        assert_eq!(outcome.facts()[0].id(), &expected_fact_id);
        assert_eq!(
            outcome.facts()[0].fact_type(),
            &FactTypeId::new("supply-chain.rfq.closed").unwrap()
        );
        match outcome.output() {
            RfqParticipantOutput::RfqClosed(fact) => {
                assert_eq!(fact.rfq_id, rfq_id);
                assert_eq!(fact.closed_at, now);
            }
            other => panic!("unexpected close participant output: {other:?}"),
        }
        assert!(matches!(
            outcome.facts()[0].payload(),
            RfqSourcingFact::RfqClosed(_)
        ));
    }

    #[test]
    fn close_participant_preserves_the_shared_domain_rejection() {
        let result = CloseRfqParticipant.prepare(
            CloseRfqRequest {
                rfq_id: Uuid::from_u128(1),
            },
            &RfqLifecycleContext::new(false, false),
            decision_time(),
            fact_id("fact.close.rejected"),
        );
        assert_eq!(
            result.unwrap_err(),
            PreparedParticipantRejection::RfqLifecycle(RfqLifecycleRejection::RfqNotFound)
        );
    }

    #[test]
    fn award_participant_prepares_the_validated_award_handoff() {
        let now = decision_time();
        let expected_fact_id = fact_id("fact.award.1");
        let outcome = AwardSupplierQuoteParticipant
            .prepare(
                &selection(),
                &sourcing_context(),
                now,
                Uuid::from_u128(10),
                expected_fact_id.clone(),
            )
            .unwrap();

        assert_eq!(
            outcome.participant_id(),
            &AwardSupplierQuoteParticipant::id()
        );
        assert_eq!(outcome.facts().len(), 1);
        assert_eq!(outcome.facts()[0].id(), &expected_fact_id);
        assert_eq!(
            outcome.facts()[0].fact_type(),
            &FactTypeId::new("supply-chain.rfq.awarded").unwrap()
        );
        match outcome.output() {
            RfqParticipantOutput::AwardedSupplierQuote(awarded) => {
                assert_eq!(awarded.award_fact().award_id, Uuid::from_u128(10));
                assert_eq!(awarded.award_fact().awarded_at, now);
                assert_eq!(awarded.source_quote(), &quote());
            }
            other => panic!("unexpected award participant output: {other:?}"),
        }
        assert!(matches!(
            outcome.facts()[0].payload(),
            RfqSourcingFact::RfqAwarded(_)
        ));
    }

    #[test]
    fn award_participant_preserves_sourcing_rejection_without_proposing_facts() {
        let mut context = sourcing_context();
        context.supplier_status = Some(SupplierStatus::Blocked);
        let result = AwardSupplierQuoteParticipant.prepare(
            &selection(),
            &context,
            decision_time(),
            Uuid::from_u128(10),
            fact_id("fact.award.rejected"),
        );

        assert_eq!(
            result.unwrap_err(),
            PreparedParticipantRejection::Sourcing(SourcingRejection::SupplierNotEligible)
        );
    }

    #[test]
    fn purchase_order_participant_consumes_the_sealed_award_handoff() {
        let award_outcome = AwardSupplierQuoteParticipant
            .prepare(
                &selection(),
                &sourcing_context(),
                decision_time(),
                Uuid::from_u128(10),
                fact_id("fact.award.for-po"),
            )
            .unwrap();
        let awarded = match award_outcome.output() {
            RfqParticipantOutput::AwardedSupplierQuote(value) => value,
            other => panic!("unexpected award participant output: {other:?}"),
        };

        let now = decision_time();
        let expected_fact_id = fact_id("fact.po.1");
        let outcome = CreatePurchaseOrderParticipant.prepare(
            awarded,
            Uuid::from_u128(11),
            now,
            expected_fact_id.clone(),
        );

        assert_eq!(
            outcome.participant_id(),
            &CreatePurchaseOrderParticipant::id()
        );
        assert_eq!(outcome.facts().len(), 1);
        assert_eq!(outcome.facts()[0].id(), &expected_fact_id);
        assert_eq!(
            outcome.facts()[0].fact_type(),
            &FactTypeId::new("supply-chain.purchase-order.created").unwrap()
        );
        match outcome.output() {
            RfqParticipantOutput::PurchaseOrderCreated(fact) => {
                assert_eq!(fact.purchase_order_id, Uuid::from_u128(11));
                assert_eq!(fact.rfq_id, Uuid::from_u128(1));
                assert_eq!(fact.supplier_id, Uuid::from_u128(2));
                assert_eq!(fact.source_quote_id, Uuid::from_u128(3));
                assert_eq!(fact.currency, "BRL");
                assert_eq!(fact.total_amount_cents, 500);
                assert_eq!(fact.delivery_days, 5);
                assert_eq!(fact.items, quote().items);
                assert_eq!(fact.created_at, now);
            }
            other => panic!("unexpected purchase-order participant output: {other:?}"),
        }
        assert!(matches!(
            outcome.facts()[0].payload(),
            RfqSourcingFact::PurchaseOrderCreated(_)
        ));
    }

    #[test]
    fn award_and_close_are_independent_but_purchase_order_depends_on_award() {
        let definition = interaction_definition();
        let close_id = ParticipantId::new("rfq.close").unwrap();
        let award_id = ParticipantId::new("rfq.award-supplier-quote").unwrap();
        let create_po_id = ParticipantId::new("rfq.create-purchase-order").unwrap();

        assert!(
            definition
                .participant(&close_id)
                .unwrap()
                .dependencies()
                .is_empty()
        );
        assert!(
            definition
                .participant(&award_id)
                .unwrap()
                .dependencies()
                .is_empty()
        );
        assert_eq!(
            definition
                .participant(&create_po_id)
                .unwrap()
                .dependencies(),
            &[award_id]
        );
        assert_eq!(definition.execution_order().len(), 3);
    }
}
