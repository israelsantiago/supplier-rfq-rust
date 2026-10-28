//! RPU: lê o contexto por meio do ASP, decide com valores explícitos e tenta append condicional.
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

#[cfg(feature = "test-support")]
use crate::application_state::ContextVersion;
#[cfg(any(test, feature = "test-support"))]
use crate::providers::{SystemClockProvider, UuidV4Provider};
use crate::{
    application_state::{ApplicationStateProvider, StateWriteError},
    events::SupplierQuoteSubmittedFact,
    providers::{ClockProvider, IdProvider},
};

use super::{
    append_consequences,
    build_context::build_context,
    decision::{SubmissionRejection, decide},
    load_context::{LoadedSubmissionContext, load_context},
    request::SubmitSupplierQuoteRequest,
};

#[derive(Debug, Error)]
pub enum PrepareError {
    #[error(transparent)]
    Rejected(#[from] SubmissionRejection),
    #[error("application state error: {0}")]
    Store(String),
}

#[derive(Debug, Error)]
pub enum CommitError {
    #[error("command context changed after the decision was made")]
    ContextChanged,
    #[error("application state error: {0}")]
    Store(String),
}

#[derive(Debug)]
pub struct PreparedSubmission {
    context: LoadedSubmissionContext,
    pub quote: SupplierQuoteSubmittedFact,
}

impl PreparedSubmission {
    /// Token opaco da versão observada, exposto somente pelo seam de testes.
    #[cfg(feature = "test-support")]
    pub fn expected_context_version(&self) -> ContextVersion {
        self.context.current_context_version.clone()
    }
}

/// Prepara a cotação com ID/instante já escolhidos; esta função não lê relógio nem gera IDs.
pub fn prepare_with_values(
    state: &dyn ApplicationStateProvider,
    rfq_id: Uuid,
    request: SubmitSupplierQuoteRequest,
    quote_id: Uuid,
    submitted_at: DateTime<Utc>,
) -> Result<PreparedSubmission, PrepareError> {
    let loaded = load_context(state, rfq_id, request.supplier_id)
        .map_err(|error| PrepareError::Store(error.to_string()))?;
    let context = build_context(&loaded.facts);
    let quote = decide(&request, &context, quote_id, submitted_at)?;
    Ok(PreparedSubmission {
        context: loaded,
        quote,
    })
}

/// Confirma o fato somente se a versão de contexto que embasou a decisão continuar válida.
pub fn commit_prepared_with_state(
    state: &dyn ApplicationStateProvider,
    prepared: PreparedSubmission,
) -> Result<SupplierQuoteSubmittedFact, CommitError> {
    let quote = prepared.quote;
    append_consequences::append_consequences(
        state,
        &prepared.context.query,
        &prepared.context.current_context_version,
        &quote,
    )?;
    Ok(quote)
}

/// Entrada de produção. ID e timestamp são capturados uma única vez por execução lógica.
pub(crate) fn process_with_providers(
    state: &dyn ApplicationStateProvider,
    rfq_id: Uuid,
    request: SubmitSupplierQuoteRequest,
    clock: &dyn ClockProvider,
    ids: &dyn IdProvider,
) -> Result<SupplierQuoteSubmittedFact, PrepareError> {
    let quote_id = ids.new_uuid();
    let submitted_at = clock.now_utc();
    for attempt in 0..2 {
        let prepared = prepare_with_values(state, rfq_id, request.clone(), quote_id, submitted_at)?;
        match commit_prepared_with_state(state, prepared) {
            Ok(quote) => return Ok(quote),
            Err(CommitError::ContextChanged) if attempt == 0 => continue,
            Err(CommitError::ContextChanged) => {
                return Err(PrepareError::Rejected(
                    SubmissionRejection::CommandContextChanged,
                ));
            }
            Err(CommitError::Store(message)) => return Err(PrepareError::Store(message)),
        }
    }
    Err(PrepareError::Rejected(
        SubmissionRejection::CommandContextChanged,
    ))
}

#[cfg(test)]
mod ccc_tests;

// White-box seam used only by unit/integration tests and the opt-in feature.
#[cfg(any(test, feature = "test-support"))]
pub fn prepare(
    state: &dyn ApplicationStateProvider,
    rfq_id: Uuid,
    request: SubmitSupplierQuoteRequest,
) -> Result<PreparedSubmission, PrepareError> {
    let quote_id = UuidV4Provider.new_uuid();
    let submitted_at = SystemClockProvider.now_utc();
    prepare_with_values(state, rfq_id, request, quote_id, submitted_at)
}

#[cfg(any(test, feature = "test-support"))]
pub fn commit_prepared(
    state: &dyn ApplicationStateProvider,
    prepared: PreparedSubmission,
) -> Result<SupplierQuoteSubmittedFact, CommitError> {
    commit_prepared_with_state(state, prepared)
}

impl From<StateWriteError> for CommitError {
    fn from(error: StateWriteError) -> Self {
        match error {
            StateWriteError::ContextChanged => Self::ContextChanged,
            other => Self::Store(other.to_string()),
        }
    }
}
