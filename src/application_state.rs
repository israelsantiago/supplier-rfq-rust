//! Porta de acesso ao Application State autoritativo do domínio.
//!
//! A interface não expõe tipos FACTSTR/SQLx. O adaptador é responsável por traduzir
//! consultas, decodificar fatos e executar append condicional (CCC).
use crate::events::{
    RfqClosedFact, RfqOpenedFact, SupplierEligibilityChangedFact, SupplierQuoteSubmittedFact,
    SupplierRegisteredFact,
};
use thiserror::Error;
use uuid::Uuid;

pub mod factstr;

/// Contextos nomeados pelo domínio, não pela tecnologia de persistência.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateQuery {
    RfqIdentity {
        rfq_id: Uuid,
        reference: String,
    },
    SupplierIdentity {
        supplier_id: Uuid,
        reference: String,
    },
    SupplierRegistration {
        supplier_id: Uuid,
    },
    RfqLifecycle {
        rfq_id: Uuid,
    },
    SupplierQuoteSubmission {
        rfq_id: Uuid,
        supplier_id: Uuid,
    },
    SupplierQuotes {
        rfq_id: Uuid,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationFact {
    RfqOpened(RfqOpenedFact),
    RfqClosed(RfqClosedFact),
    SupplierRegistered(SupplierRegisteredFact),
    SupplierEligibilityChanged(SupplierEligibilityChangedFact),
    SupplierQuoteSubmitted(SupplierQuoteSubmittedFact),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFact {
    pub sequence_number: u64,
    pub fact: ApplicationFact,
}

impl StoredFact {
    pub fn new(sequence_number: u64, fact: ApplicationFact) -> Self {
        Self {
            sequence_number,
            fact,
        }
    }
}

/// Token opaco que associa a decisão à versão do contexto observado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextVersion(Option<String>);

impl ContextVersion {
    pub fn from_opaque(token: impl Into<String>) -> Self {
        Self(Some(token.into()))
    }
    pub fn as_opaque(&self) -> Option<&str> {
        self.0.as_deref()
    }
    pub fn is_present(&self) -> bool {
        self.0.is_some()
    }
    pub(crate) fn from_factstr(version: Option<u64>) -> Self {
        Self(version.map(|value| format!("factstr:{value}")))
    }
    pub(crate) fn to_factstr(&self) -> Result<Option<u64>, StateWriteError> {
        let Some(token) = self.0.as_deref() else {
            return Ok(None);
        };
        token
            .strip_prefix("factstr:")
            .ok_or_else(|| {
                StateWriteError::Store("context token belongs to another provider".to_owned())
            })?
            .parse::<u64>()
            .map(Some)
            .map_err(|error| {
                StateWriteError::Store(format!("invalid FACTSTR context token: {error}"))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSnapshot {
    pub facts: Vec<StoredFact>,
    pub version: ContextVersion,
}

#[derive(Debug, Error)]
pub enum StateReadError {
    #[error("application state read failed: {0}")]
    Store(String),
    #[error("unsupported fact type in selected context: {0}")]
    UnsupportedFactType(String),
    #[error("invalid stored fact {fact_type}: {message}")]
    InvalidFact { fact_type: String, message: String },
}

#[derive(Debug, Error)]
pub enum StateWriteError {
    #[error("application state context changed before commit")]
    ContextChanged,
    #[error("application state write failed: {0}")]
    Store(String),
    #[error("application fact could not be encoded: {0}")]
    Encoding(String),
}

/// Read + conditional append port. It contains no business decisions.
pub trait ApplicationStateProvider: Send + Sync {
    fn read(&self, query: &StateQuery) -> Result<StateSnapshot, StateReadError>;
    fn append_if(
        &self,
        context: &StateQuery,
        expected_version: &ContextVersion,
        consequences: Vec<ApplicationFact>,
    ) -> Result<(), StateWriteError>;
}
