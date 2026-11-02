use std::sync::Arc;

use factstr::{EventFilter, EventQuery, EventStore, EventStoreError, NewEvent};

use super::context::CompositeContextSpecification;
use super::state_port::{
    AtomicInteractionStatePort, CommitReceipt, CompositeSnapshot, NewAtomicFact, SnapshotEnvelope,
    SnapshotFact, StatePortError, StoreIdentity,
};
use crate::amri::Fingerprint;

/// FACTSTR-specific consistency token. All fields are private so a caller cannot replace
/// the query or context version independently. The exact query used for read is retained
/// and passed unchanged to append_if.
#[derive(Clone)]
pub struct FactstrConsistencyToken {
    store_identity: StoreIdentity,
    specification_fingerprint: Fingerprint,
    context_query: EventQuery,
    expected_context_version: Option<u64>,
}

pub struct FactstrAtomicInteractionState<S> {
    store: Arc<S>,
    store_identity: StoreIdentity,
}

impl FactstrConsistencyToken {
    pub const fn specification_fingerprint(&self) -> Fingerprint {
        self.specification_fingerprint
    }
}

impl<S> FactstrAtomicInteractionState<S> {
    pub fn new(store: Arc<S>, store_identity: StoreIdentity) -> Self {
        Self {
            store,
            store_identity,
        }
    }
}

impl<S> AtomicInteractionStatePort for FactstrAtomicInteractionState<S>
where
    S: EventStore + Send + Sync,
{
    type Token = FactstrConsistencyToken;

    fn read_composite(
        &self,
        specification: &CompositeContextSpecification,
    ) -> Result<SnapshotEnvelope<Self::Token>, StatePortError> {
        let context_query = to_event_query(specification);
        let result = self.store.query(&context_query).map_err(map_store_error)?;
        let expected_context_version = result.current_context_version;
        let facts = result
            .event_records
            .into_iter()
            .map(|record| {
                SnapshotFact::new(record.sequence_number, record.event_type, record.payload)
            })
            .collect();
        let fingerprint = specification.fingerprint();
        let snapshot = CompositeSnapshot::new(facts, fingerprint);
        let token = FactstrConsistencyToken {
            store_identity: self.store_identity.clone(),
            specification_fingerprint: fingerprint,
            context_query,
            expected_context_version,
        };
        Ok(SnapshotEnvelope::new(snapshot, token))
    }

    fn append_if_unchanged(
        &self,
        token: &Self::Token,
        facts: Vec<NewAtomicFact>,
    ) -> Result<CommitReceipt, StatePortError> {
        ensure_token_store(&self.store_identity, token)?;
        if facts.is_empty() {
            return Err(StatePortError::EmptyBatch);
        }
        // FACTSTR 0.5.2 NewEvent::new is an infallible value constructor. Validation of
        // event-type shape happens at the application-owned NewAtomicFact boundary.
        let events = facts
            .into_iter()
            .map(|fact| NewEvent::new(fact.event_type().to_owned(), fact.payload().clone()))
            .collect::<Vec<_>>();

        self.store
            .append_if(events, &token.context_query, token.expected_context_version)
            .map(|result| {
                CommitReceipt::new(
                    result.first_sequence_number,
                    result.last_sequence_number,
                    result.committed_count,
                )
            })
            .map_err(map_store_error)
    }
}

fn to_event_query(specification: &CompositeContextSpecification) -> EventQuery {
    let filters = specification.clauses().iter().map(|clause| {
        EventFilter::for_event_types([clause.event_type().to_owned()])
            .with_payload_predicates([clause.payload_subset().clone()])
    });
    EventQuery::all().with_filters(filters)
}

fn ensure_token_store(
    expected: &StoreIdentity,
    token: &FactstrConsistencyToken,
) -> Result<(), StatePortError> {
    if expected != &token.store_identity {
        return Err(StatePortError::StoreIdentityMismatch);
    }
    Ok(())
}

fn map_store_error(error: EventStoreError) -> StatePortError {
    match error {
        EventStoreError::ConditionalAppendConflict { .. } => StatePortError::ContextChanged,
        other => StatePortError::Store(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use factstr::EventQuery;
    use serde_json::json;

    use super::{FactstrConsistencyToken, ensure_token_store, to_event_query};
    use crate::amri::{Fingerprint, ParticipantId};
    use crate::application_state::atomic_interaction::{
        context::{CompositeContextSpecification, ContextClause, ContextRequirement},
        state_port::{StatePortError, StoreIdentity},
    };

    #[test]
    fn adapter_maps_each_clause_to_one_or_filter_with_one_object_predicate() {
        let specification = CompositeContextSpecification::union(&[
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
        .unwrap();

        let query = to_event_query(&specification);
        assert_eq!(query.min_sequence_number, None);
        let filters = query.filters.expect("filters must be present");
        assert_eq!(filters.len(), 2);
        assert_eq!(
            filters[0].event_types,
            Some(vec!["rfq.opened.v1".to_owned()])
        );
        assert_eq!(
            filters[0].payload_predicates,
            Some(vec![json!({"rfq_id":"RFQ-1","tenant":"T1"})])
        );
        assert_eq!(
            filters[1].event_types,
            Some(vec!["supplier.registered.v1".to_owned()])
        );
        assert_eq!(
            filters[1].payload_predicates,
            Some(vec![json!({"supplier_id":"SUP-1"})])
        );
    }

    #[test]
    fn token_from_a_different_logical_factstr_store_is_rejected() {
        let expected = StoreIdentity::new("database-a").unwrap();
        let other = StoreIdentity::new("database-b").unwrap();
        let token = FactstrConsistencyToken {
            store_identity: other,
            specification_fingerprint: Fingerprint::from_bytes([1; 32]),
            context_query: EventQuery::all(),
            expected_context_version: Some(7),
        };

        assert_eq!(
            ensure_token_store(&expected, &token),
            Err(StatePortError::StoreIdentityMismatch)
        );
    }
}
