use std::num::NonZeroU32;

use proptest::prelude::*;

use super::{
    DefinitionError, ExecutionContext, FactId, FactTypeId, Fingerprint, FingerprintError,
    IdentityReuseError, InteractionDefinition, InteractionDefinitionId, InteractionId,
    InteractionIdentity, ParticipantDefinition, ParticipantId, PlanValidationError,
    PreparedOutcome, ProposedFact,
};

fn interaction_id(value: &str) -> InteractionId {
    InteractionId::new(value).expect("test interaction id is valid")
}

fn definition_id(value: &str) -> InteractionDefinitionId {
    InteractionDefinitionId::new(value).expect("test definition id is valid")
}

fn participant_id(value: &str) -> ParticipantId {
    ParticipantId::new(value).expect("test participant id is valid")
}

fn fact_id(value: &str) -> FactId {
    FactId::new(value).expect("test fact id is valid")
}

fn fact_type(value: &str) -> FactTypeId {
    FactTypeId::new(value).expect("test fact type is valid")
}

fn fingerprint(byte: u8) -> Fingerprint {
    Fingerprint::from_bytes([byte; 32])
}

fn identity(definition: &InteractionDefinition) -> InteractionIdentity {
    InteractionIdentity::new(
        interaction_id("interaction-1"),
        definition.id().clone(),
        definition.version(),
        fingerprint(1),
    )
}

fn execution(definition: &InteractionDefinition) -> ExecutionContext {
    ExecutionContext::new(identity(definition), 1_798_000_000_000, None)
}

fn participant(
    id: &str,
    dependencies: &[&str],
    allowed_fact_types: &[&str],
) -> ParticipantDefinition {
    ParticipantDefinition::new(
        participant_id(id),
        dependencies
            .iter()
            .map(|value| participant_id(value))
            .collect(),
        allowed_fact_types
            .iter()
            .map(|value| fact_type(value))
            .collect(),
    )
}

fn three_participant_definition() -> InteractionDefinition {
    InteractionDefinition::new(
        definition_id("sample.interaction.v1"),
        NonZeroU32::new(1).expect("version is non-zero"),
        vec![
            participant("participant-a", &[], &["fact-type-a.v1"]),
            participant("participant-b", &["participant-a"], &["fact-type-b.v1"]),
            participant("participant-c", &["participant-b"], &["fact-type-c.v1"]),
        ],
    )
    .expect("test participant graph is valid")
}

fn prepared(
    participant: &str,
    fact_id_value: &str,
    fact_type_value: &str,
    payload: u8,
) -> PreparedOutcome<u8, u8> {
    PreparedOutcome::new(
        participant_id(participant),
        payload,
        vec![ProposedFact::new(
            fact_id(fact_id_value),
            fact_type(fact_type_value),
            payload,
        )],
    )
}

#[test]
fn identifiers_reject_empty_whitespace_unicode_and_overlong_values() {
    assert_eq!(ParticipantId::new(""), Err(super::IdentifierError::Empty));
    assert_eq!(
        ParticipantId::new("has space"),
        Err(super::IdentifierError::InvalidCharacter)
    );
    assert_eq!(
        ParticipantId::new("çapacidade"),
        Err(super::IdentifierError::InvalidCharacter)
    );
    assert_eq!(
        ParticipantId::new("x".repeat(129)),
        Err(super::IdentifierError::TooLong)
    );
    assert_eq!(
        ParticipantId::new(" participant"),
        Err(super::IdentifierError::InvalidCharacter)
    );
}

#[test]
fn identifiers_accept_stable_namespaced_ascii_identifiers() {
    let id = ParticipantId::new("sample.award-resource:v1/primary").unwrap();
    assert_eq!(id.as_str(), "sample.award-resource:v1/primary");
    assert_eq!(id.to_string(), "sample.award-resource:v1/primary");
    assert_eq!(
        format!("{id:?}"),
        "ParticipantId(\"sample.award-resource:v1/primary\")"
    );
}

#[test]
fn fingerprint_hex_round_trips_lower_and_upper_case() {
    let original = fingerprint(0xab);
    let lower = original.to_hex();
    let upper = lower.to_ascii_uppercase();
    assert_eq!(Fingerprint::from_hex(&lower), Ok(original));
    assert_eq!(Fingerprint::from_hex(&upper), Ok(original));
    let mixed_hex = "0123456789abcdef".repeat(4);
    assert_eq!(
        Fingerprint::from_hex(&mixed_hex).unwrap().to_hex(),
        mixed_hex
    );
    assert_eq!(original.as_bytes(), &[0xab; 32]);
}

