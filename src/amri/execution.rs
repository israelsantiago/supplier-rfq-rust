use super::identity::{CorrelationId, InteractionIdentity};

/// Immutable metadata captured once for one logical interaction attempt.
///
/// `captured_at_unix_ms` is supplied by the delivery/application boundary; AMRI Core never
/// reads a wall clock. Retries of the same logical interaction may reuse this context when
/// the domain requires stable accepted-time semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionContext {
    identity: InteractionIdentity,
    captured_at_unix_ms: u64,
    correlation_id: Option<CorrelationId>,
}

impl ExecutionContext {
    pub fn new(
        identity: InteractionIdentity,
        captured_at_unix_ms: u64,
        correlation_id: Option<CorrelationId>,
    ) -> Self {
        Self {
            identity,
            captured_at_unix_ms,
            correlation_id,
        }
    }

    pub fn identity(&self) -> &InteractionIdentity {
        &self.identity
    }

    pub const fn captured_at_unix_ms(&self) -> u64 {
        self.captured_at_unix_ms
    }

    pub fn correlation_id(&self) -> Option<&CorrelationId> {
        self.correlation_id.as_ref()
    }
}
