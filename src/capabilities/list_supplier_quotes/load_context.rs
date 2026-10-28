use uuid::Uuid;

use crate::application_state::{ApplicationStateProvider, StateQuery, StateReadError, StoredFact};

use super::process_request::ListSupplierQuotesError;

/// Carrega fatos tipados por meio da porta da aplicação; FACTSTR fica no adaptador.
pub(super) fn load_context(
    state: &dyn ApplicationStateProvider,
    rfq_id: Uuid,
) -> Result<Vec<StoredFact>, ListSupplierQuotesError> {
    state
        .read(&StateQuery::SupplierQuotes { rfq_id })
        .map(|snapshot| snapshot.facts)
        .map_err(|error: StateReadError| ListSupplierQuotesError(error.to_string()))
}
