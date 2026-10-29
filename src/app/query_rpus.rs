//! Fachada de RPUs de consulta. A entrega HTTP não recebe o ASP nem a projeção diretamente.
use std::sync::Arc;

use crate::{
    application_state::ApplicationStateProvider,
    capabilities::{
        get_rfq_summary::{self, GetRfqSummaryError, GetRfqSummaryRequest},
        list_rfq_summaries::{self, ListRfqSummariesError, ListRfqSummariesRequest},
        list_supplier_quotes::{
            self, ListSupplierQuotesError, ListSupplierQuotesRequest, QuoteView,
        },
    },
    projections::rfq_summary_reader::{RfqSummary, RfqSummaryPage, RfqSummaryReader},
};

pub struct QueryRpus {
    state: Arc<dyn ApplicationStateProvider>,
    summary_reader: Arc<dyn RfqSummaryReader>,
}

impl QueryRpus {
    pub fn new(
        state: Arc<dyn ApplicationStateProvider>,
        summary_reader: Arc<dyn RfqSummaryReader>,
    ) -> Self {
        Self {
            state,
            summary_reader,
        }
    }

    pub fn list_supplier_quotes(
        &self,
        request: ListSupplierQuotesRequest,
    ) -> Result<Option<Vec<QuoteView>>, ListSupplierQuotesError> {
        list_supplier_quotes::process(self.state.as_ref(), request)
    }
    pub async fn get_rfq_summary(
        &self,
        request: GetRfqSummaryRequest,
    ) -> Result<RfqSummary, GetRfqSummaryError> {
        get_rfq_summary::process(self.summary_reader.as_ref(), request).await
    }
    pub async fn list_rfq_summaries(
        &self,
        request: ListRfqSummariesRequest,
    ) -> Result<RfqSummaryPage, ListRfqSummariesError> {
        list_rfq_summaries::process(self.summary_reader.as_ref(), request).await
    }
}
