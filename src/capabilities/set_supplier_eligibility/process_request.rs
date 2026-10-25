use factstr::EventStore;
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

use crate::events::SupplierEligibilityChangedFact;

use super::{append_consequences, build_context, generate_consequences, load_context};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetSupplierEligibilityRequest {
    pub status: crate::events::SupplierStatus,
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

pub fn process(
    store: &impl EventStore,
    supplier_id: Uuid,
    request: SetSupplierEligibilityRequest,
) -> Result<SupplierEligibilityChangedFact, SetSupplierEligibilityError> {
    let query = load_context::eligibility_context_query(supplier_id);
    let loaded = load_context::load_context(store, &query)?;
    let context = build_context::build_context(&loaded.facts, supplier_id);
    let fact = generate_consequences::generate_consequences(
        supplier_id,
        request.status,
        chrono::Utc::now(),
        &context,
    )?;

    match append_consequences::append_consequences(
        store,
        &query,
        loaded.current_context_version,
        &fact,
    ) {
        Ok(()) => Ok(fact),
        Err(append_consequences::AppendConsequencesError::Store(message)) => {
            Err(SetSupplierEligibilityError::Store(message))
        }
        Err(append_consequences::AppendConsequencesError::ContextChanged) => {
            // Releitura é usada para distinguir cadastro inexistente de outra alteração concorrente.
            let current = load_context::load_context(store, &query)?;
            let context = build_context::build_context(&current.facts, supplier_id);
            if !context.supplier_registered {
                Err(SetSupplierEligibilityError::SupplierNotRegistered)
            } else {
                Err(SetSupplierEligibilityError::ContextChanged)
            }
        }
    }
}
