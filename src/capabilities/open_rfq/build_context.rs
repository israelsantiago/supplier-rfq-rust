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

#[cfg(test)]
mod functional_core_tests {
    use super::*;
    use crate::events::RfqItemFact;
    use chrono::{TimeZone, Utc};
    use factstr::EventRecord;
    use serde_json::json;

    fn record(event_type: &str, payload: serde_json::Value, sequence_number: u64) -> EventRecord {
        EventRecord {
            sequence_number,
            occurred_at: time::OffsetDateTime::UNIX_EPOCH,
            event_type: event_type.to_owned(),
            payload,
        }
    }

    fn opened_fact() -> RfqOpenedFact {
        RfqOpenedFact {
            rfq_id: uuid::Uuid::from_u128(801),
            reference: "RFQ-CTX-801".to_owned(),
            currency: "BRL".to_owned(),
            opened_at: Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            items: vec![RfqItemFact {
                id: uuid::Uuid::from_u128(802),
                sku: "SKU-802".to_owned(),
                description: "Context builder fixture".to_owned(),
                required_quantity: 1,
            }],
        }
    }

    #[test]
    fn build_context_extracts_open_rfq_and_ignores_other_event_types() {
        let opened = opened_fact();
        let context = build_context(&[
            record("unrelated.v1", json!({"anything": true}), 1),
            record(RFQ_OPENED_V1, serde_json::to_value(opened).unwrap(), 2),
        ])
        .unwrap();
        assert_eq!(context.len(), 1);
        assert_eq!(context[0].rfq_id, uuid::Uuid::from_u128(801));
        assert_eq!(context[0].reference, "RFQ-CTX-801");
        assert!(build_context(&[]).unwrap().is_empty());
    }

    #[test]
    fn build_context_rejects_malformed_open_fact_payload() {
        let result = build_context(&[record(RFQ_OPENED_V1, json!({"rfq_id": "not-a-uuid"}), 1)]);
        assert!(matches!(result, Err(OpenRfqError::Store(_))));
    }
}
