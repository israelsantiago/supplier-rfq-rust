use factstr::{EventFilter, EventQuery, EventStore};
use serde_json::json;
use uuid::Uuid;

use crate::events::{
    SUPPLIER_ELIGIBILITY_CHANGED_V1, SUPPLIER_REGISTERED_V1, SupplierRegisteredFact,
};

use super::{build_context::EligibilityContextFact, process_request::SetSupplierEligibilityError};

pub(super) struct LoadedEligibilityContext {
    pub(super) facts: Vec<EligibilityContextFact>,
    pub(super) current_context_version: Option<u64>,
}

pub(super) fn eligibility_context_query(supplier_id: Uuid) -> EventQuery {
    EventQuery::all().with_filters([
        EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
            .with_payload_predicates([json!({ "supplier_id": supplier_id })]),
        EventFilter::for_event_types([SUPPLIER_ELIGIBILITY_CHANGED_V1])
            .with_payload_predicates([json!({ "supplier_id": supplier_id })]),
    ])
}

pub(super) fn load_context(
    store: &impl EventStore,
    query: &EventQuery,
) -> Result<LoadedEligibilityContext, SetSupplierEligibilityError> {
    let result = store
        .query(query)
        .map_err(|error| SetSupplierEligibilityError::Store(error.to_string()))?;
    let mut facts = Vec::with_capacity(result.event_records.len());

    for record in result.event_records {
        match record.event_type.as_str() {
            SUPPLIER_REGISTERED_V1 => {
                let fact: SupplierRegisteredFact =
                    serde_json::from_value(record.payload).map_err(|error| {
                        SetSupplierEligibilityError::Store(format!(
                            "invalid {SUPPLIER_REGISTERED_V1} fact: {error}"
                        ))
                    })?;
                facts.push(EligibilityContextFact::Registered(fact.supplier_id));
            }
            SUPPLIER_ELIGIBILITY_CHANGED_V1 => {
                // Keep this event inside the same CCC context even though registration
                // is the only fact required by this decision.
                facts.push(EligibilityContextFact::EligibilityChanged);
            }
            _ => {}
        }
    }

    Ok(LoadedEligibilityContext {
        facts,
        current_context_version: result.current_context_version,
    })
}
