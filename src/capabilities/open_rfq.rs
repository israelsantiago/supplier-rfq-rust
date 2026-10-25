//! Domain Capability / RPU: OpenRfq.
//! The RFQ definition is an immutable fact; reference uniqueness is protected by CCC.

use factstr::{EventFilter, EventQuery, EventStore, EventStoreError, NewEvent};
use serde::Deserialize;
use serde_json::{json, to_value};
use thiserror::Error;
use uuid::Uuid;

use crate::events::{RFQ_OPENED_V1, RfqItemFact, RfqOpenedFact};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenRfqRequest {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub reference: String,
    pub currency: String,
    pub items: Vec<RfqItemFact>,
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

/// The query contains both uniqueness dimensions. FACTSTR combines matching
/// filters into one context version, so two opens with the same ID OR reference
/// cannot both append successfully against an empty/stale context.
fn rfq_identity_query(rfq_id: Uuid, reference: &str) -> EventQuery {
    EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
        .with_payload_predicates([
            json!({ "rfq_id": rfq_id }),
            json!({ "reference": reference }),
        ])])
}

pub fn process(
    store: &impl EventStore,
    request: OpenRfqRequest,
) -> Result<RfqOpenedFact, OpenRfqError> {
    let rfq_id = request.id.unwrap_or_else(Uuid::new_v4);
    let reference = request.reference.trim().to_owned();
    let currency = request.currency.trim().to_ascii_uppercase();

    if reference.is_empty() {
        return Err(OpenRfqError::InvalidInput("reference must not be blank"));
    }
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(OpenRfqError::InvalidInput(
            "currency must be a three-letter ISO-style code",
        ));
    }
    if request.items.is_empty() {
        return Err(OpenRfqError::InvalidInput(
            "an RFQ must contain at least one item",
        ));
    }

    let mut item_ids = std::collections::HashSet::with_capacity(request.items.len());
    let mut skus = std::collections::HashSet::with_capacity(request.items.len());
    for item in &request.items {
        if !item_ids.insert(item.id) {
            return Err(OpenRfqError::InvalidInput(
                "RFQ item identifiers must be unique",
            ));
        }
        if item.sku.trim().is_empty() || item.description.trim().is_empty() {
            return Err(OpenRfqError::InvalidInput(
                "item SKU and description must not be blank",
            ));
        }
        if item.required_quantity <= 0 {
            return Err(OpenRfqError::InvalidInput(
                "required item quantity must be positive",
            ));
        }
        if !skus.insert(item.sku.trim().to_ascii_uppercase()) {
            return Err(OpenRfqError::InvalidInput("RFQ item SKUs must be unique"));
        }
    }

    let context_query = rfq_identity_query(rfq_id, &reference);
    let loaded = store
        .query(&context_query)
        .map_err(|error| OpenRfqError::Store(error.to_string()))?;
    if let Some(error) = classify_records(&loaded.event_records, rfq_id, &reference)? {
        return Err(error);
    }

    let fact = RfqOpenedFact {
        rfq_id,
        reference: reference.clone(),
        currency,
        opened_at: chrono::Utc::now(),
        items: request.items,
    };
    let payload = to_value(&fact).map_err(|error| OpenRfqError::Store(error.to_string()))?;
    let event = NewEvent::new(RFQ_OPENED_V1, payload);

    match store.append_if(vec![event], &context_query, loaded.current_context_version) {
        Ok(_) => Ok(fact),
        Err(EventStoreError::ConditionalAppendConflict { .. }) => {
            let current = store
                .query(&context_query)
                .map_err(|error| OpenRfqError::Store(error.to_string()))?;
            if let Some(error) = classify_records(&current.event_records, rfq_id, &reference)? {
                Err(error)
            } else {
                Err(OpenRfqError::ContextChanged)
            }
        }
        Err(error) => Err(OpenRfqError::Store(error.to_string())),
    }
}

fn classify_records(
    records: &[factstr::EventRecord],
    rfq_id: Uuid,
    reference: &str,
) -> Result<Option<OpenRfqError>, OpenRfqError> {
    for record in records {
        let fact: RfqOpenedFact =
            serde_json::from_value(record.payload.clone()).map_err(|error| {
                OpenRfqError::Store(format!("invalid {RFQ_OPENED_V1} fact: {error}"))
            })?;
        if fact.rfq_id == rfq_id {
            return Ok(Some(OpenRfqError::AlreadyExists));
        }
        if fact.reference == reference {
            return Ok(Some(OpenRfqError::ReferenceAlreadyExists));
        }
    }
    Ok(None)
}
