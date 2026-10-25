use factstr::{EventFilter, EventQuery, EventRecord, EventStore};
use serde_json::json;
use uuid::Uuid;

use crate::facts::{RFQ_OPENED_V1, SUPPLIER_QUOTE_SUBMITTED_V1};

use super::process_request::ListSupplierQuotesError;

/// Shell de leitura: consulta apenas os tipos de fato necessários à capability.
pub(super) fn load_context(
    store: &impl EventStore,
    rfq_id: Uuid,
) -> Result<Vec<EventRecord>, ListSupplierQuotesError> {
    let query = EventQuery::all().with_filters([
        EventFilter::for_event_types([RFQ_OPENED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
        EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
    ]);

    store
        .query(&query)
        .map(|result| result.event_records)
        .map_err(|error| ListSupplierQuotesError(error.to_string()))
}
