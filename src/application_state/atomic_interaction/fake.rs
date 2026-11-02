use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::context::{CompositeContextSpecification, ContextClause};
use super::state_port::{
    AtomicInteractionStatePort, CommitReceipt, CompositeSnapshot, NewAtomicFact, SnapshotEnvelope,
    SnapshotFact, StatePortError, StoreIdentity,
};

#[derive(Clone)]
pub(super) struct FakeAtomicInteractionState {
    identity: StoreIdentity,
    state: Arc<Mutex<FakeStore>>,
}

#[derive(Default)]
struct FakeStore {
    next_sequence: u64,
    facts: Vec<SnapshotFact>,
    read_calls: usize,
}

#[derive(Clone)]
pub(super) struct FakeConsistencyToken {
    identity: StoreIdentity,
    clauses: Vec<ContextClause>,
    expected_context_version: Option<u64>,
}

impl FakeAtomicInteractionState {
    pub(super) fn new(identity: &str) -> Self {
        Self {
            identity: StoreIdentity::new(identity).expect("test store identity is valid"),
            state: Arc::new(Mutex::new(FakeStore::default())),
        }
    }

    pub(super) fn append_external(
        &self,
        event_type: &str,
        payload: Value,
    ) -> Result<u64, StatePortError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| StatePortError::Store("fake store mutex poisoned".to_owned()))?;
        state.next_sequence = state
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| StatePortError::Store("fake sequence overflow".to_owned()))?;
        let sequence = state.next_sequence;
        state
            .facts
            .push(SnapshotFact::new(sequence, event_type, payload));
        Ok(sequence)
    }

    pub(super) fn read_calls(&self) -> usize {
        self.state
            .lock()
            .expect("fake store mutex is healthy")
            .read_calls
    }

    pub(super) fn fact_count(&self) -> usize {
        self.state
            .lock()
            .expect("fake store mutex is healthy")
            .facts
            .len()
    }
}

impl AtomicInteractionStatePort for FakeAtomicInteractionState {
    type Token = FakeConsistencyToken;

    fn read_composite(
        &self,
        specification: &CompositeContextSpecification,
    ) -> Result<SnapshotEnvelope<Self::Token>, StatePortError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| StatePortError::Store("fake store mutex poisoned".to_owned()))?;
        state.read_calls += 1;
        let clauses = specification.clauses().to_vec();
        let facts = state
            .facts
            .iter()
            .filter(|fact| matches_any_clause(fact, &clauses))
            .cloned()
            .collect::<Vec<_>>();
        let expected_context_version = facts.iter().map(SnapshotFact::sequence_number).max();
        let fingerprint = specification.fingerprint();
        let snapshot = CompositeSnapshot::new(facts, fingerprint);
        let token = FakeConsistencyToken {
            identity: self.identity.clone(),
            clauses,
            expected_context_version,
        };
        Ok(SnapshotEnvelope::new(snapshot, token))
    }

    fn append_if_unchanged(
        &self,
        token: &Self::Token,
        facts: Vec<NewAtomicFact>,
    ) -> Result<CommitReceipt, StatePortError> {
        if token.identity != self.identity {
            return Err(StatePortError::StoreIdentityMismatch);
        }
        if facts.is_empty() {
            return Err(StatePortError::EmptyBatch);
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| StatePortError::Store("fake store mutex poisoned".to_owned()))?;
        let actual_context_version = state
            .facts
            .iter()
            .filter(|fact| matches_any_clause(fact, &token.clauses))
            .map(SnapshotFact::sequence_number)
            .max();
        if actual_context_version != token.expected_context_version {
            return Err(StatePortError::ContextChanged);
        }
        let count = u64::try_from(facts.len())
            .map_err(|_| StatePortError::Store("fake batch size exceeds u64".to_owned()))?;
        let first = state
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| StatePortError::Store("fake sequence overflow".to_owned()))?;
        let last = state
            .next_sequence
            .checked_add(count)
            .ok_or_else(|| StatePortError::Store("fake sequence overflow".to_owned()))?;

        // Validate the entire range before mutating state so even synthetic overflow
        // cannot leave a prefix of the batch committed.
        for (offset, fact) in (0..count).zip(facts) {
            let sequence = first + offset;
            state.facts.push(SnapshotFact::new(
                sequence,
                fact.event_type(),
                fact.payload().clone(),
            ));
        }
        state.next_sequence = last;
        Ok(CommitReceipt::new(first, last, count))
    }
}

