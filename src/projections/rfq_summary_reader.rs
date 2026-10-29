//! Contratos de leitura para resumos projetados de RFQ.
//!
//! Este módulo não depende do adaptador PostgreSQL/SQLx. As RPUs usam o trait;
//! implementações falsas permitem testar o processamento sem banco de dados.
use std::{future::Future, pin::Pin};

use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

pub type RfqSummaryReadFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RfqSummaryReadError>> + Send + 'a>>;

#[derive(Debug, Clone, Serialize)]
pub struct RfqSummary {
    pub rfq_id: Uuid,
    pub reference: String,
    pub currency: String,
    pub status: String,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub item_count: i32,
    pub quote_count: i64,
    pub lowest_quote_amount_cents: Option<i64>,
    pub lowest_quote_supplier_id: Option<Uuid>,
    pub last_event_sequence: i64,
    pub projected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RfqSummaryPage {
    pub items: Vec<RfqSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Error)]
#[error("RFQ summary reader failed: {message}")]
pub struct RfqSummaryReadError {
    message: String,
}

impl RfqSummaryReadError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Contrato de leitura da projeção. Não expõe SQLx nem conexão/pool.
pub trait RfqSummaryReader: Send + Sync {
    fn get(&self, rfq_id: Uuid) -> RfqSummaryReadFuture<'_, Option<RfqSummary>>;

    fn list<'a>(
        &'a self,
        status: Option<&'a str>,
        limit: i64,
        offset: i64,
    ) -> RfqSummaryReadFuture<'a, RfqSummaryPage>;
}
