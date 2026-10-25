use chrono::Utc;
use factstr::EventStore;
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

use crate::events::RfqOpenedFact;

use super::{append_consequences, build_context, generate_consequences, load_context};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenRfqRequest {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub reference: String,
    pub currency: String,
    pub items: Vec<crate::events::RfqItemFact>,
}

#[derive(Debug, Error)]
pub enum OpenRfqError {
    #[error("invalid RFQ: {0}")]
    InvalidInput(&'static str),
    #[error("an RFQ with this identifier already exists")]
    AlreadyExists,
    #[error("an RFQ with this reference already exists")]
    ReferenceAlreadyExists,
    #[error("RFQ identity context changed concurrently; retry the command")]
    ContextChanged,
    #[error("event store error: {0}")]
    Store(String),
}

/// Shell do RPU: prepara o comando, carrega fatos, decide e tenta gravar
/// as consequências somente se o contexto de identidade continuar inalterado.
pub fn process(
    store: &impl EventStore,
    request: OpenRfqRequest,
) -> Result<RfqOpenedFact, OpenRfqError> {
    // Nondeterministic values are supplied to the pure decision rather than read there.
    let assigned_id = request.id.unwrap_or_else(Uuid::new_v4);
    let prepared = generate_consequences::prepare(request, assigned_id)?;
    let query = load_context::context_query(prepared.rfq_id, &prepared.reference);

    let loaded = load_context::load_context(store, &query)?;
    let context = build_context::build_context(&loaded.event_records)?;
    let fact = generate_consequences::generate_consequences(&prepared, &context, Utc::now())?;

    match append_consequences::append_consequences(
        store,
        &query,
        loaded.current_context_version,
        &fact,
    ) {
        Ok(()) => Ok(fact),
        Err(append_consequences::AppendConsequencesError::Store(message)) => {
            Err(OpenRfqError::Store(message))
        }
        Err(append_consequences::AppendConsequencesError::ContextChanged) => {
            // Re-read the same context after a CCC conflict to return a specific duplicate
            // rejection when the concurrent fact establishes one; otherwise report a retryable conflict.
            let current = load_context::load_context(store, &query)?;
            let current_context = build_context::build_context(&current.event_records)?;
            match generate_consequences::duplicate_rejection(
                &current_context,
                prepared.rfq_id,
                &prepared.reference,
            ) {
                Some(rejection) => Err(rejection),
                None => Err(OpenRfqError::ContextChanged),
            }
        }
    }
}