#[test]
fn fingerprint_hex_rejects_wrong_length_and_invalid_characters() {
    assert_eq!(
        Fingerprint::from_hex("abc"),
        Err(FingerprintError::InvalidLength)
    );
    assert_eq!(
        Fingerprint::from_hex(&"g".repeat(64)),
        Err(FingerprintError::InvalidHex)
    );
    let invalid_low_nibble = format!("0g{}", "0".repeat(62));
    assert_eq!(
        Fingerprint::from_hex(&invalid_low_nibble),
        Err(FingerprintError::InvalidHex)
    );
    assert_eq!(
        Fingerprint::from_hex(&"é".repeat(32)),
        Err(FingerprintError::InvalidHex)
    );
}

#[test]
fn same_interaction_identity_requires_same_definition_version_and_request_fingerprint() {
    let base = InteractionIdentity::new(
        interaction_id("same-id"),
        definition_id("definition-a"),
        NonZeroU32::new(1).unwrap(),
        fingerprint(1),
    );
    let same = base.clone();
    assert_eq!(base.ensure_same_logical_request(&same), Ok(()));
    assert_eq!(base.request_fingerprint(), fingerprint(1));

    let different_interaction = InteractionIdentity::new(
        interaction_id("other-id"),
        definition_id("definition-a"),
        NonZeroU32::new(1).unwrap(),
        fingerprint(1),
    );
    assert_eq!(
        base.ensure_same_logical_request(&different_interaction),
        Err(IdentityReuseError::DifferentInteractionId)
    );

    let different_definition = InteractionIdentity::new(
        interaction_id("same-id"),
        definition_id("definition-b"),
        NonZeroU32::new(1).unwrap(),
        fingerprint(1),
    );
    assert_eq!(
        base.ensure_same_logical_request(&different_definition),
        Err(IdentityReuseError::DifferentDefinition)
    );

    let different_version = InteractionIdentity::new(
        interaction_id("same-id"),
        definition_id("definition-a"),
        NonZeroU32::new(2).unwrap(),
        fingerprint(1),
    );
    assert_eq!(
        base.ensure_same_logical_request(&different_version),
        Err(IdentityReuseError::DifferentDefinitionVersion)
    );

    let different_request = InteractionIdentity::new(
        interaction_id("same-id"),
        definition_id("definition-a"),
        NonZeroU32::new(1).unwrap(),
        fingerprint(2),
    );
    assert_eq!(
        base.ensure_same_logical_request(&different_request),
        Err(IdentityReuseError::RequestFingerprintMismatch)
    );
}

#[test]
fn execution_context_preserves_captured_metadata_without_reading_a_clock() {
    let definition = three_participant_definition();
    let context = ExecutionContext::new(
        identity(&definition),
        1_798_000_000_123,
        Some(super::CorrelationId::new("trace-123").unwrap()),
    );
    assert_eq!(context.captured_at_unix_ms(), 1_798_000_000_123);
    assert_eq!(context.correlation_id().unwrap().as_str(), "trace-123");
    assert_eq!(
        context.identity().interaction_id().as_str(),
        "interaction-1"
    );
}

#[test]
fn interaction_definition_rejects_empty_participant_set() {
    assert_eq!(
        InteractionDefinition::new(definition_id("empty"), NonZeroU32::new(1).unwrap(), vec![],),
        Err(DefinitionError::EmptyParticipantSet)
    );
}

#[test]
fn interaction_definition_rejects_duplicate_participants() {
    let result = InteractionDefinition::new(
        definition_id("duplicate"),
        NonZeroU32::new(1).unwrap(),
        vec![
            participant("same", &[], &["fact.v1"]),
            participant("same", &[], &["fact.v1"]),
        ],
    );
    assert!(matches!(
        result,
        Err(DefinitionError::DuplicateParticipant(id)) if id.as_str() == "same"
    ));
}

#[test]
fn interaction_definition_rejects_self_and_duplicate_dependencies() {
    let self_dependency = InteractionDefinition::new(
        definition_id("self"),
        NonZeroU32::new(1).unwrap(),
        vec![participant("a", &["a"], &["fact.v1"])],
    );
    assert!(matches!(
        self_dependency,
        Err(DefinitionError::SelfDependency(id)) if id.as_str() == "a"
    ));

    let duplicate_dependency = InteractionDefinition::new(
        definition_id("duplicate-dependency"),
        NonZeroU32::new(1).unwrap(),
        vec![
            participant("a", &[], &["fact.v1"]),
            participant("b", &["a", "a"], &["fact.v1"]),
        ],
    );
    assert!(matches!(
        duplicate_dependency,
        Err(DefinitionError::DuplicateDependency { participant, dependency })
            if participant.as_str() == "b" && dependency.as_str() == "a"
    ));
}

