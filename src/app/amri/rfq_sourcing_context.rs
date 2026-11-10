use serde::de::DeserializeOwned;
use serde_json::json;
use thiserror::Error;

use crate::{
    amri_participants::rfq_sourcing::{
        AwardQuoteSelection, AwardSupplierQuoteParticipant, CloseRfqParticipant,
        CreatePurchaseOrderParticipant, RfqLifecycleContext, SourcingContext,
    },
    application_state::atomic_interaction::{
        context::{
            CompositeContextSpecification, ContextClause, ContextPlanError, ContextRequirement,
        },
        state_port::{CompositeSnapshot, SnapshotFact},
    },
    events::{
        PURCHASE_ORDER_CREATED_V1, PURCHASE_ORDER_DISPATCH_REQUESTED_V1, PurchaseOrderCreatedFact,
        PurchaseOrderDispatchRequestedFact, RFQ_AWARDED_V1, RFQ_CLOSED_V1, RFQ_OPENED_V1,
        RfqAwardedFact, RfqClosedFact, RfqOpenedFact, SUPPLIER_ELIGIBILITY_CHANGED_V1,
        SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
        SupplierQuoteSubmittedFact, SupplierRegisteredFact, SupplierStatus,
    },
};

/// Both participant contexts are derived from exactly one acquired composite snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RfqSourcingContextProjection {
    pub lifecycle: RfqLifecycleContext,
    pub sourcing: SourcingContext,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextProjectionError {
    #[error("duplicate sequence number in composite snapshot: {0}")]
    DuplicateSequenceNumber(u64),
    #[error("unexpected event type `{event_type}` at sequence {sequence}")]
    UnexpectedEventType { sequence: u64, event_type: String },
    #[error("invalid payload for `{event_type}` at sequence {sequence}: {message}")]
    InvalidPayload {
        sequence: u64,
        event_type: String,
        message: String,
    },
    #[error("supplier quote count overflow")]
    SupplierQuoteCountOverflow,
}

/// Builds one deterministic union of all state requirements for the three Stage 4B participants.
///
/// All quote facts for the RFQ are selected, not only the chosen supplier's quotes:
/// the shared award decision compares the reviewed quote-set version to the maximum
/// sequence number across the complete quote set for that RFQ.
pub fn composite_context_specification(
    selection: &AwardQuoteSelection,
) -> Result<CompositeContextSpecification, ContextPlanError> {
    let rfq_opened = ContextClause::new(RFQ_OPENED_V1, json!({"rfq_id": selection.rfq_id()}))?;
    let rfq_closed = ContextClause::new(RFQ_CLOSED_V1, json!({"rfq_id": selection.rfq_id()}))?;

    let close_requirement = ContextRequirement::new(
        CloseRfqParticipant::id(),
        vec![rfq_opened.clone(), rfq_closed.clone()],
    );

    let award_requirement = ContextRequirement::new(
        AwardSupplierQuoteParticipant::id(),
        vec![
            rfq_opened,
            rfq_closed,
            ContextClause::new(
                SUPPLIER_REGISTERED_V1,
                json!({"supplier_id": selection.supplier_id()}),
            )?,
            ContextClause::new(
                SUPPLIER_ELIGIBILITY_CHANGED_V1,
                json!({"supplier_id": selection.supplier_id()}),
            )?,
            ContextClause::new(
                SUPPLIER_QUOTE_SUBMITTED_V1,
                json!({"rfq_id": selection.rfq_id()}),
            )?,
            ContextClause::new(RFQ_AWARDED_V1, json!({"rfq_id": selection.rfq_id()}))?,
            ContextClause::new(
                PURCHASE_ORDER_CREATED_V1,
                json!({"rfq_id": selection.rfq_id()}),
            )?,
            ContextClause::new(
                PURCHASE_ORDER_DISPATCH_REQUESTED_V1,
                json!({"rfq_id": selection.rfq_id()}),
            )?,
        ],
    );

    // Create PO has no independent pre-interaction read set: its input is Award's sealed hand-off.
    let create_po_requirement =
        ContextRequirement::new(CreatePurchaseOrderParticipant::id(), Vec::new());

    CompositeContextSpecification::union(&[
        close_requirement,
        award_requirement,
        create_po_requirement,
    ])
}

fn decode<T: DeserializeOwned>(fact: &SnapshotFact) -> Result<T, ContextProjectionError> {
    serde_json::from_value(fact.payload().clone()).map_err(|error| {
        ContextProjectionError::InvalidPayload {
            sequence: fact.sequence_number(),
            event_type: fact.event_type().to_owned(),
            message: error.to_string(),
        }
    })
}

