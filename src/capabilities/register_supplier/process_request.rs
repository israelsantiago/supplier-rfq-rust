use factstr::EventStore;
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

use crate::events::SupplierRegisteredFact;
use crate::providers::{ClockProvider, IdProvider};

use super::{append_consequences, build_context, generate_consequences, load_context};

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

/// Shell do RPU: carrega o contexto de identidade e só grava contra a versão observada.
/// Executa a capability com recursos externos explicitamente injetados na shell.
pub(crate) fn process_with_providers(
    store: &impl EventStore,
    request: RegisterSupplierRequest,
    clock: &dyn ClockProvider,
    ids: &dyn IdProvider,
) -> Result<SupplierRegisteredFact, RegisterSupplierError> {
    let assigned_id = request.id.unwrap_or_else(|| ids.new_uuid());
    let prepared = generate_consequences::prepare(request, assigned_id)?;
    let registered_at = clock.now_utc();
    let query = load_context::identity_context_query(prepared.supplier_id, &prepared.reference);
    let loaded = load_context::load_context(store, &query)?;
    let context = build_context::build_context(loaded.existing_suppliers);

    if let Some(rejection) = generate_consequences::duplicate_rejection(
        &context,
        prepared.supplier_id,
        &prepared.reference,
    ) {
        return Err(rejection);
    }

    let fact = generate_consequences::generate_consequences(&prepared, registered_at);
    match append_consequences::append_consequences(
        store,
        &query,
        loaded.current_context_version,
        &fact,
    ) {
        Ok(()) => Ok(fact),
        Err(append_consequences::AppendConsequencesError::Store(message)) => {
            Err(RegisterSupplierError::Store(message))
        }
        Err(append_consequences::AppendConsequencesError::ContextChanged) => {
            // Releitura classifica duplicidade concorrente; se não houver duplicata,
            // devolve conflito recuperável sem aceitar a decisão antiga.
            let current = load_context::load_context(store, &query)?;
            let context = build_context::build_context(current.existing_suppliers);
            match generate_consequences::duplicate_rejection(
                &context,
                prepared.supplier_id,
                &prepared.reference,
            ) {
                Some(rejection) => Err(rejection),
                None => Err(RegisterSupplierError::ContextChanged),
            }
        }
    }
}
