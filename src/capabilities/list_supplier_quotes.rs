//! Query Capability: list quotes by folding quote facts from the append-only log.

use factstr::{EventFilter, EventQuery, EventStore, EventStoreError};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::facts::{
    RFQ_OPENED_V1, RfqOpenedFact, SUPPLIER_QUOTE_SUBMITTED_V1, SupplierQuoteSubmittedFact,
};

#[derive(Debug, Clone, Serialize)]
pub struct QuoteView {
    pub sequence_number: u64,
    #[serde(flatten)]
    pub quote: SupplierQuoteSubmittedFact,
}

#[derive(Debug, thiserror::Error)]
#[error("event store error: {0}")]
pub struct ListSupplierQuotesError(pub String);

pub fn process(
    store: &impl EventStore,
    rfq_id: Uuid,
) -> Result<Option<Vec<QuoteView>>, ListSupplierQuotesError> {
    let query = EventQuery::all().with_filters([
        EventFilter::for_event_types([RFQ_OPENED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
        EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1])
            .with_payload_predicates([json!({ "rfq_id": rfq_id })]),
    ]);
    let result = store
        .query(&query)
        .map_err(|error| ListSupplierQuotesError(error.to_string()))?;
    let mut exists = false;
    let mut quotes = Vec::new();

    for record in result.event_records {
        match record.event_type.as_str() {
            RFQ_OPENED_V1 => {
                let _fact: RfqOpenedFact =
                    serde_json::from_value(record.payload).map_err(|error| {
                        ListSupplierQuotesError(format!("invalid RFQ opened fact: {error}"))
                    })?;
                exists = true;
            }
            SUPPLIER_QUOTE_SUBMITTED_V1 => {
                let quote: SupplierQuoteSubmittedFact = serde_json::from_value(record.payload)
                    .map_err(|error| {
                        ListSupplierQuotesError(format!("invalid quote fact: {error}"))
                    })?;
                quotes.push(QuoteView {
                    sequence_number: record.sequence_number,
                    quote,
                });
            }
            _ => {}
        }
    }

    if exists { Ok(Some(quotes)) } else { Ok(None) }
}

impl From<EventStoreError> for ListSupplierQuotesError {
    fn from(error: EventStoreError) -> Self {
        Self(error.to_string())
    }
}