/// Projects lifecycle and sourcing contexts from one already-acquired snapshot.
///
/// Records are reduced in ascending sequence order. Malformed payloads, duplicate
/// sequences and event types outside the declared read set are projection errors,
/// not business rejections. Close and Award therefore observe the same pre-interaction state.
pub fn project_snapshot(
    selection: &AwardQuoteSelection,
    snapshot: &CompositeSnapshot,
) -> Result<RfqSourcingContextProjection, ContextProjectionError> {
    let mut facts = snapshot.facts().iter().collect::<Vec<_>>();
    facts.sort_by_key(|fact| fact.sequence_number());

    for pair in facts.windows(2) {
        if pair[0].sequence_number() == pair[1].sequence_number() {
            return Err(ContextProjectionError::DuplicateSequenceNumber(
                pair[0].sequence_number(),
            ));
        }
    }

    let mut sourcing = SourcingContext::default();
    let mut latest_supplier_eligibility: Option<(u64, SupplierStatus)> = None;

    for fact in facts {
        let sequence = fact.sequence_number();
        match fact.event_type() {
            RFQ_OPENED_V1 => {
                let value: RfqOpenedFact = decode(fact)?;
                if value.rfq_id == selection.rfq_id() {
                    sourcing.rfq = Some(value);
                }
            }
            RFQ_CLOSED_V1 => {
                let value: RfqClosedFact = decode(fact)?;
                if value.rfq_id == selection.rfq_id() {
                    sourcing.closed = true;
                }
            }
            SUPPLIER_REGISTERED_V1 => {
                let value: SupplierRegisteredFact = decode(fact)?;
                if value.supplier_id == selection.supplier_id() {
                    sourcing.supplier_registered = true;
                }
            }
            SUPPLIER_ELIGIBILITY_CHANGED_V1 => {
                let value: SupplierEligibilityChangedFact = decode(fact)?;
                if value.supplier_id == selection.supplier_id()
                    && latest_supplier_eligibility
                        .as_ref()
                        .is_none_or(|(previous, _)| sequence > *previous)
                {
                    latest_supplier_eligibility = Some((sequence, value.status));
                }
            }
            SUPPLIER_QUOTE_SUBMITTED_V1 => {
                let value: SupplierQuoteSubmittedFact = decode(fact)?;
                if value.rfq_id == selection.rfq_id() {
                    sourcing.latest_quote_sequence_number = Some(
                        sourcing
                            .latest_quote_sequence_number
                            .map_or(sequence, |previous| previous.max(sequence)),
                    );
                    if value.supplier_id == selection.supplier_id() {
                        sourcing.supplier_quote_count = sourcing
                            .supplier_quote_count
                            .checked_add(1)
                            .ok_or(ContextProjectionError::SupplierQuoteCountOverflow)?;
                        if value.quote_id == selection.quote_id() {
                            sourcing.selected_quote = Some(value);
                        }
                    }
                }
            }
            RFQ_AWARDED_V1 => {
                let value: RfqAwardedFact = decode(fact)?;
                if value.rfq_id == selection.rfq_id() {
                    sourcing.finalized = true;
                }
            }
            PURCHASE_ORDER_CREATED_V1 => {
                let value: PurchaseOrderCreatedFact = decode(fact)?;
                if value.rfq_id == selection.rfq_id() {
                    sourcing.finalized = true;
                }
            }
            PURCHASE_ORDER_DISPATCH_REQUESTED_V1 => {
                let value: PurchaseOrderDispatchRequestedFact = decode(fact)?;
                if value.rfq_id == selection.rfq_id() {
                    sourcing.finalized = true;
                }
            }
            other => {
                return Err(ContextProjectionError::UnexpectedEventType {
                    sequence,
                    event_type: other.to_owned(),
                });
            }
        }
    }

    sourcing.supplier_status = latest_supplier_eligibility.map(|(_, status)| status);
    let lifecycle = RfqLifecycleContext::new(sourcing.rfq.is_some(), sourcing.closed);
    Ok(RfqSourcingContextProjection {
        lifecycle,
        sourcing,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::{
        amri::Fingerprint,
        amri_participants::rfq_sourcing::{AwardQuoteSelection, RfqLifecycleContext},
        application_state::atomic_interaction::state_port::{CompositeSnapshot, SnapshotFact},
        events::{
            RFQ_AWARDED_V1, RFQ_CLOSED_V1, RFQ_OPENED_V1, SUPPLIER_ELIGIBILITY_CHANGED_V1,
            SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierStatus,
        },
    };

    use super::{ContextProjectionError, composite_context_specification, project_snapshot};

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }
    fn selection(version: u64) -> AwardQuoteSelection {
        AwardQuoteSelection::new(id(1), id(2), id(3), version)
    }
    fn fact(sequence: u64, event_type: &str, payload: Value) -> SnapshotFact {
        SnapshotFact::new(sequence, event_type, payload)
    }
    fn snapshot(facts: Vec<SnapshotFact>) -> CompositeSnapshot {
        CompositeSnapshot::new(facts, Fingerprint::from_bytes([7; 32]))
    }
    fn opened() -> Value {
        json!({
            "rfq_id": id(1), "reference": "RFQ-STAGE5A-1", "currency": "BRL",
            "opened_at": "2026-01-01T10:00:00Z",
            "items": [{"id": id(4), "sku": "SKU-1", "description": "Item 1", "required_quantity": 2}]
        })
    }
    fn quote(quote_id: Uuid, supplier_id: Uuid) -> Value {
        json!({
            "quote_id": quote_id, "rfq_id": id(1), "supplier_id": supplier_id,
            "currency": "BRL", "total_amount_cents": 500, "delivery_days": 5,
            "valid_until": "2026-12-31", "submitted_at": "2026-01-02T10:00:00Z",
            "items": [{"rfq_item_id": id(4), "quantity": 2, "unit_price_cents": 250, "line_total_cents": 500}]
        })
    }

    #[test]
    fn specification_unions_three_participants_and_eight_unique_clauses() {
        let spec = composite_context_specification(&selection(6)).unwrap();
        assert_eq!(spec.participants().len(), 3);
        assert_eq!(spec.clauses().len(), 8);
    }

    #[test]
    fn both_contexts_are_projected_from_one_ordered_snapshot() {
        let facts = vec![
            fact(6, SUPPLIER_QUOTE_SUBMITTED_V1, quote(id(3), id(2))),
            fact(1, RFQ_OPENED_V1, opened()),
            fact(
                2,
                SUPPLIER_REGISTERED_V1,
                json!({"supplier_id":id(2),"reference":"SUP-1","name":"Supplier 1","registered_at":"2026-01-01T10:00:00Z"}),
            ),
            fact(
                3,
                SUPPLIER_ELIGIBILITY_CHANGED_V1,
                json!({"supplier_id":id(2),"status":"blocked","changed_at":"2026-01-02T10:00:00Z"}),
            ),
            fact(
                4,
                SUPPLIER_ELIGIBILITY_CHANGED_V1,
                json!({"supplier_id":id(2),"status":"eligible","changed_at":"2026-01-03T10:00:00Z"}),
            ),
            fact(7, SUPPLIER_QUOTE_SUBMITTED_V1, quote(id(5), id(9))),
        ];
        let projection = project_snapshot(&selection(7), &snapshot(facts)).unwrap();
        assert_eq!(projection.lifecycle, RfqLifecycleContext::new(true, false));
        assert!(projection.sourcing.rfq.is_some());
        assert!(!projection.sourcing.closed);
        assert!(projection.sourcing.supplier_registered);
        assert_eq!(
            projection.sourcing.supplier_status,
            Some(SupplierStatus::Eligible)
        );
        assert_eq!(projection.sourcing.supplier_quote_count, 1);
        assert_eq!(projection.sourcing.latest_quote_sequence_number, Some(7));
        assert_eq!(
            projection
                .sourcing
                .selected_quote
                .as_ref()
                .map(|q| q.quote_id),
            Some(id(3))
        );
    }

    #[test]
    fn close_and_finalization_are_projected_from_the_pre_interaction_snapshot() {
        let facts = vec![
            fact(1, RFQ_OPENED_V1, opened()),
            fact(
                2,
                RFQ_CLOSED_V1,
                json!({"rfq_id":id(1),"closed_at":"2026-02-01T10:00:00Z"}),
            ),
            fact(
                3,
                RFQ_AWARDED_V1,
                json!({
                    "award_id":id(10), "rfq_id":id(1), "quote_id":id(3), "supplier_id":id(2),
                    "awarded_at":"2026-02-02T10:00:00Z", "currency":"BRL",
                    "total_amount_cents":500, "delivery_days":5, "valid_until":"2026-12-31"
                }),
            ),
        ];
        let projection = project_snapshot(&selection(0), &snapshot(facts)).unwrap();
        assert_eq!(projection.lifecycle, RfqLifecycleContext::new(true, true));
        assert!(projection.sourcing.closed);
        assert!(projection.sourcing.finalized);
    }

    #[test]
    fn malformed_payload_is_a_projection_error_not_a_business_rejection() {
        let facts = vec![fact(1, RFQ_OPENED_V1, json!(null))];
        assert!(matches!(
            project_snapshot(&selection(0), &snapshot(facts)),
            Err(ContextProjectionError::InvalidPayload { .. })
        ));
    }

    #[test]
    fn unknown_event_type_is_rejected() {
        let facts = vec![fact(1, "unlisted.event.v1", json!({"id":1}))];
        assert!(matches!(
            project_snapshot(&selection(0), &snapshot(facts)),
            Err(ContextProjectionError::UnexpectedEventType { .. })
        ));
    }

    #[test]
    fn duplicate_sequence_numbers_are_rejected() {
        let facts = vec![
            fact(1, RFQ_OPENED_V1, opened()),
            fact(
                1,
                RFQ_CLOSED_V1,
                json!({"rfq_id":id(1),"closed_at":"2026-02-01T10:00:00Z"}),
            ),
        ];
        assert_eq!(
            project_snapshot(&selection(0), &snapshot(facts)),
            Err(ContextProjectionError::DuplicateSequenceNumber(1))
        );
    }

    #[test]
    fn quote_set_selector_is_rf_q_scoped_not_supplier_scoped() {
        let spec = composite_context_specification(&selection(6)).unwrap();
        let clause = spec
            .clauses()
            .iter()
            .find(|clause| clause.event_type() == SUPPLIER_QUOTE_SUBMITTED_V1)
            .unwrap();
        assert_eq!(clause.payload_subset(), &json!({"rfq_id":id(1)}));
    }
}
