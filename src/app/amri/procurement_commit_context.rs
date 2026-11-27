use serde::de::DeserializeOwned;
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    amri_participants::procurement_commit::{
        AwardSupplierQuoteInteractionParticipant, IssuePurchaseOrderParticipant,
        ProcurementBudgetContext, ProcurementCommitRequest, ReserveProcurementBudgetParticipant,
    },
    app::amri::rfq_sourcing_context::{
        ContextProjectionError, RfqSourcingContextProjection, composite_context_specification,
        project_snapshot,
    },
    application_state::atomic_interaction::{
        context::{
            CompositeContextSpecification, ContextClause, ContextPlanError, ContextRequirement,
        },
        state_port::{CompositeSnapshot, SnapshotFact},
    },
    events::{
        PROCUREMENT_BUDGET_ALLOCATED_V1, PROCUREMENT_BUDGET_RESERVED_V1, PURCHASE_ORDER_CREATED_V2,
        ProcurementBudgetAllocatedFact, ProcurementBudgetReservedFact, PurchaseOrderCreatedV2Fact,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcurementCommitContextProjection {
    pub sourcing: RfqSourcingContextProjection,
    pub budget: ProcurementBudgetContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProcurementCommitContextError {
    #[error(transparent)]
    Specification(#[from] ContextPlanError),
    #[error(transparent)]
    Sourcing(#[from] ContextProjectionError),
    #[error("invalid budget payload for `{event_type}` at sequence {sequence}: {message}")]
    InvalidBudgetPayload {
        sequence: u64,
        event_type: String,
        message: String,
    },
    #[error("budget event at sequence {sequence} is outside the requested budget scope")]
    UnexpectedBudgetScope { sequence: u64 },
    #[error("composite snapshot contains duplicate sequence number {0}")]
    DuplicateSequenceNumber(u64),
    #[error("purchase-order v2 event at sequence {sequence} is outside the requested RFQ scope")]
    UnexpectedPurchaseOrderV2Scope { sequence: u64 },
    #[error("invalid purchase_order_created.v2 payload at sequence {sequence}: {message}")]
    InvalidPurchaseOrderV2Payload { sequence: u64, message: String },
}

/// Creates the complete read specification for the three interaction participants.
/// All source clauses are read once; the budget and sourcing projections are derived from
/// that one snapshot. The new v2 PO event is included so later awards see it as finalization.
pub fn procurement_commit_context_specification(
    request: &ProcurementCommitRequest,
) -> Result<CompositeContextSpecification, ProcurementCommitContextError> {
    let sourcing = composite_context_specification(request.selection())?;
    let mut sourcing_clauses = sourcing.clauses().to_vec();
    sourcing_clauses.push(ContextClause::new(
        PURCHASE_ORDER_CREATED_V2,
        json!({"rfq_id": request.selection().rfq_id()}),
    )?);

    let award = ContextRequirement::new(
        AwardSupplierQuoteInteractionParticipant::id(),
        sourcing_clauses,
    );
    let budget = ContextRequirement::new(
        ReserveProcurementBudgetParticipant::id(),
        vec![
            ContextClause::new(
                PROCUREMENT_BUDGET_ALLOCATED_V1,
                json!({"budget_id": request.budget_id()}),
            )?,
            ContextClause::new(
                PROCUREMENT_BUDGET_RESERVED_V1,
                json!({"budget_id": request.budget_id()}),
            )?,
        ],
    );
    let purchase_order = ContextRequirement::new(IssuePurchaseOrderParticipant::id(), Vec::new());

    Ok(CompositeContextSpecification::union(&[
        award,
        budget,
        purchase_order,
    ])?)
}

fn decode_budget<T: DeserializeOwned>(
    fact: &SnapshotFact,
) -> Result<T, ProcurementCommitContextError> {
    serde_json::from_value(fact.payload().clone()).map_err(|error| {
        ProcurementCommitContextError::InvalidBudgetPayload {
            sequence: fact.sequence_number(),
            event_type: fact.event_type().to_owned(),
            message: error.to_string(),
        }
    })
}

fn project_budget_context(
    budget_id: Uuid,
    snapshot: &CompositeSnapshot,
) -> Result<ProcurementBudgetContext, ProcurementCommitContextError> {
    let mut context = ProcurementBudgetContext::default();
    for fact in snapshot.facts() {
        match fact.event_type() {
            PROCUREMENT_BUDGET_ALLOCATED_V1 => {
                let value: ProcurementBudgetAllocatedFact = decode_budget(fact)?;
                if value.budget_id != budget_id {
                    return Err(ProcurementCommitContextError::UnexpectedBudgetScope {
                        sequence: fact.sequence_number(),
                    });
                }
                context.allocations.push(value);
            }
            PROCUREMENT_BUDGET_RESERVED_V1 => {
                let value: ProcurementBudgetReservedFact = decode_budget(fact)?;
                if value.budget_id != budget_id {
                    return Err(ProcurementCommitContextError::UnexpectedBudgetScope {
                        sequence: fact.sequence_number(),
                    });
                }
                context.reservations.push(value);
            }
            _ => {}
        }
    }
    Ok(context)
}

/// Projects sourcing, budget, and existing v2 PO state from the same composite snapshot.
pub fn project_procurement_commit_snapshot(
    request: &ProcurementCommitRequest,
    snapshot: &CompositeSnapshot,
) -> Result<ProcurementCommitContextProjection, ProcurementCommitContextError> {
    let mut ordered = snapshot.facts().iter().collect::<Vec<_>>();
    ordered.sort_by_key(|fact| fact.sequence_number());
    for pair in ordered.windows(2) {
        if pair[0].sequence_number() == pair[1].sequence_number() {
            return Err(ProcurementCommitContextError::DuplicateSequenceNumber(
                pair[0].sequence_number(),
            ));
        }
    }

    let budget = project_budget_context(request.budget_id(), snapshot)?;

    // The pre-existing sourcing projector deliberately accepts only the original sourcing
    // event vocabulary. Isolate its slice, then account for the new PO v2 schema separately.
    let sourcing_facts = snapshot
        .facts()
        .iter()
        .filter(|fact| {
            fact.event_type() != PROCUREMENT_BUDGET_ALLOCATED_V1
                && fact.event_type() != PROCUREMENT_BUDGET_RESERVED_V1
                && fact.event_type() != PURCHASE_ORDER_CREATED_V2
        })
        .cloned()
        .collect::<Vec<_>>();
    let sourcing_snapshot =
        CompositeSnapshot::new(sourcing_facts, snapshot.specification_fingerprint());
    let mut sourcing = project_snapshot(request.selection(), &sourcing_snapshot)?;

    for fact in snapshot
        .facts()
        .iter()
        .filter(|fact| fact.event_type() == PURCHASE_ORDER_CREATED_V2)
    {
        let value: PurchaseOrderCreatedV2Fact = serde_json::from_value(fact.payload().clone())
            .map_err(
                |error| ProcurementCommitContextError::InvalidPurchaseOrderV2Payload {
                    sequence: fact.sequence_number(),
                    message: error.to_string(),
                },
            )?;
        if value.rfq_id != request.selection().rfq_id() {
            return Err(
                ProcurementCommitContextError::UnexpectedPurchaseOrderV2Scope {
                    sequence: fact.sequence_number(),
                },
            );
        }
        sourcing.sourcing.finalized = true;
    }

    Ok(ProcurementCommitContextProjection { sourcing, budget })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use crate::{
        amri::Fingerprint,
        amri_participants::{
            procurement_commit::ProcurementCommitRequest, rfq_sourcing::AwardQuoteSelection,
        },
        application_state::atomic_interaction::state_port::{CompositeSnapshot, SnapshotFact},
        events::{
            PROCUREMENT_BUDGET_ALLOCATED_V1, PROCUREMENT_BUDGET_RESERVED_V1,
            PURCHASE_ORDER_CREATED_V2,
        },
    };

    use super::{
        ProcurementCommitContextError, procurement_commit_context_specification,
        project_procurement_commit_snapshot,
    };

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }
    fn request() -> ProcurementCommitRequest {
        ProcurementCommitRequest::new(
            AwardQuoteSelection::new(id(1), id(2), id(3), 4),
            id(9),
            500,
            "BRL",
        )
    }

    #[test]
    fn specification_contains_three_participants_and_budget_read_clauses() {
        let specification = procurement_commit_context_specification(&request()).unwrap();
        assert_eq!(specification.participants().len(), 3);
        assert!(
            specification
                .clauses()
                .iter()
                .any(|clause| clause.event_type() == PROCUREMENT_BUDGET_ALLOCATED_V1)
        );
        assert!(
            specification
                .clauses()
                .iter()
                .any(|clause| clause.event_type() == PROCUREMENT_BUDGET_RESERVED_V1)
        );
    }

    #[test]
    fn budget_projection_rejects_events_returned_for_a_different_budget() {
        let snapshot = CompositeSnapshot::new(
            vec![SnapshotFact::new(
                8,
                PROCUREMENT_BUDGET_ALLOCATED_V1,
                json!({"budget_id":id(99),"currency":"BRL","allocated_amount_cents":1000,"allocated_at":"2026-01-01T00:00:00Z"}),
            )],
            Fingerprint::from_bytes([7; 32]),
        );
        assert!(matches!(
            project_procurement_commit_snapshot(&request(), &snapshot),
            Err(ProcurementCommitContextError::UnexpectedBudgetScope { sequence: 8 })
        ));
    }

    fn po_v2_payload(rfq_id: Uuid) -> serde_json::Value {
        json!({
            "purchase_order_id": id(30),
            "rfq_id": rfq_id,
            "supplier_id": id(2),
            "source_quote_id": id(3),
            "budget_id": id(9),
            "budget_reservation_id": id(31),
            "currency": "BRL",
            "total_amount_cents": 500,
            "delivery_days": 5,
            "created_at": "2026-03-01T10:00:00Z",
            "items": [{
                "rfq_item_id": id(4),
                "quantity": 2,
                "unit_price_cents": 250,
                "line_total_cents": 500
            }]
        })
    }

    #[test]
    fn existing_po_v2_marks_the_rfq_as_finalized() {
        let snapshot = CompositeSnapshot::new(
            vec![SnapshotFact::new(
                12,
                PURCHASE_ORDER_CREATED_V2,
                po_v2_payload(id(1)),
            )],
            Fingerprint::from_bytes([7; 32]),
        );
        let projection = project_procurement_commit_snapshot(&request(), &snapshot).unwrap();
        assert!(projection.sourcing.sourcing.finalized);
    }

    #[test]
    fn malformed_po_v2_payload_is_rejected_including_unknown_fields() {
        let mut payload = po_v2_payload(id(1));
        payload["unexpected_field"] = json!(true);
        let snapshot = CompositeSnapshot::new(
            vec![SnapshotFact::new(13, PURCHASE_ORDER_CREATED_V2, payload)],
            Fingerprint::from_bytes([7; 32]),
        );
        assert!(matches!(
            project_procurement_commit_snapshot(&request(), &snapshot),
            Err(ProcurementCommitContextError::InvalidPurchaseOrderV2Payload { sequence: 13, .. })
        ));
    }

    #[test]
    fn po_v2_event_for_another_rfq_is_rejected() {
        let snapshot = CompositeSnapshot::new(
            vec![SnapshotFact::new(
                14,
                PURCHASE_ORDER_CREATED_V2,
                po_v2_payload(id(99)),
            )],
            Fingerprint::from_bytes([7; 32]),
        );
        assert_eq!(
            project_procurement_commit_snapshot(&request(), &snapshot),
            Err(ProcurementCommitContextError::UnexpectedPurchaseOrderV2Scope { sequence: 14 })
        );
    }
}
