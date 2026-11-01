use std::fmt;

use super::identity::{FactId, FactTypeId, InteractionDefinitionId, ParticipantId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionError {
    EmptyParticipantSet,
    DuplicateParticipant(ParticipantId),
    DuplicateDependency {
        participant: ParticipantId,
        dependency: ParticipantId,
    },
    SelfDependency(ParticipantId),
    UnknownDependency {
        participant: ParticipantId,
        dependency: ParticipantId,
    },
    DuplicateAllowedFactType {
        participant: ParticipantId,
        fact_type: FactTypeId,
    },
    DependencyCycle(Vec<ParticipantId>),
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DefinitionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanValidationError {
    DefinitionIdMismatch {
        expected: InteractionDefinitionId,
        actual: InteractionDefinitionId,
    },
    DefinitionVersionMismatch {
        expected: u32,
        actual: u32,
    },
    UnknownParticipant(ParticipantId),
    DuplicateOutcome(ParticipantId),
    MissingOutcome(ParticipantId),
    DisallowedFactType {
        participant: ParticipantId,
        fact_type: FactTypeId,
    },
    EmptyParticipantFactSet(ParticipantId),
    DuplicateFactId(FactId),
    EmptyFactBatch,
}

impl fmt::Display for PlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PlanValidationError {}
