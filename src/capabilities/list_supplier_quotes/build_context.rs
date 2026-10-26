use factstr::EventRecord;

use crate::events::{
    RFQ_OPENED_V1, RfqOpenedFact, SUPPLIER_QUOTE_SUBMITTED_V1, SupplierQuoteSubmittedFact,
};

use super::process_request::{ListSupplierQuotesError, QuoteView};

/// Projeta os fatos carregados em uma visão de consulta, sem executar IO.
pub(super) fn build_context(
    records: Vec<EventRecord>,
) -> Result<Option<Vec<QuoteView>>, ListSupplierQuotesError> {
    let mut rfq_exists = false;
    let mut quotes = Vec::new();

    for record in records {
        match record.event_type.as_str() {
            RFQ_OPENED_V1 => {
                let _fact: RfqOpenedFact =
                    serde_json::from_value(record.payload).map_err(|error| {
                        ListSupplierQuotesError(format!("invalid RFQ opened fact: {error}"))
                    })?;
                rfq_exists = true;
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

    Ok(rfq_exists.then_some(quotes))
}

#[cfg(test)]
mod functional_core_tests {
    use super::*;
    use crate::events::{QuotedItemFact, RfqItemFact};
    use chrono::{NaiveDate, TimeZone, Utc};
    use factstr::EventRecord;
    use serde_json::json;
    use uuid::Uuid;

    fn record(event_type: &str, payload: serde_json::Value, sequence_number: u64) -> EventRecord {
        EventRecord {
            sequence_number,
            occurred_at: time::OffsetDateTime::UNIX_EPOCH,
            event_type: event_type.to_owned(),
            payload,
        }
    }

    fn rfq_payload(rfq_id: Uuid) -> serde_json::Value {
        serde_json::to_value(RfqOpenedFact {
            rfq_id,
            reference: "RFQ-QUOTE-VIEW".to_owned(),
            currency: "BRL".to_owned(),
            opened_at: Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            items: vec![RfqItemFact {
                id: Uuid::from_u128(1202),
                sku: "SKU-1202".to_owned(),
                description: "Quote view fixture".to_owned(),
                required_quantity: 1,
            }],
        })
        .unwrap()
    }

    fn quote_payload(rfq_id: Uuid) -> serde_json::Value {
        serde_json::to_value(SupplierQuoteSubmittedFact {
            quote_id: Uuid::from_u128(1203),
            rfq_id,
            supplier_id: Uuid::from_u128(1204),
            currency: "BRL".to_owned(),
            total_amount_cents: 1250,
            delivery_days: 7,
            valid_until: NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
            submitted_at: Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            items: vec![QuotedItemFact {
                rfq_item_id: Uuid::from_u128(1202),
                quantity: 1,
                unit_price_cents: 1250,
                line_total_cents: 1250,
            }],
        })
        .unwrap()
    }

    #[test]
    fn empty_facts_return_none() {
        assert!(build_context(vec![]).unwrap().is_none());
    }

    #[test]
    fn opened_rfq_without_quotes_returns_an_empty_list() {
        let id = Uuid::from_u128(1201);
        let quotes = build_context(vec![record(RFQ_OPENED_V1, rfq_payload(id), 1)])
            .unwrap()
            .expect("opened RFQ must be present");
        assert!(quotes.is_empty());
    }

    #[test]
    fn quote_without_open_fact_does_not_make_an_rfq_visible() {
        let id = Uuid::from_u128(1201);
        assert!(
            build_context(vec![record(
                SUPPLIER_QUOTE_SUBMITTED_V1,
                quote_payload(id),
                2
            )])
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn valid_facts_are_mapped_and_unknown_types_are_ignored() {
        let id = Uuid::from_u128(1201);
        let quotes = build_context(vec![
            record("unrelated.v1", json!({"ignored": true}), 1),
            record(RFQ_OPENED_V1, rfq_payload(id), 2),
            record(SUPPLIER_QUOTE_SUBMITTED_V1, quote_payload(id), 3),
        ])
        .unwrap()
        .unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].sequence_number, 3);
        assert_eq!(quotes[0].quote.rfq_id, id);
        assert_eq!(quotes[0].quote.total_amount_cents, 1250);
    }

    #[test]
    fn malformed_open_fact_returns_an_error() {
        let result = build_context(vec![record(RFQ_OPENED_V1, json!({"rfq_id": "bad"}), 1)]);
        assert!(result.is_err());
    }

    #[test]
    fn malformed_quote_fact_returns_an_error() {
        let result = build_context(vec![
            record(RFQ_OPENED_V1, rfq_payload(Uuid::from_u128(1201)), 1),
            record(SUPPLIER_QUOTE_SUBMITTED_V1, json!({"quote_id": "bad"}), 2),
        ]);
        assert!(result.is_err());
    }
}
