use factstr::{EventFilter, EventQuery, EventStore};
use serde_json::json;
use uuid::Uuid;

use crate::events::{SUPPLIER_REGISTERED_V1, SupplierRegisteredFact};

use super::process_request::RegisterSupplierError;

pub(super) struct LoadedSupplierContext {
    pub(super) existing_suppliers: Vec<SupplierRegisteredFact>,
    pub(super) current_context_version: Option<u64>,
}

/// Identificador e referência natural são protegidos pelo mesmo contexto CCC.
pub(super) fn identity_context_query(supplier_id: Uuid, reference: &str) -> EventQuery {
    EventQuery::all().with_filters([EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
        .with_payload_predicates([
            json!({ "supplier_id": supplier_id }),
            json!({ "reference": reference }),
        ])])
}

pub(super) fn load_context(
    store: &impl EventStore,
    query: &EventQuery,
) -> Result<LoadedSupplierContext, RegisterSupplierError> {
    let result = store
        .query(query)
        .map_err(|error| RegisterSupplierError::Store(error.to_string()))?;
    let mut existing_suppliers = Vec::with_capacity(result.event_records.len());

    for record in result.event_records {
        if record.event_type != SUPPLIER_REGISTERED_V1 {
            continue;
        }
        let fact: SupplierRegisteredFact =
            serde_json::from_value(record.payload).map_err(|error| {
                RegisterSupplierError::Store(format!(
                    "invalid {SUPPLIER_REGISTERED_V1} fact: {error}"
                ))
            })?;
        existing_suppliers.push(fact);
    }

    Ok(LoadedSupplierContext {
        existing_suppliers,
        current_context_version: result.current_context_version,
    })
}