#[test]
fn interaction_definition_rejects_unknown_dependencies_and_duplicate_fact_types() {
    let unknown = InteractionDefinition::new(
        definition_id("unknown-dependency"),
        NonZeroU32::new(1).unwrap(),
        vec![participant("b", &["missing"], &["fact.v1"])],
    );
    assert!(matches!(
        unknown,
        Err(DefinitionError::UnknownDependency { participant, dependency })
            if participant.as_str() == "b" && dependency.as_str() == "missing"
    ));

    let duplicate_fact_type = InteractionDefinition::new(
        definition_id("duplicate-fact-type"),
        NonZeroU32::new(1).unwrap(),
        vec![participant("a", &[], &["fact.v1", "fact.v1"])],
    );
    assert!(matches!(
        duplicate_fact_type,
        Err(DefinitionError::DuplicateAllowedFactType { participant, fact_type })
            if participant.as_str() == "a" && fact_type.as_str() == "fact.v1"
    ));
}

#[test]
fn interaction_definition_rejects_cycles_and_uses_stable_topological_order() {
    let cycle = InteractionDefinition::new(
        definition_id("cycle"),
        NonZeroU32::new(1).unwrap(),
        vec![
            participant("a", &["c"], &["fact.v1"]),
            participant("b", &["a"], &["fact.v1"]),
            participant("c", &["b"], &["fact.v1"]),
        ],
    );
    assert!(matches!(cycle, Err(DefinitionError::DependencyCycle(members)) if members.len() == 3));

    let stable = InteractionDefinition::new(
        definition_id("stable"),
        NonZeroU32::new(1).unwrap(),
        vec![
            participant("z", &[], &["fact.v1"]),
            participant("b", &[], &["fact.v1"]),
            participant("a", &[], &["fact.v1"]),
        ],
    )
    .unwrap();
    let order = stable
        .execution_order()
        .iter()
        .map(ParticipantId::as_str)
        .collect::<Vec<_>>();
    assert_eq!(order, vec!["a", "b", "z"]);
}

#[test]
fn valid_plan_orders_outcomes_and_facts_by_the_declared_dependency_graph() {
    let definition = three_participant_definition();
    let execution_context = execution(&definition);
    let plan = definition
        .validate_prepared_outcomes(
            execution_context.clone(),
            vec![
                prepared("participant-c", "fact-c", "fact-type-c.v1", 3),
                prepared("participant-b", "fact-b", "fact-type-b.v1", 2),
                prepared("participant-a", "fact-a", "fact-type-a.v1", 1),
            ],
            fingerprint(9),
        )
        .unwrap();

    assert_eq!(plan.execution(), &execution_context);
    assert_eq!(plan.summary().participant_count(), 3);
    assert_eq!(plan.summary().fact_count(), 3);
    assert_eq!(
        plan.summary()
            .participant_order()
            .iter()
            .map(ParticipantId::as_str)
            .collect::<Vec<_>>(),
        vec!["participant-a", "participant-b", "participant-c"]
    );
    assert_eq!(plan.plan_fingerprint(), fingerprint(9));
    assert_eq!(
        plan.outcomes()
            .iter()
            .map(|item| item.participant_id().as_str())
            .collect::<Vec<_>>(),
        vec!["participant-a", "participant-b", "participant-c"]
    );
    assert_eq!(
        plan.facts()
            .map(|item| item.fact().id().as_str())
            .collect::<Vec<_>>(),
        vec!["fact-a", "fact-b", "fact-c"]
    );
    assert_eq!(plan.outcomes()[0].output(), &1);
}

