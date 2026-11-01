use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use super::{
    error::DefinitionError,
    identity::{FactTypeId, InteractionDefinitionId, ParticipantId},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantDefinition {
    id: ParticipantId,
    dependencies: Vec<ParticipantId>,
    allowed_fact_types: Vec<FactTypeId>,
    allows_empty_fact_set: bool,
}

impl ParticipantDefinition {
    pub fn new(
        id: ParticipantId,
        dependencies: Vec<ParticipantId>,
        allowed_fact_types: Vec<FactTypeId>,
    ) -> Self {
        Self {
            id,
            dependencies,
            allowed_fact_types,
            allows_empty_fact_set: false,
        }
    }

    /// Explicitly permits this participant to produce a valid outcome without facts.
    /// The complete interaction must still contain at least one fact overall.
    pub fn allowing_empty_fact_set(mut self) -> Self {
        self.allows_empty_fact_set = true;
        self
    }

    pub const fn allows_empty_fact_set(&self) -> bool {
        self.allows_empty_fact_set
    }

    pub fn id(&self) -> &ParticipantId {
        &self.id
    }

    pub fn dependencies(&self) -> &[ParticipantId] {
        &self.dependencies
    }

    pub fn allowed_fact_types(&self) -> &[FactTypeId] {
        &self.allowed_fact_types
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionDefinition {
    id: InteractionDefinitionId,
    version: NonZeroU32,
    participants: BTreeMap<ParticipantId, ParticipantDefinition>,
    execution_order: Vec<ParticipantId>,
}

impl InteractionDefinition {
    pub fn new(
        id: InteractionDefinitionId,
        version: NonZeroU32,
        participant_definitions: Vec<ParticipantDefinition>,
    ) -> Result<Self, DefinitionError> {
        if participant_definitions.is_empty() {
            return Err(DefinitionError::EmptyParticipantSet);
        }

        let mut participants = BTreeMap::new();
        for participant in participant_definitions {
            let participant_id = participant.id.clone();
            let mut dependencies = BTreeSet::new();
            for dependency in &participant.dependencies {
                if dependency == &participant.id {
                    return Err(DefinitionError::SelfDependency(participant.id.clone()));
                }
                if !dependencies.insert(dependency.clone()) {
                    return Err(DefinitionError::DuplicateDependency {
                        participant: participant.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }

            let mut allowed_fact_types = BTreeSet::new();
            for fact_type in &participant.allowed_fact_types {
                if !allowed_fact_types.insert(fact_type.clone()) {
                    return Err(DefinitionError::DuplicateAllowedFactType {
                        participant: participant.id.clone(),
                        fact_type: fact_type.clone(),
                    });
                }
            }

            if participants
                .insert(participant_id.clone(), participant)
                .is_some()
            {
                return Err(DefinitionError::DuplicateParticipant(participant_id));
            }
        }

        for participant in participants.values() {
            for dependency in &participant.dependencies {
                if !participants.contains_key(dependency) {
                    return Err(DefinitionError::UnknownDependency {
                        participant: participant.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }

        let execution_order = stable_topological_order(&participants)?;
        Ok(Self {
            id,
            version,
            participants,
            execution_order,
        })
    }

    pub fn id(&self) -> &InteractionDefinitionId {
        &self.id
    }

    pub const fn version(&self) -> NonZeroU32 {
        self.version
    }

    /// Stable topological order. Ties are resolved lexicographically by ParticipantId.
    pub fn execution_order(&self) -> &[ParticipantId] {
        &self.execution_order
    }

    pub fn participant(&self, id: &ParticipantId) -> Option<&ParticipantDefinition> {
        self.participants.get(id)
    }
}

fn stable_topological_order(
    participants: &BTreeMap<ParticipantId, ParticipantDefinition>,
) -> Result<Vec<ParticipantId>, DefinitionError> {
    let mut remaining_dependencies: BTreeMap<ParticipantId, usize> = participants
        .keys()
        .cloned()
        .map(|participant_id| (participant_id, 0))
        .collect();
    let mut dependents: BTreeMap<ParticipantId, Vec<ParticipantId>> = participants
        .keys()
        .cloned()
        .map(|participant_id| (participant_id, Vec::new()))
        .collect();

    for participant in participants.values() {
        *remaining_dependencies
            .get_mut(&participant.id)
            .expect("participant index was created from the same map") =
            participant.dependencies.len();
        for dependency in &participant.dependencies {
            dependents
                .get_mut(dependency)
                .expect("dependencies were validated before sorting")
                .push(participant.id.clone());
        }
    }

    for children in dependents.values_mut() {
        children.sort();
    }

    let mut ready: BTreeSet<ParticipantId> = remaining_dependencies
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(participant_id, _)| participant_id.clone())
        .collect();
    let mut ordered = Vec::with_capacity(participants.len());

    while let Some(participant_id) = ready.pop_first() {
        ordered.push(participant_id.clone());
        let children = dependents
            .get(&participant_id)
            .expect("every declared participant has a dependent list");
        for child in children {
            let count = remaining_dependencies
                .get_mut(child)
                .expect("dependent index was created from the same map");
            *count -= 1;
            if *count == 0 {
                ready.insert(child.clone());
            }
        }
    }

    if ordered.len() != participants.len() {
        let cycle_members = remaining_dependencies
            .into_iter()
            .filter_map(|(participant_id, count)| (count > 0).then_some(participant_id))
            .collect();
        return Err(DefinitionError::DependencyCycle(cycle_members));
    }

    Ok(ordered)
}
