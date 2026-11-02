use serde_json::Value;
use thiserror::Error;

use crate::amri::Fingerprint;

use super::context::CompositeContextSpecification;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StoreIdentity(String);

impl StoreIdentity {
    pub fn new(value: impl Into<String>) -> Result<Self, StatePortError> {
        let value = value.into();
        if value.trim().is_empty() || value.trim() != value {
            return Err(StatePortError::InvalidStoreIdentity);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotFact {
    sequence_number: u64,
    event_type: String,
    payload: Value,
}

impl SnapshotFact {
    pub fn new(sequence_number: u64, event_type: impl Into<String>, payload: Value) -> Self {
        Self {
            sequence_number,
            event_type: event_type.into(),
            payload,
        }
    }

    pub const fn sequence_number(&self) -> u64 {
        self.sequence_number
    }

    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompositeSnapshot {
    facts: Vec<SnapshotFact>,
    specification_fingerprint: Fingerprint,
}

impl CompositeSnapshot {
    /// Builds a composite snapshot from the facts returned by a state adapter.
    /// Public visibility allows independent implementations of the state port to
    /// materialize this application-owned result without exposing its fields.
    pub fn new(facts: Vec<SnapshotFact>, specification_fingerprint: Fingerprint) -> Self {
        Self {
            facts,
            specification_fingerprint,
        }
    }

    pub fn facts(&self) -> &[SnapshotFact] {
        &self.facts
    }

    pub const fn specification_fingerprint(&self) -> Fingerprint {
        self.specification_fingerprint
    }
}

/// A snapshot and its single opaque token are returned together. The token type is chosen
/// by the adapter; callers cannot manufacture a token from a sequence number.
pub struct SnapshotEnvelope<T> {
    snapshot: CompositeSnapshot,
    token: T,
}

impl<T> SnapshotEnvelope<T> {
    /// Pairs a composite snapshot with the opaque consistency token issued by its adapter.
    /// Public visibility is required so state-port implementations can return the envelope.
    pub fn new(snapshot: CompositeSnapshot, token: T) -> Self {
        Self { snapshot, token }
    }

    pub fn snapshot(&self) -> &CompositeSnapshot {
        &self.snapshot
    }

    pub fn token(&self) -> &T {
        &self.token
    }

    pub fn into_parts(self) -> (CompositeSnapshot, T) {
        (self.snapshot, self.token)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewAtomicFact {
    event_type: String,
    payload: Value,
}

impl NewAtomicFact {
    pub fn new(event_type: impl Into<String>, payload: Value) -> Result<Self, StatePortError> {
        let event_type = event_type.into();
        if event_type.trim().is_empty() || event_type.trim() != event_type {
            return Err(StatePortError::InvalidEventType);
        }
        Ok(Self {
            event_type,
            payload,
        })
    }

    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitReceipt {
    first_sequence_number: u64,
    last_sequence_number: u64,
    committed_count: u64,
}

impl CommitReceipt {
    /// Constructs the receipt returned by a successful atomic state-port append.
    /// Public visibility allows state-port implementations to create this result.
    pub const fn new(
        first_sequence_number: u64,
        last_sequence_number: u64,
        committed_count: u64,
    ) -> Self {
        Self {
            first_sequence_number,
            last_sequence_number,
            committed_count,
        }
    }

    pub const fn first_sequence_number(&self) -> u64 {
        self.first_sequence_number
    }

    pub const fn last_sequence_number(&self) -> u64 {
        self.last_sequence_number
    }

    pub const fn committed_count(&self) -> u64 {
        self.committed_count
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StatePortError {
    #[error("store identity must be non-empty and have no leading/trailing whitespace")]
    InvalidStoreIdentity,
    #[error("event type must be non-empty and have no leading/trailing whitespace")]
    InvalidEventType,
    #[error("at least one fact is required for a conditional append")]
    EmptyBatch,
    #[error("consistency token belongs to a different logical store")]
    StoreIdentityMismatch,
    #[error("composite context changed before the conditional append")]
    ContextChanged,
    #[error("state store operation failed: {0}")]
    Store(String),
}

/// Application State boundary for one composite snapshot and one conditional batch write.
pub trait AtomicInteractionStatePort: Send + Sync {
    type Token: Send + Sync;

    fn read_composite(
        &self,
        specification: &CompositeContextSpecification,
    ) -> Result<SnapshotEnvelope<Self::Token>, StatePortError>;

    /// The query and expected context version are carried inside the token; callers cannot
    /// accidentally rebuild a broader/narrower query for the write.
    fn append_if_unchanged(
        &self,
        token: &Self::Token,
        facts: Vec<NewAtomicFact>,
    ) -> Result<CommitReceipt, StatePortError>;
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{NewAtomicFact, SnapshotFact, StatePortError, StoreIdentity};

    #[test]
    fn store_identity_rejects_empty_and_surrounding_whitespace() {
        for value in ["", " ", "  ", " store-a", "store-a "] {
            assert_eq!(
                StoreIdentity::new(value),
                Err(StatePortError::InvalidStoreIdentity)
            );
        }
        assert_eq!(StoreIdentity::new("store-a").unwrap().as_str(), "store-a");
    }

    #[test]
    fn snapshot_fact_accessors_preserve_record_values() {
        let fact = SnapshotFact::new(7, "rfq.opened.v1", json!({"rfq_id":"RFQ-1"}));
        assert_eq!(fact.sequence_number(), 7);
        assert_eq!(fact.event_type(), "rfq.opened.v1");
        assert_eq!(fact.payload(), &json!({"rfq_id":"RFQ-1"}));
    }

    #[test]
    fn new_atomic_fact_validates_type_and_exposes_typed_payload() {
        for value in ["", " ", " rfq.opened.v1", "rfq.opened.v1 "] {
            assert_eq!(
                NewAtomicFact::new(value, json!({})),
                Err(StatePortError::InvalidEventType)
            );
        }
        let fact = NewAtomicFact::new("rfq.closed.v1", json!({"rfq_id":"RFQ-1"})).unwrap();
        assert_eq!(fact.event_type(), "rfq.closed.v1");
        assert_eq!(fact.payload(), &json!({"rfq_id":"RFQ-1"}));
    }
}
