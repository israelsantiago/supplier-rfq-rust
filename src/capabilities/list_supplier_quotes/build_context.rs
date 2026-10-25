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
