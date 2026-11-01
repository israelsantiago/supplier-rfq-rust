//! Minimal, domain- and store-independent AMRI protocol primitives.
//!
//! Stage 2 deliberately stops before snapshot acquisition, persistence ports, participant
//! adapters and Reactor orchestration. Those concerns belong to later implementation stages.

mod definition;
mod error;
mod execution;
mod identity;
mod outcome;
mod plan;

pub use definition::{InteractionDefinition, ParticipantDefinition};
pub use error::{DefinitionError, PlanValidationError};
pub use execution::ExecutionContext;
pub use identity::{
    CorrelationId, FactId, FactTypeId, Fingerprint, FingerprintError, IdentifierError,
    IdentityReuseError, InteractionDefinitionId, InteractionId, InteractionIdentity, ParticipantId,
};
pub use outcome::{PreparedOutcome, ProposedFact};
pub use plan::{PlannedFactRef, ValidatedCommitPlan, ValidationSummary};

#[cfg(test)]
mod tests;
