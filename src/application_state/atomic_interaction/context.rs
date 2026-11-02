use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::amri::{Fingerprint, ParticipantId};

const FINGERPRINT_DOMAIN: &[u8] = b"supplier-rfq/amri/composite-context/v1\0";

/// One application-owned read clause: a fact/event type and a structured payload selector.
/// A specification contains alternatives; storage adapters translate the clauses into their
/// own query language. The AMRI Core does not know this application-level query model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextClause {
    event_type: String,
    payload_subset: Value,
    canonical_payload: String,
}

impl ContextClause {
    pub fn new(
        event_type: impl Into<String>,
        payload_subset: Value,
    ) -> Result<Self, ContextPlanError> {
        let event_type = event_type.into();
        if event_type.trim().is_empty() || event_type.trim() != event_type {
            return Err(ContextPlanError::InvalidEventType);
        }
        if !payload_subset.is_object() {
            return Err(ContextPlanError::PayloadSelectorMustBeObject);
        }

        let payload_subset = canonicalize_json(payload_subset);
        let canonical_payload = canonical_json_key(&payload_subset);

        Ok(Self {
            event_type,
            payload_subset,
            canonical_payload,
        })
    }

    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    pub fn payload_subset(&self) -> &Value {
        &self.payload_subset
    }
}

/// Context needs declared by one participant before the state read occurs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextRequirement {
    participant_id: ParticipantId,
    clauses: Vec<ContextClause>,
}

impl ContextRequirement {
    pub fn new(participant_id: ParticipantId, clauses: Vec<ContextClause>) -> Self {
        Self {
            participant_id,
            clauses,
        }
    }

    pub fn participant_id(&self) -> &ParticipantId {
        &self.participant_id
    }
}

/// Canonical union of all participant requirements. It contains no backend version/token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositeContextSpecification {
    participants: Vec<ParticipantId>,
    clauses: Vec<ContextClause>,
    fingerprint: Fingerprint,
}

impl CompositeContextSpecification {
    pub fn union(requirements: &[ContextRequirement]) -> Result<Self, ContextPlanError> {
        if requirements.is_empty() {
            return Err(ContextPlanError::NoParticipantRequirements);
        }

        let mut participants = BTreeSet::new();
        let mut clauses = BTreeMap::<(String, String), ContextClause>::new();

        for requirement in requirements {
            if !participants.insert(requirement.participant_id.clone()) {
                return Err(ContextPlanError::DuplicateParticipantRequirement(
                    requirement.participant_id.clone(),
                ));
            }
            for clause in &requirement.clauses {
                clauses
                    .entry((clause.event_type.clone(), clause.canonical_payload.clone()))
                    .or_insert_with(|| clause.clone());
            }
        }

        if clauses.is_empty() {
            return Err(ContextPlanError::EmptyCompositeContext);
        }

        let participants = participants.into_iter().collect::<Vec<_>>();
        let clauses = clauses.into_values().collect::<Vec<_>>();
        let fingerprint = fingerprint_specification(&participants, &clauses);

        Ok(Self {
            participants,
            clauses,
            fingerprint,
        })
    }

    pub fn participants(&self) -> &[ParticipantId] {
        &self.participants
    }

    pub fn clauses(&self) -> &[ContextClause] {
        &self.clauses
    }

    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextPlanError {
    #[error("at least one participant context requirement is required")]
    NoParticipantRequirements,
    #[error("participant {0} declared context requirements more than once")]
    DuplicateParticipantRequirement(ParticipantId),
    #[error("the union of participant requirements cannot be empty")]
    EmptyCompositeContext,
    #[error("event type must be non-empty and have no leading/trailing whitespace")]
    InvalidEventType,
    #[error("a context payload selector must be a JSON object")]
    PayloadSelectorMustBeObject,
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries = object.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut canonical = Map::new();
            for (key, value) in entries {
                canonical.insert(key, canonicalize_json(value));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        scalar => scalar,
    }
}

/// Produces an unambiguous, type-tagged representation without a fallible serializer.
/// Length prefixes prevent collisions between strings, delimiters and nested structures.
fn canonical_json_key(value: &Value) -> String {
    let mut output = String::new();
    append_canonical_json(value, &mut output);
    output
}

fn append_canonical_json(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push('n'),
        Value::Bool(value) => output.push(if *value { 't' } else { 'f' }),
        Value::Number(value) => {
            output.push('d');
            let _ = write!(output, "{}:", value);
        }
        Value::String(value) => {
            output.push('s');
            let _ = write!(output, "{}:", value.len());
            output.push_str(value);
        }
        Value::Array(values) => {
            output.push('[');
            let _ = write!(output, "{}:", values.len());
            for value in values {
                append_canonical_json(value, output);
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let _ = write!(output, "{}:", values.len());
            for (key, value) in values {
                let _ = write!(output, "{}:", key.len());
                output.push_str(key);
                append_canonical_json(value, output);
            }
            output.push('}');
        }
    }
}

fn fingerprint_specification(
    participants: &[ParticipantId],
    clauses: &[ContextClause],
) -> Fingerprint {
    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_DOMAIN);
    hasher.update((participants.len() as u64).to_be_bytes());
    for participant in participants {
        update_len_prefixed(&mut hasher, participant.as_str().as_bytes());
    }
    hasher.update((clauses.len() as u64).to_be_bytes());
    for clause in clauses {
        update_len_prefixed(&mut hasher, clause.event_type.as_bytes());
        update_len_prefixed(&mut hasher, clause.canonical_payload.as_bytes());
    }
    Fingerprint::from_bytes(hasher.finalize().into())
}

