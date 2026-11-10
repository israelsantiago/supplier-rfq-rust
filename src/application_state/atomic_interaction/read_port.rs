use super::{
    context::CompositeContextSpecification,
    state_port::{AtomicInteractionStatePort, SnapshotEnvelope, StatePortError},
};

/// Read-only capability consumed by the Stage 5 Reactor.
///
/// The adapter-issued token is returned unchanged. This contract deliberately
/// exposes no append, commit, receipt, or persistence operation.
pub trait CompositeContextReader: Send + Sync {
    type Token: Send + Sync;

    fn read_composite(
        &self,
        specification: &CompositeContextSpecification,
    ) -> Result<SnapshotEnvelope<Self::Token>, StatePortError>;
}

/// Restricts an existing writable state port to its read capability.
///
/// The wrapped port is private and is never returned. The Reactor is constructed
/// with this wrapper as `CompositeContextReader`, not as `AtomicInteractionStatePort`.
pub struct ReadOnlyCompositeContextReader<P> {
    inner: P,
}

impl<P> ReadOnlyCompositeContextReader<P> {
    pub fn new(inner: P) -> Self {
        Self { inner }
    }
}

impl<P> CompositeContextReader for ReadOnlyCompositeContextReader<P>
where
    P: AtomicInteractionStatePort,
{
    type Token = P::Token;

    fn read_composite(
        &self,
        specification: &CompositeContextSpecification,
    ) -> Result<SnapshotEnvelope<Self::Token>, StatePortError> {
        self.inner.read_composite(specification)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use serde_json::json;

    use crate::{
        amri::ParticipantId,
        application_state::atomic_interaction::{
            context::{CompositeContextSpecification, ContextClause, ContextRequirement},
            state_port::{
                AtomicInteractionStatePort, CommitReceipt, CompositeSnapshot, NewAtomicFact,
                SnapshotEnvelope, StatePortError,
            },
        },
    };

    use super::{CompositeContextReader, ReadOnlyCompositeContextReader};

    #[derive(Clone, Default)]
    struct WritableFake {
        reads: Arc<AtomicUsize>,
        appends: Arc<AtomicUsize>,
    }

    impl AtomicInteractionStatePort for WritableFake {
        type Token = String;

        fn read_composite(
            &self,
            specification: &CompositeContextSpecification,
        ) -> Result<SnapshotEnvelope<Self::Token>, StatePortError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(SnapshotEnvelope::new(
                CompositeSnapshot::new(Vec::new(), specification.fingerprint()),
                "opaque-read-token".to_owned(),
            ))
        }

        fn append_if_unchanged(
            &self,
            _token: &Self::Token,
            facts: Vec<NewAtomicFact>,
        ) -> Result<CommitReceipt, StatePortError> {
            self.appends.fetch_add(1, Ordering::SeqCst);
            let count = u64::try_from(facts.len()).expect("a test batch length must fit into u64");
            Ok(CommitReceipt::new(1, count, count))
        }
    }

    fn specification() -> CompositeContextSpecification {
        let clause = ContextClause::new("test.fact.v1", json!({"id":"one"})).unwrap();
        CompositeContextSpecification::union(&[ContextRequirement::new(
            ParticipantId::new("reader-test").unwrap(),
            vec![clause],
        )])
        .unwrap()
    }

    #[test]
    fn read_only_wrapper_preserves_opaque_token_and_never_appends() {
        let state = WritableFake::default();
        let observed = state.clone();
        let reader = ReadOnlyCompositeContextReader::new(state);

        let envelope = reader.read_composite(&specification()).unwrap();

        assert_eq!(observed.reads.load(Ordering::SeqCst), 1);
        assert_eq!(observed.appends.load(Ordering::SeqCst), 0);
        assert_eq!(envelope.token(), "opaque-read-token");
        assert!(envelope.snapshot().facts().is_empty());
    }

    #[test]
    fn reactor_facing_contract_requires_only_the_read_capability() {
        fn read<R: CompositeContextReader<Token = String>>(
            reader: &R,
            spec: &CompositeContextSpecification,
        ) -> Result<SnapshotEnvelope<String>, StatePortError> {
            reader.read_composite(spec)
        }

        let reader = ReadOnlyCompositeContextReader::new(WritableFake::default());
        assert_eq!(
            read(&reader, &specification()).unwrap().token(),
            "opaque-read-token"
        );
    }
}
