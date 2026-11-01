use super::identity::{FactId, FactTypeId, ParticipantId};

/// A typed business consequence proposed by a participant, not yet persisted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedFact<F> {
    id: FactId,
    fact_type: FactTypeId,
    payload: F,
}

impl<F> ProposedFact<F> {
    pub fn new(id: FactId, fact_type: FactTypeId, payload: F) -> Self {
        Self {
            id,
            fact_type,
            payload,
        }
    }

    pub fn id(&self) -> &FactId {
        &self.id
    }

    pub fn fact_type(&self) -> &FactTypeId {
        &self.fact_type
    }

    pub fn payload(&self) -> &F {
        &self.payload
    }
}

/// The immutable result of preparing one declared participant.
///
/// `output` is the typed information that a dependent participant may consume. `facts` are
/// proposed consequences only; this type has no persistence API or reference to a state port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOutcome<O, F> {
    participant_id: ParticipantId,
    output: O,
    facts: Vec<ProposedFact<F>>,
}

impl<O, F> PreparedOutcome<O, F> {
    pub fn new(participant_id: ParticipantId, output: O, facts: Vec<ProposedFact<F>>) -> Self {
        Self {
            participant_id,
            output,
            facts,
        }
    }

    pub fn participant_id(&self) -> &ParticipantId {
        &self.participant_id
    }

    pub fn output(&self) -> &O {
        &self.output
    }

    pub fn facts(&self) -> &[ProposedFact<F>] {
        &self.facts
    }
}
