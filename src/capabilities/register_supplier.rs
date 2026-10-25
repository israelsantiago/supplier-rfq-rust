//! Domain Capability / Request Processing Unit: RegisterSupplier.
//! Supplier identity and natural-reference uniqueness are protected by CCC.

use factstr::{EventFilter, EventQuery, EventStore, EventStoreError, NewEvent};
use serde::Deserialize;
use serde_json::{json, to_value};
use thiserror::Error;
use uuid::Uuid;

use crate::facts::{SUPPLIER_REGISTERED_V1, SupplierRegisteredFact};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterSupplierRequest {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub reference: String,
    pub name: String,
}

#[derive(Debug, Error)]
pub enum RegisterSupplierError {
    #[error("invalid supplier: {0}")]
    InvalidInput(&'static str),
    #[error("a supplier with this identifier already exists")]
    AlreadyExists,
    #[error("a supplier with this reference already exists")]
    ReferenceAlreadyExists,
    #[error("supplier identity context changed concurrently; retry the command")]
    ContextChanged,
    #[error("event store error: {0}")]
    Store(String),
}

/// Payload predicates are alternatives in FACTSTR; one query thus protects both
/// the supplied ID and the natural reference in the same CCC context.
fn identity_context_query(supplier_id: Uuid, reference: &str) -> EventQuery {
    EventQuery::all().with_filters([EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
        .with_payload_predicates([
            json!({ "supplier_id": supplier_id }),
            json!({ "reference": reference }),
        ])])
}

fn classify_existing(
    records: &[factstr::EventRecord],
    supplier_id: Uuid,
    reference: &str,
) -> Result<Option<RegisterSupplierError>, RegisterSupplierError> {
    for record in records {
        let fact: SupplierRegisteredFact =
            serde_json::from_value(record.payload.clone()).map_err(|error| {
                RegisterSupplierError::Store(format!(
                    "invalid {SUPPLIER_REGISTERED_V1} fact: {error}"
                ))
            })?;
        if fact.supplier_id == supplier_id {
            return Ok(Some(RegisterSupplierError::AlreadyExists));
        }
        if fact.reference == reference {
            return Ok(Some(RegisterSupplierError::ReferenceAlreadyExists));
        }
    }
    Ok(None)
}

pub fn process(
    store: &impl EventStore,
    request: RegisterSupplierRequest,
) -> Result<SupplierRegisteredFact, RegisterSupplierError> {
    let supplier_id = request.id.unwrap_or_else(Uuid::new_v4);
    let reference = request.reference.trim().to_owned();
    let name = request.name.trim().to_owned();
    if reference.is_empty() {
        return Err(RegisterSupplierError::InvalidInput(
            "supplier reference must not be blank",
        ));
    }
    if name.is_empty() {
        return Err(RegisterSupplierError::InvalidInput(
            "supplier name must not be blank",
        ));
    }

    let context_query = identity_context_query(supplier_id, &reference);
    let loaded = store
        .query(&context_query)
        .map_err(|error| RegisterSupplierError::Store(error.to_string()))?;
    if let Some(error) = classify_existing(&loaded.event_records, supplier_id, &reference)? {
        return Err(error);
    }

    let fact = SupplierRegisteredFact {
        supplier_id,
        reference: reference.clone(),
        name,
        registered_at: chrono::Utc::now(),
    };
    let event = NewEvent::new(
        SUPPLIER_REGISTERED_V1,
        to_value(&fact).map_err(|error| RegisterSupplierError::Store(error.to_string()))?,
    );

    match store.append_if(vec![event], &context_query, loaded.current_context_version) {
        Ok(_) => Ok(fact),
        Err(EventStoreError::ConditionalAppendConflict { .. }) => {
            let current = store
                .query(&context_query)
                .map_err(|error| RegisterSupplierError::Store(error.to_string()))?;
            if let Some(error) = classify_existing(&current.event_records, supplier_id, &reference)?
            {
                Err(error)
            } else {
                Err(RegisterSupplierError::ContextChanged)
            }
        }
        Err(error) => Err(RegisterSupplierError::Store(error.to_string())),
    }
}
