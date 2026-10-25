use factstr::{EventFilter, EventQuery, EventRecord, EventStore};
use serde_json::json;
use uuid::Uuid;

use crate::events::RFQ_OPENED_V1;

use super::process_request::OpenRfqError;

pub(super) struct LoadedRfqContext {
    pub(super) event_records: Vec<EventRecord>,
    pub(super) current_context_version: Option<u64>,
}

/// Descreve exatamente os fatos que protegem a identidade da RFQ.
pub(super) fn context_query(rfq_id: Uuid, reference: &str) -> EventQuery {
    EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([
            json!({ "rfq_id": rfq_id }),
            json!({ "reference": reference }),
        ])])
}

/// Shell de leitura: recupera fatos e a versão de contexto fornecida pelo FACTSTR.
pub(super) fn load_context(
    store: &impl EventStore,
    query: &EventQuery,
) -> Result<LoadedRfqContext, OpenRfqError> {
    let result = store
        .query(query)
        .map_err(|error| OpenRfqError::Store(error.to_string()))?;

    Ok(LoadedRfqContext {
        event_records: result.event_records,
        current_context_version: result.current_context_version,
    })
}