fn matches_any_clause(fact: &SnapshotFact, clauses: &[ContextClause]) -> bool {
    clauses.iter().any(|clause| {
        fact.event_type() == clause.event_type()
            && json_is_subset(fact.payload(), clause.payload_subset())
    })
}

fn json_is_subset(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => expected.iter().all(|(key, value)| {
            actual
                .get(key)
                .is_some_and(|actual_value| json_is_subset(actual_value, value))
        }),
        (Value::Array(actual), Value::Array(expected)) => expected.iter().all(|expected_value| {
            actual
                .iter()
                .any(|actual_value| json_is_subset(actual_value, expected_value))
        }),
        (actual, expected) => actual == expected,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::amri::ParticipantId;

    use super::super::context::{CompositeContextSpecification, ContextClause, ContextRequirement};
    use super::super::state_port::{AtomicInteractionStatePort, NewAtomicFact, StatePortError};
    use super::FakeAtomicInteractionState;

    fn specification() -> CompositeContextSpecification {
        CompositeContextSpecification::union(&[
            ContextRequirement::new(
                ParticipantId::new("close-rfq").unwrap(),
                vec![
                    ContextClause::new("rfq.opened.v1", json!({"rfq_id":"RFQ-1","tenant":"T1"}))
                        .unwrap(),
                ],
            ),
            ContextRequirement::new(
                ParticipantId::new("award-quote").unwrap(),
                vec![
                    ContextClause::new("supplier.registered.v1", json!({"supplier_id":"SUP-1"}))
                        .unwrap(),
                ],
            ),
        ])
        .unwrap()
    }

    fn fact(event_type: &str, payload: serde_json::Value) -> NewAtomicFact {
        NewAtomicFact::new(event_type, payload).unwrap()
    }

    #[test]
    fn read_returns_one_snapshot_and_one_token_for_the_union() {
        let state = FakeAtomicInteractionState::new("rfq-test-store");
        state
            .append_external("rfq.opened.v1", json!({"rfq_id":"RFQ-1","tenant":"T1"}))
            .unwrap();
        state
            .append_external("supplier.registered.v1", json!({"supplier_id":"SUP-1"}))
            .unwrap();

        let snapshot = state.read_composite(&specification()).unwrap();

        assert_eq!(state.read_calls(), 1);
        assert_eq!(snapshot.snapshot().facts().len(), 2);
        assert_eq!(
            snapshot.snapshot().specification_fingerprint(),
            specification().fingerprint()
        );
        let (composite_snapshot, _single_token) = snapshot.into_parts();
        assert_eq!(composite_snapshot.facts().len(), 2);
    }

    #[test]
    fn relevant_context_change_rejects_the_whole_batch() {
        let state = FakeAtomicInteractionState::new("rfq-test-store");
        let snapshot = state.read_composite(&specification()).unwrap();
        state
            .append_external(
                "rfq.opened.v1",
                json!({"rfq_id":"RFQ-1","tenant":"T1","revision":2}),
            )
            .unwrap();
        let before = state.fact_count();

        let result = state.append_if_unchanged(
            snapshot.token(),
            vec![
                fact(
                    "rfq.opened.v1",
                    json!({"rfq_id":"RFQ-1","tenant":"T1","attempt":"a"}),
                ),
                fact(
                    "supplier.registered.v1",
                    json!({"supplier_id":"SUP-1","attempt":"b"}),
                ),
            ],
        );

        assert_eq!(result, Err(StatePortError::ContextChanged));
        assert_eq!(
            state.fact_count(),
            before,
            "no prefix of a failed batch may be stored"
        );
    }

    #[test]
    fn irrelevant_context_change_does_not_invalidate_the_snapshot() {
        let state = FakeAtomicInteractionState::new("rfq-test-store");
        let snapshot = state.read_composite(&specification()).unwrap();
        state
            .append_external("supplier.registered.v1", json!({"supplier_id":"SUP-OTHER"}))
            .unwrap();

        let receipt = state
            .append_if_unchanged(
                snapshot.token(),
                vec![fact(
                    "rfq.opened.v1",
                    json!({"rfq_id":"RFQ-1","tenant":"T1","closed":true}),
                )],
            )
            .unwrap();

        assert_eq!(receipt.committed_count(), 1);
    }

    #[test]
    fn consistency_token_from_another_logical_store_is_rejected() {
        let first = FakeAtomicInteractionState::new("store-a");
        let second = FakeAtomicInteractionState::new("store-b");
        let snapshot = first.read_composite(&specification()).unwrap();

        let result = second.append_if_unchanged(
            snapshot.token(),
            vec![fact(
                "rfq.opened.v1",
                json!({"rfq_id":"RFQ-1","tenant":"T1"}),
            )],
        );

        assert_eq!(result, Err(StatePortError::StoreIdentityMismatch));
        assert_eq!(second.fact_count(), 0);
    }

    #[test]
    fn empty_batch_and_invalid_event_type_are_rejected_before_store_access() {
        let state = FakeAtomicInteractionState::new("rfq-test-store");
        let snapshot = state.read_composite(&specification()).unwrap();
        assert_eq!(
            state.append_if_unchanged(snapshot.token(), vec![]),
            Err(StatePortError::EmptyBatch)
        );
        assert!(matches!(
            NewAtomicFact::new(" rfq.opened.v1", json!({})),
            Err(StatePortError::InvalidEventType)
        ));
    }
    #[test]
    fn store_identity_is_non_empty_and_stable_as_a_configuration_key() {
        use super::super::state_port::StoreIdentity;

        assert!(StoreIdentity::new(" ").is_err());
        assert!(StoreIdentity::new(" store-a").is_err());
        assert_eq!(StoreIdentity::new("store-a").unwrap().as_str(), "store-a");
    }

    #[test]
    fn participant_requirement_exposes_its_declaring_participant() {
        let requirement = ContextRequirement::new(
            ParticipantId::new("close-rfq").unwrap(),
            vec![ContextClause::new("rfq.opened.v1", json!({"rfq_id":"RFQ-1"})).unwrap()],
        );
        assert_eq!(requirement.participant_id().as_str(), "close-rfq");
    }

    #[test]
    fn snapshot_fact_and_commit_receipt_expose_the_complete_batch_metadata() {
        let state = FakeAtomicInteractionState::new("receipt-store");
        state
            .append_external("rfq.opened.v1", json!({"rfq_id":"RFQ-1","tenant":"T1"}))
            .unwrap();
        let envelope = state.read_composite(&specification()).unwrap();
        let snapshot_fact = &envelope.snapshot().facts()[0];
        assert_eq!(snapshot_fact.sequence_number(), 1);
        assert_eq!(snapshot_fact.event_type(), "rfq.opened.v1");
        assert_eq!(
            snapshot_fact.payload(),
            &json!({"rfq_id":"RFQ-1","tenant":"T1"})
        );

        let receipt = state
            .append_if_unchanged(
                envelope.token(),
                vec![
                    fact("unrelated.audit.v1", json!({"id":1})),
                    fact("unrelated.audit.v1", json!({"id":2})),
                ],
            )
            .unwrap();
        assert_eq!(receipt.first_sequence_number(), 2);
        assert_eq!(receipt.last_sequence_number(), 3);
        assert_eq!(receipt.committed_count(), 2);
    }
}
