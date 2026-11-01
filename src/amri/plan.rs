use std::collections::BTreeSet;

use super::{
    definition::InteractionDefinition,
    error::PlanValidationError,
    execution::ExecutionContext,
    identity::{FactId, Fingerprint, ParticipantId},
    outcome::{PreparedOutcome, ProposedFact},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationSummary {
    participant_count: usize,
    fact_count: usize,
    participant_order: Vec<ParticipantId>,
}

impl ValidationSummary {
    pub const fn participant_count(&self) -> usize {
        self.participant_count
    }

    pub const fn fact_count(&self) -> usize {
        self.fact_count
    }

    pub fn participant_order(&self) -> &[ParticipantId] {
        &self.participant_order
    }
}

/// A reference to one fact in the exact order that a future adapter must preserve.
pub struct PlannedFactRef<'a, F> {
    participant_id: &'a ParticipantId,
    fact: &'a ProposedFact<F>,
}

impl<'a, F> PlannedFactRef<'a, F> {
    pub fn participant_id(&self) -> &'a ParticipantId {
        self.participant_id
    }

    pub fn fact(&self) -> &'a ProposedFact<F> {
        self.fact
    }
}

/// A structurally validated plan. It is not yet a storage commit authorization:
/// the coherent snapshot/token and atomic state boundary are Stage 3 responsibilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCommitPlan<O, F> {
    execution: ExecutionContext,
    plan_fingerprint: Fingerprint,
    outcomes: Vec<PreparedOutcome<O, F>>,
    summary: ValidationSummary,
}

impl<O, F> ValidatedCommitPlan<O, F> {
    pub fn execution(&self) -> &ExecutionContext {
        &self.execution
    }

    pub const fn plan_fingerprint(&self) -> Fingerprint {
        self.plan_fingerprint
    }

    pub fn outcomes(&self) -> &[PreparedOutcome<O, F>] {
        &self.outcomes
    }

    pub fn summary(&self) -> &ValidationSummary {
        &self.summary
    }

    /// Iterates facts in deterministic participant topological order, preserving each
    /// participant's local fact order.
    pub fn facts(&self) -> impl Iterator<Item = PlannedFactRef<'_, F>> {
        self.outcomes.iter().flat_map(|outcome| {
            outcome.facts().iter().map(move |fact| PlannedFactRef {
                participant_id: outcome.participant_id(),
                fact,
            })
        })
    }
}

impl InteractionDefinition {
    /// Validates a complete set of prepared participant outcomes and seals a deterministic plan.
    ///
    /// The fingerprint must be computed by the application from a versioned canonical encoding.
    /// Cross-capability business rules remain outside AMRI Core and are not evaluated here.
    pub fn validate_prepared_outcomes<O, F>(
        &self,
        execution: ExecutionContext,
        prepared_outcomes: Vec<PreparedOutcome<O, F>>,
        plan_fingerprint: Fingerprint,
    ) -> Result<ValidatedCommitPlan<O, F>, PlanValidationError> {
        let identity = execution.identity();
        if identity.definition_id() != self.id() {
            return Err(PlanValidationError::DefinitionIdMismatch {
                expected: self.id().clone(),
                actual: identity.definition_id().clone(),
            });
        }
        if identity.definition_version() != self.version() {
            return Err(PlanValidationError::DefinitionVersionMismatch {
                expected: self.version().get(),
                actual: identity.definition_version().get(),
            });
        }

        let mut outcomes_by_participant = std::collections::BTreeMap::new();
        for outcome in prepared_outcomes {
            let participant_id = outcome.participant_id().clone();
            if self.participant(&participant_id).is_none() {
                return Err(PlanValidationError::UnknownParticipant(participant_id));
            }
            if outcomes_by_participant
                .insert(participant_id.clone(), outcome)
                .is_some()
            {
                return Err(PlanValidationError::DuplicateOutcome(participant_id));
            }
        }

        for participant_id in self.execution_order() {
            if !outcomes_by_participant.contains_key(participant_id) {
                return Err(PlanValidationError::MissingOutcome(participant_id.clone()));
            }
        }

        let mut seen_fact_ids = BTreeSet::<FactId>::new();
        let mut ordered_outcomes = Vec::with_capacity(self.execution_order().len());
        let mut fact_count = 0_usize;

        for participant_id in self.execution_order() {
            let outcome = outcomes_by_participant
                .remove(participant_id)
                .expect("the complete participant set was validated above");
            let participant = self
                .participant(participant_id)
                .expect("execution order contains only declared participants");

            if outcome.facts().is_empty() && !participant.allows_empty_fact_set() {
                return Err(PlanValidationError::EmptyParticipantFactSet(
                    participant_id.clone(),
                ));
            }

            for fact in outcome.facts() {
                if !participant.allowed_fact_types().contains(fact.fact_type()) {
                    return Err(PlanValidationError::DisallowedFactType {
                        participant: participant_id.clone(),
                        fact_type: fact.fact_type().clone(),
                    });
                }
                if !seen_fact_ids.insert(fact.id().clone()) {
                    return Err(PlanValidationError::DuplicateFactId(fact.id().clone()));
                }
                fact_count += 1;
            }
            ordered_outcomes.push(outcome);
        }

        if fact_count == 0 {
            return Err(PlanValidationError::EmptyFactBatch);
        }

        let participant_order = self.execution_order().to_vec();
        let summary = ValidationSummary {
            participant_count: participant_order.len(),
            fact_count,
            participant_order,
        };

        Ok(ValidatedCommitPlan {
            execution,
            plan_fingerprint,
            outcomes: ordered_outcomes,
            summary,
        })
    }
}
