//! RPU: query the decision context, run the pure decision, then conditionally append.

use chrono::{DateTime, Utc};
use factstr::{EventStore, NewEvent};
use serde_json::to_value;
use thiserror::Error;
use uuid::Uuid;

use crate::events::{SUPPLIER_QUOTE_SUBMITTED_V1, SupplierQuoteSubmittedFact};
use crate::providers::{ClockProvider, IdProvider, SystemClockProvider, UuidV4Provider};

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
    #[error("event store error: {0}")]
    Store(String),
}

#[derive(Debug, Error)]
pub enum CommitError {
    #[error("command context changed after the decision was made")]
    ContextChanged,
    #[error("event store error: {0}")]
    Store(String),
}

#[derive(Debug)]
pub struct PreparedSubmission {
    context: LoadedSubmissionContext,
    event: NewEvent,
    pub quote: SupplierQuoteSubmittedFact,
}

impl PreparedSubmission {
    /// Context version observed before the pure decision; exposed only for CCC tests.
    #[cfg(feature = "test-support")]
    pub fn expected_context_version(&self) -> Option<u64> {
        self.context.current_context_version
    }
}

pub fn prepare(
    store: &impl EventStore,
    rfq_id: Uuid,
    request: SubmitSupplierQuoteRequest,
) -> Result<PreparedSubmission, PrepareError> {
    let quote_id = UuidV4Provider.new_uuid();
    let submitted_at = SystemClockProvider.now_utc();
    prepare_with_values(store, rfq_id, request, quote_id, submitted_at)
}

/// Prepara a cotação usando valores já escolhidos para a execução lógica do comando.
/// Esta função não lê o relógio nem gera IDs.
pub fn prepare_with_values(
    store: &impl EventStore,
    rfq_id: Uuid,
    request: SubmitSupplierQuoteRequest,
    quote_id: Uuid,
    submitted_at: DateTime<Utc>,
) -> Result<PreparedSubmission, PrepareError> {
    let loaded = load_context(store, rfq_id, request.supplier_id)
        .map_err(|error| PrepareError::Store(error.to_string()))?;
    let context = build_context(&loaded.facts);
    let quote = decide(&request, &context, quote_id, submitted_at)?;
    let payload = to_value(&quote).map_err(|error| PrepareError::Store(error.to_string()))?;
    let event = NewEvent::new(SUPPLIER_QUOTE_SUBMITTED_V1, payload);

    Ok(PreparedSubmission {
        context: loaded,
        event,
        quote,
    })
}

/// Exposed as a separate operation to make CCC's read/decide/conditional-append
/// boundary directly testable with two independent FACTSTR/PostgreSQL stores.
pub fn commit_prepared(
    store: &impl EventStore,
    prepared: PreparedSubmission,
) -> Result<SupplierQuoteSubmittedFact, CommitError> {
    let quote = prepared.quote;
    append_consequences::append_consequences(
        store,
        &prepared.context.query,
        prepared.context.current_context_version,
        prepared.event,
    )
    .map(|()| quote)
}

pub fn process(
    store: &impl EventStore,
    rfq_id: Uuid,
    request: SubmitSupplierQuoteRequest,
) -> Result<SupplierQuoteSubmittedFact, PrepareError> {
    process_with_providers(
        store,
        rfq_id,
        request,
        &SystemClockProvider,
        &UuidV4Provider,
    )
}

/// Executa a RPU com Providers explícitos. O ID e submitted_at são capturados uma
/// única vez e permanecem estáveis durante as tentativas internas de CCC.
pub fn process_with_providers(
    store: &impl EventStore,
    rfq_id: Uuid,
    request: SubmitSupplierQuoteRequest,
    clock: &dyn ClockProvider,
    ids: &dyn IdProvider,
) -> Result<SupplierQuoteSubmittedFact, PrepareError> {
    let quote_id = ids.new_uuid();
    let submitted_at = clock.now_utc();

    // A single re-read after CCC conflict turns common races into the current
    // business rejection (duplicate or closed), without blind retry loops.
    for attempt in 0..2 {
        let prepared = prepare_with_values(store, rfq_id, request.clone(), quote_id, submitted_at)?;
        match commit_prepared(store, prepared) {
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