#[test]
fn plan_rejects_definition_identity_and_version_mismatches() {
    let definition = three_participant_definition();
    let wrong_id = ExecutionContext::new(
        InteractionIdentity::new(
            interaction_id("interaction-1"),
            definition_id("other-definition"),
            NonZeroU32::new(1).unwrap(),
            fingerprint(1),
        ),
        0,
        None,
    );
    assert!(matches!(
        definition.validate_prepared_outcomes(
            wrong_id,
            Vec::<PreparedOutcome<(), ()>>::new(),
            fingerprint(1)
        ),
        Err(PlanValidationError::DefinitionIdMismatch { .. })
    ));

    let wrong_version = ExecutionContext::new(
        InteractionIdentity::new(
            interaction_id("interaction-1"),
            definition.id().clone(),
            NonZeroU32::new(2).unwrap(),
            fingerprint(1),
        ),
        0,
        None,
    );
    assert!(matches!(
        definition.validate_prepared_outcomes(
            wrong_version,
            Vec::<PreparedOutcome<(), ()>>::new(),
            fingerprint(1)
        ),
        Err(PlanValidationError::DefinitionVersionMismatch { .. })
    ));
}

#[test]
fn plan_rejects_unknown_duplicate_and_missing_participants() {
    let definition = three_participant_definition();
    let unknown = definition.validate_prepared_outcomes(
        execution(&definition),
        vec![prepared("unknown", "f", "fact-type-a.v1", 1)],
        fingerprint(1),
    );
    assert!(matches!(
        unknown,
        Err(PlanValidationError::UnknownParticipant(id)) if id.as_str() == "unknown"
    ));

    let duplicate = definition.validate_prepared_outcomes(
        execution(&definition),
        vec![
            prepared("participant-a", "f1", "fact-type-a.v1", 1),
            prepared("participant-a", "f2", "fact-type-a.v1", 1),
        ],
        fingerprint(1),
    );
    assert!(matches!(
        duplicate,
        Err(PlanValidationError::DuplicateOutcome(id)) if id.as_str() == "participant-a"
    ));

    let missing = definition.validate_prepared_outcomes(
        execution(&definition),
        vec![prepared("participant-a", "f1", "fact-type-a.v1", 1)],
        fingerprint(1),
    );
    assert!(matches!(
        missing,
        Err(PlanValidationError::MissingOutcome(id)) if id.as_str() == "participant-b"
    ));
}

#[test]
fn plan_rejects_disallowed_types_duplicate_fact_ids_and_empty_fact_batches() {
    let definition = three_participant_definition();
    let disallowed = definition.validate_prepared_outcomes(
        execution(&definition),
        vec![
            prepared("participant-a", "f1", "fact-type-unlisted.v1", 1),
            prepared("participant-b", "f2", "fact-type-b.v1", 2),
            prepared("participant-c", "f3", "fact-type-c.v1", 3),
        ],
        fingerprint(1),
    );
    assert!(matches!(
        disallowed,
        Err(PlanValidationError::DisallowedFactType { participant, .. })
            if participant.as_str() == "participant-a"
    ));

    let duplicate_fact = definition.validate_prepared_outcomes(
        execution(&definition),
        vec![
            prepared("participant-a", "same-fact", "fact-type-a.v1", 1),
            prepared("participant-b", "same-fact", "fact-type-b.v1", 2),
            prepared("participant-c", "f3", "fact-type-c.v1", 3),
        ],
        fingerprint(1),
    );
    assert!(matches!(
        duplicate_fact,
        Err(PlanValidationError::DuplicateFactId(id)) if id.as_str() == "same-fact"
    ));

    let no_facts_definition = InteractionDefinition::new(
        definition_id("allow-empty-participants"),
        NonZeroU32::new(1).unwrap(),
        vec![
            participant("participant-a", &[], &["fact-type-a.v1"]).allowing_empty_fact_set(),
            participant("participant-b", &["participant-a"], &["fact-type-b.v1"])
                .allowing_empty_fact_set(),
            participant("participant-c", &["participant-b"], &["fact-type-c.v1"])
                .allowing_empty_fact_set(),
        ],
    )
    .unwrap();
    let empty = no_facts_definition.validate_prepared_outcomes(
        execution(&no_facts_definition),
        vec![
            PreparedOutcome::<u8, u8>::new(participant_id("participant-a"), 1, vec![]),
            PreparedOutcome::<u8, u8>::new(participant_id("participant-b"), 2, vec![]),
            PreparedOutcome::<u8, u8>::new(participant_id("participant-c"), 3, vec![]),
        ],
        fingerprint(1),
    );
    assert_eq!(empty, Err(PlanValidationError::EmptyFactBatch));
}

