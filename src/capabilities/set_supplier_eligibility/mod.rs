//! Domain Capability / Request Processing Unit: SetSupplierEligibility.
//! Eligibility is a separate fact from supplier identity/registration.

use factstr::{EventFilter, EventQuery, EventStore, EventStoreError, NewEvent};
use serde::Deserialize;
use serde_json::{json, to_value};
use thiserror::Error;
use uuid::Uuid;

use crate::facts::{
    SUPPLIER_ELIGIBILITY_CHANGED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
    SupplierRegisteredFact, SupplierStatus,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetSupplierEligibilityRequest {
    pub status: SupplierStatus,
}

#[derive(Debug, Error)]
pub enum SetSupplierEligibilityError {
    #[error("supplier has not been registered")]
    SupplierNotRegistered,
    #[error("supplier eligibility context changed concurrently; retry the command")]
    ContextChanged,
    #[error("event store error: {0}")]
    Store(String),
}

fn eligibility_context_query(supplier_id: Uuid) -> EventQuery {
    EventQuery::all().with_filters([
        EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
            .with_payload_predicates([json!({ "supplier_id": supplier_id })]),
        EventFilter::for_event_types([SUPPLIER_ELIGIBILITY_CHANGED_V1])
            .with_payload_predicates([json!({ "supplier_id": supplier_id })]),
    ])
}

fn ensure_registered(
    records: &[factstr::EventRecord],
    supplier_id: Uuid,
) -> Result<(), SetSupplierEligibilityError> {
    for record in records {
        if record.event_type == SUPPLIER_REGISTERED_V1 {
            let registration: SupplierRegisteredFact =
                serde_json::from_value(record.payload.clone()).map_err(|error| {
                    SetSupplierEligibilityError::Store(format!(
                        "invalid {SUPPLIER_REGISTERED_V1} fact: {error}"
                    ))
                })?;
            if registration.supplier_id == supplier_id {
                return Ok(());
            }
        }
    }
    Err(SetSupplierEligibilityError::SupplierNotRegistered)
}

pub fn process(
    store: &impl EventStore,
    supplier_id: Uuid,
    request: SetSupplierEligibilityRequest,
) -> Result<SupplierEligibilityChangedFact, SetSupplierEligibilityError> {
    let context_query = eligibility_context_query(supplier_id);
    let loaded = store
        .query(&context_query)
        .map_err(|error| SetSupplierEligibilityError::Store(error.to_string()))?;
    ensure_registered(&loaded.event_records, supplier_id)?;

    let fact = SupplierEligibilityChangedFact {
        supplier_id,
        status: request.status,
        changed_at: chrono::Utc::now(),
    };
    let event = NewEvent::new(
        SUPPLIER_ELIGIBILITY_CHANGED_V1,
        to_value(&fact).map_err(|error| SetSupplierEligibilityError::Store(error.to_string()))?,
    );

    match store.append_if(vec![event], &context_query, loaded.current_context_version) {
        Ok(_) => Ok(fact),
        Err(EventStoreError::ConditionalAppendConflict { .. }) => {
            let current = store
                .query(&context_query)
                .map_err(|error| SetSupplierEligibilityError::Store(error.to_string()))?;
            ensure_registered(&current.event_records, supplier_id)?;
            Err(SetSupplierEligibilityError::ContextChanged)
        }
        Err(error) => Err(SetSupplierEligibilityError::Store(error.to_string())),
    }
}
