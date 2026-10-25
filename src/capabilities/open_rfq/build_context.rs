use factstr::EventRecord;

use crate::events::{RFQ_OPENED_V1, RfqOpenedFact};

use super::process_request::OpenRfqError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExistingRfqIdentity {
    pub(super) rfq_id: uuid::Uuid,
    pub(super) reference: String,
}

/// Converte os fatos carregados no contexto mínimo relevante à decisão.
pub(super) fn build_context(
    records: &[EventRecord],
) -> Result<Vec<ExistingRfqIdentity>, OpenRfqError> {
    records
        .iter()
        .filter(|record| record.event_type == RFQ_OPENED_V1)
        .map(|record| {
            let fact: RfqOpenedFact =
                serde_json::from_value(record.payload.clone()).map_err(|error| {
                    OpenRfqError::Store(format!("invalid {RFQ_OPENED_V1} fact: {error}"))
                })?;
            Ok(ExistingRfqIdentity {
                rfq_id: fact.rfq_id,
                reference: fact.reference,
            })
        })
        .collect()
}