#[test]
fn participant_without_empty_fact_opt_in_cannot_return_no_facts() {
    let definition = three_participant_definition();
    let result = definition.validate_prepared_outcomes(
        execution(&definition),
        vec![
            PreparedOutcome::<u8, u8>::new(participant_id("participant-a"), 1, vec![]),
            prepared("participant-b", "fact-b", "fact-type-b.v1", 2),
            prepared("participant-c", "fact-c", "fact-type-c.v1", 3),
        ],
        fingerprint(1),
    );
    assert_eq!(
        result,
        Err(PlanValidationError::EmptyParticipantFactSet(
            participant_id("participant-a",)
        ))
    );
}

#[test]
fn public_identity_and_error_accessors_are_stable_and_format_without_leaking_payloads() {
    let interaction = interaction_id("interaction-format");
    let definition = definition_id("definition-format");
    let participant = participant_id("participant-format");
    let fact = fact_id("fact-format");
    let fact_kind = fact_type("fact-kind.v1");
    let correlation = super::CorrelationId::new("correlation-format").unwrap();

    assert_eq!(interaction.as_ref(), "interaction-format");
    assert_eq!(interaction.to_string(), "interaction-format");
    assert!(format!("{interaction:?}").contains("InteractionId"));
    assert_eq!(definition.as_ref(), "definition-format");
    assert_eq!(definition.to_string(), "definition-format");
    assert!(format!("{definition:?}").contains("InteractionDefinitionId"));
    assert_eq!(participant.as_ref(), "participant-format");
    assert_eq!(participant.to_string(), "participant-format");
    assert!(format!("{participant:?}").contains("ParticipantId"));
    assert_eq!(fact.as_ref(), "fact-format");
    assert_eq!(fact.to_string(), "fact-format");
    assert!(format!("{fact:?}").contains("FactId"));
    assert_eq!(fact_kind.as_ref(), "fact-kind.v1");
    assert_eq!(fact_kind.to_string(), "fact-kind.v1");
    assert!(format!("{fact_kind:?}").contains("FactTypeId"));
    assert_eq!(correlation.as_ref(), "correlation-format");
    assert_eq!(correlation.to_string(), "correlation-format");
    assert!(format!("{correlation:?}").contains("CorrelationId"));
    assert!(format!("{:?}", fingerprint(4)).contains("Fingerprint"));
    assert_eq!(fingerprint(4).to_string(), "04".repeat(32));

    assert!(!format!("{}", super::IdentifierError::InvalidCharacter).is_empty());
    assert!(!format!("{}", FingerprintError::InvalidHex).is_empty());
    assert!(!format!("{}", IdentityReuseError::RequestFingerprintMismatch).is_empty());
    assert!(!format!("{}", DefinitionError::EmptyParticipantSet).is_empty());
    assert!(!format!("{}", PlanValidationError::EmptyFactBatch).is_empty());
}

#[test]
fn participant_definition_exposes_declared_dependencies_and_fact_contracts() {
    let definition = participant("downstream", &["upstream"], &["fact.v1", "fact.v2"]);
    assert_eq!(definition.id().as_str(), "downstream");
    assert_eq!(
        definition
            .dependencies()
            .iter()
            .map(ParticipantId::as_str)
            .collect::<Vec<_>>(),
        vec!["upstream"]
    );
    assert_eq!(definition.allowed_fact_types().len(), 2);
    assert!(!definition.allows_empty_fact_set());
    assert!(definition.allowing_empty_fact_set().allows_empty_fact_set());
}

#[test]
fn topological_sort_handles_multiple_prerequisites_and_stable_ready_queue() {
    let definition = InteractionDefinition::new(
        definition_id("multi-prerequisite"),
        NonZeroU32::new(1).unwrap(),
        vec![
            participant("c", &["a", "b"], &["fact.c"]),
            participant("b", &[], &["fact.b"]),
            participant("a", &[], &["fact.a"]),
            participant("d", &[], &["fact.d"]),
        ],
    )
    .unwrap();
    let order = definition
        .execution_order()
        .iter()
        .map(ParticipantId::as_str)
        .collect::<Vec<_>>();
    assert_eq!(order, vec!["a", "b", "c", "d"]);
}

#[test]
fn planned_fact_reference_exposes_owner_and_typed_payload() {
    let definition = three_participant_definition();
    let plan = definition
        .validate_prepared_outcomes(
            execution(&definition),
            vec![
                prepared("participant-a", "fact-a", "fact-type-a.v1", 11),
                prepared("participant-b", "fact-b", "fact-type-b.v1", 22),
                prepared("participant-c", "fact-c", "fact-type-c.v1", 33),
            ],
            fingerprint(7),
        )
        .unwrap();
    let first = plan.facts().next().unwrap();
    assert_eq!(first.participant_id().as_str(), "participant-a");
    assert_eq!(first.fact().payload(), &11);
    assert_eq!(first.fact().fact_type().as_str(), "fact-type-a.v1");
}

