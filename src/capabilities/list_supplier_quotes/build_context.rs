use crate::application_state::{ApplicationFact, StoredFact};

use super::process_request::{ListSupplierQuotesError, QuoteView};

/// Projeta fatos de domínio tipados para a resposta da query, sem IO ou storage.
pub(super) fn build_context(
    records: Vec<StoredFact>,
) -> Result<Option<Vec<QuoteView>>, ListSupplierQuotesError> {
    let mut rfq_exists = false;
    let mut quotes = Vec::new();

    for record in records {
        match record.fact {
            ApplicationFact::RfqOpened(_) => rfq_exists = true,
            ApplicationFact::SupplierQuoteSubmitted(quote) => quotes.push(QuoteView {
                sequence_number: record.sequence_number,
                quote,
            }),
            _ => {}
        }
    }

    Ok(rfq_exists.then_some(quotes))
}

#[cfg(test)]
mod functional_core_tests {
    use super::*;
    use crate::{
        application_state::{ApplicationFact, StoredFact},
        events::{
            QuotedItemFact, RfqItemFact, RfqOpenedFact, SupplierQuoteSubmittedFact,
            SupplierRegisteredFact,
        },
    };
    use chrono::{NaiveDate, TimeZone, Utc};
    use uuid::Uuid;

    fn rfq_fact(rfq_id: Uuid) -> RfqOpenedFact {
        RfqOpenedFact {
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
        }
    }

    fn quote_fact(rfq_id: Uuid) -> SupplierQuoteSubmittedFact {
        SupplierQuoteSubmittedFact {
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
        }
    }

    #[test]
    fn empty_facts_return_none() {
        assert!(build_context(vec![]).unwrap().is_none());
    }

    #[test]
    fn opened_rfq_without_quotes_returns_an_empty_list() {
        let id = Uuid::from_u128(1201);
        let quotes = build_context(vec![StoredFact::new(
            1,
            ApplicationFact::RfqOpened(rfq_fact(id)),
        )])
        .unwrap()
        .expect("opened RFQ must be present");
        assert!(quotes.is_empty());
    }

    #[test]
    fn quote_without_open_fact_does_not_make_an_rfq_visible() {
        let id = Uuid::from_u128(1201);
        assert!(
            build_context(vec![StoredFact::new(
                2,
                ApplicationFact::SupplierQuoteSubmitted(quote_fact(id))
            )])
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn valid_facts_are_mapped_and_unrelated_variants_are_ignored() {
        let id = Uuid::from_u128(1201);
        let rfq = rfq_fact(id);
        let unrelated = SupplierRegisteredFact {
            supplier_id: Uuid::from_u128(1205),
            reference: "SUP-1205".to_owned(),
            name: "Supplier 1205".to_owned(),
            registered_at: rfq.opened_at,
        };
        let quotes = build_context(vec![
            StoredFact::new(1, ApplicationFact::SupplierRegistered(unrelated)),
            StoredFact::new(2, ApplicationFact::RfqOpened(rfq)),
            StoredFact::new(3, ApplicationFact::SupplierQuoteSubmitted(quote_fact(id))),
        ])
        .unwrap()
        .unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].sequence_number, 3);
        assert_eq!(quotes[0].quote.rfq_id, id);
        assert_eq!(quotes[0].quote.total_amount_cents, 1250);
    }
}