fn update_len_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn participant(value: &str) -> ParticipantId {
        ParticipantId::new(value).expect("valid participant id")
    }

    fn clause(event_type: &str, payload: Value) -> ContextClause {
        ContextClause::new(event_type, payload).expect("valid context clause")
    }

    #[test]
    fn union_is_order_independent_and_deduplicates_identical_clauses() {
        let shared = clause("rfq.opened.v1", json!({"rfq_id":"RFQ-1","tenant":"T1"}));
        let supplier = clause("supplier.registered.v1", json!({"supplier_id":"SUP-1"}));
        let first = [
            ContextRequirement::new(participant("close-rfq"), vec![shared.clone()]),
            ContextRequirement::new(
                participant("award-quote"),
                vec![shared.clone(), supplier.clone()],
            ),
        ];
        let second = [
            ContextRequirement::new(participant("award-quote"), vec![supplier, shared.clone()]),
            ContextRequirement::new(participant("close-rfq"), vec![shared]),
        ];

        let left = CompositeContextSpecification::union(&first).unwrap();
        let right = CompositeContextSpecification::union(&second).unwrap();

        assert_eq!(left, right);
        assert_eq!(left.clauses().len(), 2);
        assert_eq!(left.participants().len(), 2);
    }

    #[test]
    fn object_key_order_does_not_change_the_specification_fingerprint() {
        let first = clause(
            "rfq.opened.v1",
            json!({"rfq_id":"RFQ-1","tenant":{"id":"T1","region":"BR"}}),
        );
        let second = clause(
            "rfq.opened.v1",
            json!({"tenant":{"region":"BR","id":"T1"},"rfq_id":"RFQ-1"}),
        );
        let left = CompositeContextSpecification::union(&[ContextRequirement::new(
            participant("close-rfq"),
            vec![first],
        )])
        .unwrap();
        let right = CompositeContextSpecification::union(&[ContextRequirement::new(
            participant("close-rfq"),
            vec![second],
        )])
        .unwrap();

        assert_eq!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn composite_specification_keeps_each_payload_conjunction_as_one_object() {
        let specification = CompositeContextSpecification::union(&[
            ContextRequirement::new(
                participant("close-rfq"),
                vec![clause(
                    "rfq.opened.v1",
                    json!({"rfq_id":"RFQ-1","tenant":"T1"}),
                )],
            ),
            ContextRequirement::new(
                participant("award-quote"),
                vec![clause(
                    "supplier.registered.v1",
                    json!({"supplier_id":"SUP-1"}),
                )],
            ),
        ])
        .unwrap();

        assert_eq!(specification.clauses().len(), 2);
        assert_eq!(
            specification.clauses()[0].payload_subset(),
            &json!({"rfq_id":"RFQ-1","tenant":"T1"})
        );
        assert_eq!(
            specification.clauses()[1].payload_subset(),
            &json!({"supplier_id":"SUP-1"})
        );
    }

    #[test]
    fn empty_or_ambiguous_requirements_are_rejected() {
        assert_eq!(
            CompositeContextSpecification::union(&[]),
            Err(ContextPlanError::NoParticipantRequirements)
        );
        assert_eq!(
            CompositeContextSpecification::union(&[ContextRequirement::new(
                participant("close-rfq"),
                vec![],
            )]),
            Err(ContextPlanError::EmptyCompositeContext)
        );
        assert_eq!(
            ContextClause::new("rfq.opened.v1", json!("RFQ-1")),
            Err(ContextPlanError::PayloadSelectorMustBeObject)
        );
        for invalid in ["", " ", " rfq.opened.v1", "rfq.opened.v1 "] {
            assert_eq!(
                ContextClause::new(invalid, json!({})),
                Err(ContextPlanError::InvalidEventType)
            );
        }
    }

    #[test]
    fn duplicate_participant_requirement_is_rejected_even_if_clauses_match() {
        let requirement = ContextRequirement::new(
            participant("close-rfq"),
            vec![clause("rfq.opened.v1", json!({"rfq_id":"RFQ-1"}))],
        );
        assert!(matches!(
            CompositeContextSpecification::union(&[requirement.clone(), requirement]),
            Err(ContextPlanError::DuplicateParticipantRequirement(_))
        ));
    }

    #[test]
    fn clause_exposes_application_owned_event_and_payload_selector() {
        let clause = clause("rfq.opened.v1", json!({"rfq_id":"RFQ-1","tenant":"T1"}));
        assert_eq!(clause.event_type(), "rfq.opened.v1");
        assert_eq!(
            clause.payload_subset(),
            &json!({"rfq_id":"RFQ-1","tenant":"T1"})
        );
    }

    #[test]
    fn canonicalization_recurses_through_arrays_and_nested_objects() {
        let first = clause(
            "rfq.opened.v1",
            json!({"selector":[{"tenant":{"id":"T1","region":"BR"}}, {"status":"open"}, 7, true, false, null]}),
        );
        let second = clause(
            "rfq.opened.v1",
            json!({"selector":[{"tenant":{"region":"BR","id":"T1"}}, {"status":"open"}, 7, true, false, null]}),
        );
        let left = CompositeContextSpecification::union(&[ContextRequirement::new(
            participant("close-rfq"),
            vec![first],
        )])
        .unwrap();
        let right = CompositeContextSpecification::union(&[ContextRequirement::new(
            participant("close-rfq"),
            vec![second],
        )])
        .unwrap();
        assert_eq!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn canonical_json_key_preserves_type_tags_lengths_and_nested_content() {
        // This locks the documented type-tagged, length-prefixed representation.
        // It catches erased output, constant output, and a no-op recursive writer.
        let value = json!({"name":"rfq","selector":[null,true,7,"a"]});
        assert_eq!(
            canonical_json_key(&value),
            "{2:4:names3:rfq8:selector[4:ntd7:s1:a]}"
        );

        // Length prefixes make otherwise ambiguous concatenations distinct.
        assert_ne!(
            canonical_json_key(&json!(["ab", "c"])),
            canonical_json_key(&json!(["a", "bc"]))
        );
    }

    #[test]
    fn union_keeps_distinct_payload_selectors_for_the_same_event_type() {
        let specification = CompositeContextSpecification::union(&[ContextRequirement::new(
            participant("close-rfq"),
            vec![
                clause("rfq.opened.v1", json!({"rfq_id":"RFQ-1"})),
                clause("rfq.opened.v1", json!({"rfq_id":"RFQ-2"})),
            ],
        )])
        .unwrap();

        assert_eq!(specification.clauses().len(), 2);
        assert!(
            specification
                .clauses()
                .iter()
                .any(|clause| clause.payload_subset() == &json!({"rfq_id":"RFQ-1"}))
        );
        assert!(
            specification
                .clauses()
                .iter()
                .any(|clause| clause.payload_subset() == &json!({"rfq_id":"RFQ-2"}))
        );
    }

    #[test]
    fn specification_fingerprint_changes_when_participant_or_clause_data_changes() {
        fn specification(
            participant_id: &str,
            event_type: &str,
            payload: Value,
        ) -> CompositeContextSpecification {
            CompositeContextSpecification::union(&[ContextRequirement::new(
                participant(participant_id),
                vec![clause(event_type, payload)],
            )])
            .unwrap()
        }

        let baseline = specification("close-rfq", "rfq.opened.v1", json!({"rfq_id":"RFQ-1"}));
        let changed_participant =
            specification("award-quote", "rfq.opened.v1", json!({"rfq_id":"RFQ-1"}));
        let changed_event_type =
            specification("close-rfq", "rfq.closed.v1", json!({"rfq_id":"RFQ-1"}));
        let changed_payload =
            specification("close-rfq", "rfq.opened.v1", json!({"rfq_id":"RFQ-2"}));

        assert_ne!(baseline.fingerprint(), changed_participant.fingerprint());
        assert_ne!(baseline.fingerprint(), changed_event_type.fingerprint());
        assert_ne!(baseline.fingerprint(), changed_payload.fingerprint());
    }
}
