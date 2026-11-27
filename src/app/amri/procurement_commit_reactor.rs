use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::{
    amri::{ExecutionContext, ParticipantId},
    amri_participants::procurement_commit::{
        AwardSupplierQuoteInteractionParticipant, IssuePurchaseOrderParticipant,
        PreparedProcurementCommitOutcome, ProcurementCommitOutput,
        ProcurementCommitParticipantRejection, ProcurementCommitRequest,
        ReserveProcurementBudgetParticipant, procurement_commit_interaction_definition,
    },
    app::amri::{
        procurement_commit_context::{
            ProcurementCommitContextError, procurement_commit_context_specification,
            project_procurement_commit_snapshot,
        },
        procurement_commit_execution::{
            ProcurementCommitExecution, procurement_commit_request_fingerprint,
        },
    },
    application_state::atomic_interaction::{CompositeContextReader, state_port::StatePortError},
};

#[derive(Debug, Error)]
pub enum ProcurementCommitReactorError {
    #[error(transparent)]
    Context(#[from] ProcurementCommitContextError),
    #[error(transparent)]
    State(#[from] StatePortError),
    #[error(transparent)]
    Participant(#[from] ProcurementCommitParticipantRejection),
    #[error("execution context belongs to another interaction definition")]
    DefinitionIdentityMismatch,
    #[error("execution context fingerprint does not match the request")]
    RequestFingerprintMismatch,
    #[error("timestamp captured in execution context cannot be represented as UTC")]
    InvalidCapturedTimestamp,
    #[error("state adapter returned a snapshot for a different context specification")]
    SnapshotSpecificationMismatch,
    #[error("prepared participant `{0}` is missing from the deterministic outcome set")]
    MissingPreparedParticipant(ParticipantId),
}

/// Result of Stage 5 preparation. The adapter's opaque token is preserved unchanged for
/// Stage 6/7; this value is not a commit plan and grants no write authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedProcurementInteraction<T> {
    execution: ExecutionContext,
    context_specification_fingerprint: crate::amri::Fingerprint,
    token: T,
    outcomes: Vec<PreparedProcurementCommitOutcome>,
}

impl<T> PreparedProcurementInteraction<T> {
    pub fn execution(&self) -> &ExecutionContext {
        &self.execution
    }
    pub const fn context_specification_fingerprint(&self) -> crate::amri::Fingerprint {
        self.context_specification_fingerprint
    }
    pub fn token(&self) -> &T {
        &self.token
    }
    pub fn outcomes(&self) -> &[PreparedProcurementCommitOutcome] {
        &self.outcomes
    }
    pub fn into_parts(
        self,
    ) -> (
        ExecutionContext,
        crate::amri::Fingerprint,
        T,
        Vec<PreparedProcurementCommitOutcome>,
    ) {
        (
            self.execution,
            self.context_specification_fingerprint,
            self.token,
            self.outcomes,
        )
    }
}

/// Read-only Stage 5 Reactor for the Procurement Commit reference interaction.
///
/// Its generic boundary is only `CompositeContextReader`; it has no reference to
/// `AtomicInteractionStatePort`, FACTSTR, EventStore, SQLx, HTTP, or a commit API.
/// The budget reservation and award are independent participants prepared from one snapshot.
/// Issue PO consumes both sealed outputs. Execution is sequential and deterministic by design.
pub struct ProcurementCommitReactor<R> {
    reader: R,
}

impl<R> ProcurementCommitReactor<R>
where
    R: CompositeContextReader,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn prepare(
        &self,
        request: &ProcurementCommitRequest,
        execution: &ProcurementCommitExecution,
    ) -> Result<PreparedProcurementInteraction<R::Token>, ProcurementCommitReactorError> {
        let definition = procurement_commit_interaction_definition();
        let identity = execution.context().identity();
        if identity.definition_id() != definition.id()
            || identity.definition_version() != definition.version()
        {
            return Err(ProcurementCommitReactorError::DefinitionIdentityMismatch);
        }
        if identity.request_fingerprint() != procurement_commit_request_fingerprint(request) {
            return Err(ProcurementCommitReactorError::RequestFingerprintMismatch);
        }

        let specification = procurement_commit_context_specification(request)?;
        let envelope = self.reader.read_composite(&specification)?;
        let (snapshot, token) = envelope.into_parts();
        if snapshot.specification_fingerprint() != specification.fingerprint() {
            return Err(ProcurementCommitReactorError::SnapshotSpecificationMismatch);
        }

        let projection = project_procurement_commit_snapshot(request, &snapshot)?;
        let timestamp = DateTime::<Utc>::from_timestamp_millis(
            i64::try_from(execution.context().captured_at_unix_ms())
                .map_err(|_| ProcurementCommitReactorError::InvalidCapturedTimestamp)?,
        )
        .ok_or(ProcurementCommitReactorError::InvalidCapturedTimestamp)?;
        let ids = execution.ids();
        let interaction_id = identity.interaction_id().as_str();

        // The two independent participants consume the same immutable projections.
        // Their order is deterministic and matches the Core's lexicographic topological order.
        let budget = ReserveProcurementBudgetParticipant.prepare(
            request,
            &projection.budget,
            timestamp,
            ids,
            interaction_id,
        )?;
        let award = AwardSupplierQuoteInteractionParticipant.prepare(
            request,
            &projection.sourcing.sourcing,
            timestamp,
            ids,
            interaction_id,
        )?;

        let awarded = match award.output() {
            ProcurementCommitOutput::AwardedSupplierQuote(value) => value.as_ref(),
            _ => {
                return Err(ProcurementCommitReactorError::MissingPreparedParticipant(
                    AwardSupplierQuoteInteractionParticipant::id(),
                ));
            }
        };
        let reservation = match budget.output() {
            ProcurementCommitOutput::BudgetReserved(value) => value.as_ref(),
            _ => {
                return Err(ProcurementCommitReactorError::MissingPreparedParticipant(
                    ReserveProcurementBudgetParticipant::id(),
                ));
            }
        };
        let issue_po = IssuePurchaseOrderParticipant.prepare(
            awarded,
            reservation,
            ids.purchase_order_id,
            timestamp,
            interaction_id,
        )?;

        let mut by_participant = BTreeMap::<ParticipantId, PreparedProcurementCommitOutcome>::new();
        by_participant.insert(budget.participant_id().clone(), budget);
        by_participant.insert(award.participant_id().clone(), award);
        by_participant.insert(issue_po.participant_id().clone(), issue_po);

        let mut outcomes = Vec::with_capacity(definition.execution_order().len());
        for participant_id in definition.execution_order() {
            let outcome = by_participant.remove(participant_id).ok_or(
                ProcurementCommitReactorError::MissingPreparedParticipant(participant_id.clone()),
            )?;
            outcomes.push(outcome);
        }

        Ok(PreparedProcurementInteraction {
            execution: execution.context().clone(),
            context_specification_fingerprint: specification.fingerprint(),
            token,
            outcomes,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use chrono::{TimeZone, Utc};
    use serde_json::to_value;
    use uuid::Uuid;

    use crate::{
        amri_participants::{
            procurement_commit::ProcurementCommitRequest, rfq_sourcing::AwardQuoteSelection,
        },
        app::amri::procurement_commit_execution::ProcurementCommitExecutionContextFactory,
        application_state::atomic_interaction::{
            CompositeContextReader,
            state_port::{CompositeSnapshot, SnapshotEnvelope, SnapshotFact, StatePortError},
        },
        events::{
            PROCUREMENT_BUDGET_ALLOCATED_V1, ProcurementBudgetAllocatedFact, QuotedItemFact,
            RFQ_OPENED_V1, RfqItemFact, RfqOpenedFact, SUPPLIER_ELIGIBILITY_CHANGED_V1,
            SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
            SupplierQuoteSubmittedFact, SupplierRegisteredFact, SupplierStatus,
        },
        providers::{ClockProvider, IdProvider},
    };

    use super::{ProcurementCommitReactor, ProcurementCommitReactorError};

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    struct CountingReader {
        facts: Vec<SnapshotFact>,
        reads: Arc<AtomicUsize>,
    }
    impl CompositeContextReader for CountingReader {
        type Token = String;
        fn read_composite(
            &self,
            spec: &crate::application_state::atomic_interaction::context::CompositeContextSpecification,
        ) -> Result<SnapshotEnvelope<Self::Token>, StatePortError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(SnapshotEnvelope::new(
                CompositeSnapshot::new(self.facts.clone(), spec.fingerprint()),
                "opaque-test-token".to_owned(),
            ))
        }
    }

    struct FixedClock;
    impl ClockProvider for FixedClock {
        fn now_utc(&self) -> chrono::DateTime<Utc> {
            Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap()
        }
    }
    struct FixedIds(AtomicUsize);
    impl IdProvider for FixedIds {
        fn new_uuid(&self) -> Uuid {
            id(100 + self.0.fetch_add(1, Ordering::SeqCst) as u128)
        }
    }

    fn request() -> ProcurementCommitRequest {
        ProcurementCommitRequest::new(
            AwardQuoteSelection::new(id(1), id(2), id(3), 4),
            id(9),
            500,
            "BRL",
        )
    }

    fn snapshot_facts() -> Vec<SnapshotFact> {
        let now = Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap();
        let items = vec![QuotedItemFact {
            rfq_item_id: id(4),
            quantity: 2,
            unit_price_cents: 250,
            line_total_cents: 500,
        }];
        vec![
            SnapshotFact::new(
                1,
                RFQ_OPENED_V1,
                to_value(RfqOpenedFact {
                    rfq_id: id(1),
                    reference: "RFQ-1".into(),
                    currency: "BRL".into(),
                    opened_at: now,
                    items: vec![RfqItemFact {
                        id: id(4),
                        sku: "SKU-1".into(),
                        description: "Item".into(),
                        required_quantity: 2,
                    }],
                })
                .unwrap(),
            ),
            SnapshotFact::new(
                2,
                SUPPLIER_REGISTERED_V1,
                to_value(SupplierRegisteredFact {
                    supplier_id: id(2),
                    reference: "SUP-1".into(),
                    name: "Supplier".into(),
                    registered_at: now,
                })
                .unwrap(),
            ),
            SnapshotFact::new(
                3,
                SUPPLIER_ELIGIBILITY_CHANGED_V1,
                to_value(SupplierEligibilityChangedFact {
                    supplier_id: id(2),
                    status: SupplierStatus::Eligible,
                    changed_at: now,
                })
                .unwrap(),
            ),
            SnapshotFact::new(
                4,
                SUPPLIER_QUOTE_SUBMITTED_V1,
                to_value(SupplierQuoteSubmittedFact {
                    quote_id: id(3),
                    rfq_id: id(1),
                    supplier_id: id(2),
                    currency: "BRL".into(),
                    total_amount_cents: 500,
                    delivery_days: 5,
                    valid_until: chrono::NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
                    submitted_at: now,
                    items,
                })
                .unwrap(),
            ),
            SnapshotFact::new(
                5,
                PROCUREMENT_BUDGET_ALLOCATED_V1,
                to_value(ProcurementBudgetAllocatedFact {
                    budget_id: id(9),
                    currency: "BRL".into(),
                    allocated_amount_cents: 20_000,
                    allocated_at: now,
                })
                .unwrap(),
            ),
        ]
    }

    #[test]
    fn reads_once_prepares_three_outcomes_and_preserves_opaque_token() {
        let request = request();
        let clock = FixedClock;
        let ids = FixedIds(AtomicUsize::new(0));
        let execution = ProcurementCommitExecutionContextFactory::new(&clock, &ids)
            .create_for_request(&request, None)
            .unwrap();
        let reads = Arc::new(AtomicUsize::new(0));
        let reader = CountingReader {
            facts: snapshot_facts(),
            reads: reads.clone(),
        };
        let reactor = ProcurementCommitReactor::new(reader);

        let prepared = reactor.prepare(&request, &execution).unwrap();
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(prepared.token(), "opaque-test-token");
        assert_eq!(prepared.outcomes().len(), 3);
        assert_eq!(
            prepared.outcomes()[0].participant_id().as_str(),
            "procurement.budget.reserve"
        );
        assert_eq!(
            prepared.outcomes()[1].participant_id().as_str(),
            "sourcing.award-supplier-quote"
        );
        assert_eq!(
            prepared.outcomes()[2].participant_id().as_str(),
            "procurement.purchase-order.issue"
        );
    }

    #[test]
    fn insufficient_budget_aborts_before_returning_any_prepared_interaction() {
        let request = request();
        let clock = FixedClock;
        let ids = FixedIds(AtomicUsize::new(0));
        let execution = ProcurementCommitExecutionContextFactory::new(&clock, &ids)
            .create_for_request(&request, None)
            .unwrap();
        let mut facts = snapshot_facts();
        let allocation_index = facts
            .iter()
            .position(|fact| fact.event_type() == PROCUREMENT_BUDGET_ALLOCATED_V1)
            .expect("the successful snapshot fixture contains one allocation");
        let allocation_sequence = facts[allocation_index].sequence_number();
        let now = Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap();
        facts[allocation_index] = SnapshotFact::new(
            allocation_sequence,
            PROCUREMENT_BUDGET_ALLOCATED_V1,
            to_value(ProcurementBudgetAllocatedFact {
                budget_id: id(9),
                currency: "BRL".into(),
                allocated_amount_cents: 499,
                allocated_at: now,
            })
            .unwrap(),
        );
        let reads = Arc::new(AtomicUsize::new(0));
        let reactor = ProcurementCommitReactor::new(CountingReader {
            facts,
            reads: reads.clone(),
        });
        assert!(matches!(
            reactor.prepare(&request, &execution),
            Err(ProcurementCommitReactorError::Participant(_))
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn participant_rejection_discards_every_prepared_outcome() {
        let request = request();
        let clock = FixedClock;
        let ids = FixedIds(AtomicUsize::new(0));
        let execution = ProcurementCommitExecutionContextFactory::new(&clock, &ids)
            .create_for_request(&request, None)
            .unwrap();
        let mut facts = snapshot_facts();
        let eligibility_index = facts
            .iter()
            .position(|fact| fact.event_type() == SUPPLIER_ELIGIBILITY_CHANGED_V1)
            .expect("the sourcing fixture contains supplier eligibility");
        let eligibility_sequence = facts[eligibility_index].sequence_number();
        let now = Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap();
        facts[eligibility_index] = SnapshotFact::new(
            eligibility_sequence,
            SUPPLIER_ELIGIBILITY_CHANGED_V1,
            to_value(SupplierEligibilityChangedFact {
                supplier_id: id(2),
                status: SupplierStatus::Blocked,
                changed_at: now,
            })
            .unwrap(),
        );
        let reads = Arc::new(AtomicUsize::new(0));
        let reactor = ProcurementCommitReactor::new(CountingReader {
            facts,
            reads: reads.clone(),
        });
        assert!(matches!(
            reactor.prepare(&request, &execution),
            Err(ProcurementCommitReactorError::Participant(_))
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn retry_reuses_execution_identity_but_performs_a_new_snapshot_read() {
        let request = request();
        let clock = FixedClock;
        let ids = FixedIds(AtomicUsize::new(0));
        let execution = ProcurementCommitExecutionContextFactory::new(&clock, &ids)
            .create_for_request(&request, None)
            .unwrap();
        let reads = Arc::new(AtomicUsize::new(0));
        let reactor = ProcurementCommitReactor::new(CountingReader {
            facts: snapshot_facts(),
            reads: reads.clone(),
        });

        let first = reactor.prepare(&request, &execution).unwrap();
        let retry = reactor.prepare(&request, &execution).unwrap();

        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(first.execution(), execution.context());
        assert_eq!(retry.execution(), execution.context());
        assert_eq!(first, retry);
    }

    #[test]
    fn request_fingerprint_mismatch_is_rejected_before_the_read() {
        let request = request();
        let clock = FixedClock;
        let ids = FixedIds(AtomicUsize::new(0));
        let execution = ProcurementCommitExecutionContextFactory::new(&clock, &ids)
            .create_for_request(&request, None)
            .unwrap();
        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(id(1), id(2), id(3), 5),
            id(9),
            500,
            "BRL",
        );
        let reads = Arc::new(AtomicUsize::new(0));
        let reactor = ProcurementCommitReactor::new(CountingReader {
            facts: snapshot_facts(),
            reads: reads.clone(),
        });
        assert!(matches!(
            reactor.prepare(&changed, &execution),
            Err(ProcurementCommitReactorError::RequestFingerprintMismatch)
        ));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }
}
