use factstr::EventStore;
use thiserror::Error;
use uuid::Uuid;

use crate::events::RfqClosedFact;
use crate::providers::ClockProvider;

use super::{append_consequences, build_context, generate_consequences, load_context};

#[derive(Debug, Clone, Copy)]
pub struct CloseRfqRequest {
    pub rfq_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CloseRfqRejection {
    #[error("RFQ was not found")]
    RfqNotFound,
    #[error("RFQ is already closed")]
    RfqAlreadyClosed,
    #[error("RFQ context changed concurrently; retry the command")]
    CommandContextChanged,
}

#[derive(Debug, Error)]
pub enum CloseRfqError {
    #[error(transparent)]
    Rejected(#[from] CloseRfqRejection),
    #[error("event store error: {0}")]
    Store(String),
}

/// Executa o fechamento com o relógio explicitamente injetado na shell.
pub(crate) fn process_with_providers(
    store: &impl EventStore,
    request: CloseRfqRequest,
    clock: &dyn ClockProvider,
) -> Result<RfqClosedFact, CloseRfqError> {
    let query = load_context::rfq_lifecycle_query(request.rfq_id);

    // O timestamp representa o instante lógico da solicitação e permanece estável no retry CCC.
    let closed_at = clock.now_utc();

    // Uma releitura após o primeiro conflito converte corridas comuns em rejeições de negócio.
    for attempt in 0..2 {
        let loaded = load_context::load_context(store, &query)?;
        let context = build_context::build_context(&loaded.facts);
        let fact = generate_consequences::generate_consequences(request, &context, closed_at)?;

        match append_consequences::append_consequences(
            store,
            &query,
            loaded.current_context_version,
            &fact,
        ) {
            Ok(()) => return Ok(fact),
            Err(append_consequences::AppendConsequencesError::Store(message)) => {
                return Err(CloseRfqError::Store(message));
            }
            Err(append_consequences::AppendConsequencesError::ContextChanged) if attempt == 0 => {
                continue;
            }
            Err(append_consequences::AppendConsequencesError::ContextChanged) => {
                return Err(CloseRfqRejection::CommandContextChanged.into());
            }
        }
    }

    Err(CloseRfqRejection::CommandContextChanged.into())
}