#[test]
fn dependency_cycle_diagnostic_excludes_resolved_participants() {
    // An independent participant is processed successfully while the other three
    // participants remain in a cycle. It must not be reported as a cycle member.
    let result = InteractionDefinition::new(
        definition_id("cycle-with-independent-participant"),
        NonZeroU32::new(1).unwrap(),
        vec![
            participant("a", &["c"], &["fact.v1"]),
            participant("b", &["a"], &["fact.v1"]),
            participant("c", &["b"], &["fact.v1"]),
            participant("independent", &[], &["fact.v1"]),
        ],
    );

    let members = match result {
        Err(DefinitionError::DependencyCycle(members)) => members,
        other => panic!("expected dependency-cycle error, got {other:?}"),
    };
    assert_eq!(
        members
            .iter()
            .map(ParticipantId::as_str)
            .collect::<Vec<_>>(),
        vec!["a", "b", "c"]
    );
}

#[test]
fn identifier_accepts_exact_maximum_byte_length() {
    let identifier = ParticipantId::new("x".repeat(128))
        .expect("the configured 128-byte identifier limit is inclusive");
    assert_eq!(identifier.as_str().len(), 128);
}

proptest! {
    #[test]
    fn plan_order_is_independent_of_prepared_outcome_input_permutation(
        order in prop::collection::vec(0usize..3, 3).prop_filter("must be a permutation", |values| {
            let mut sorted = values.clone();
            sorted.sort_unstable();
            sorted == vec![0, 1, 2]
        })
    ) {
        let definition = three_participant_definition();
        let all = [
            prepared("participant-a", "fact-a", "fact-type-a.v1", 1),
            prepared("participant-b", "fact-b", "fact-type-b.v1", 2),
            prepared("participant-c", "fact-c", "fact-type-c.v1", 3),
        ];
        let outcomes = order.into_iter().map(|index| all[index].clone()).collect();
        let plan = definition
            .validate_prepared_outcomes(execution(&definition), outcomes, fingerprint(5))
            .unwrap();
        prop_assert_eq!(
            plan.outcomes().iter().map(|item| item.participant_id().as_str()).collect::<Vec<_>>(),
            vec!["participant-a", "participant-b", "participant-c"]
        );
        prop_assert_eq!(
            plan.facts().map(|item| item.fact().id().as_str()).collect::<Vec<_>>(),
            vec!["fact-a", "fact-b", "fact-c"]
        );
    }


    #[test]
    fn duplicate_fact_ids_are_rejected_for_all_valid_identifier_values(
        shared_id in prop::string::string_regex("[a-z][a-z0-9_-]{0,24}").unwrap()
    ) {
        let definition = three_participant_definition();
        let duplicate = FactId::new(shared_id).unwrap();
        let outcome = |participant: &str, kind: &str, payload: u8| {
            PreparedOutcome::new(
                participant_id(participant),
                payload,
                vec![ProposedFact::new(duplicate.clone(), fact_type(kind), payload)],
            )
        };
        let result = definition.validate_prepared_outcomes(
            execution(&definition),
            vec![
                outcome("participant-a", "fact-type-a.v1", 1),
                outcome("participant-b", "fact-type-b.v1", 2),
                prepared("participant-c", "fact-c", "fact-type-c.v1", 3),
            ],
            fingerprint(6),
        );
        prop_assert!(matches!(result, Err(PlanValidationError::DuplicateFactId(_))));
    }

    #[test]
    fn every_missing_required_participant_prevents_a_validated_plan(
        missing_index in 0usize..3
    ) {
        let definition = three_participant_definition();
        let all = [
            prepared("participant-a", "fact-a", "fact-type-a.v1", 1),
            prepared("participant-b", "fact-b", "fact-type-b.v1", 2),
            prepared("participant-c", "fact-c", "fact-type-c.v1", 3),
        ];
        let outcomes = all.into_iter().enumerate()
            .filter_map(|(index, outcome)| (index != missing_index).then_some(outcome))
            .collect();
        let result = definition.validate_prepared_outcomes(
            execution(&definition), outcomes, fingerprint(8)
        );
        prop_assert!(matches!(result, Err(PlanValidationError::MissingOutcome(_))));
    }
}
