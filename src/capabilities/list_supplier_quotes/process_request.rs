use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::application_state::ApplicationStateProvider;

use super::{build_context, load_context};

#[derive(Debug, Clone, Copy)]
pub struct ListSupplierQuotesRequest {
    pub rfq_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuoteView {
    pub sequence_number: u64,
    #[serde(flatten)]
    pub quote: crate::events::SupplierQuoteSubmittedFact,
}

#[derive(Debug, Error)]
#[error("event store error: {0}")]
pub struct ListSupplierQuotesError(pub String);

/// Orquestra a leitura dos fatos e a construção da resposta desta query RPU.
pub fn process(
    state: &dyn ApplicationStateProvider,
    request: ListSupplierQuotesRequest,
) -> Result<Option<Vec<QuoteView>>, ListSupplierQuotesError> {
    let records = load_context::load_context(state, request.rfq_id)?;
    build_context::build_context(records)
}
