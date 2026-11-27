use std::collections::HashSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    amri::{
        CorrelationId, ExecutionContext, Fingerprint, IdentifierError, InteractionDefinitionId,
        InteractionId, InteractionIdentity,
    },
    amri_participants::procurement_commit::{
        PROCUREMENT_COMMIT_INTERACTION_ID, PROCUREMENT_COMMIT_INTERACTION_VERSION,
        ProcurementCommitIds, ProcurementCommitRequest,
    },
    providers::{ClockProvider, IdProvider},
};

pub const PROCUREMENT_COMMIT_FINGERPRINT_SCHEMA_VERSION: u16 = 1;
const FINGERPRINT_DOMAIN: &[u8] = b"AMRI/PROCUREMENT-COMMIT\0";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProcurementCommitExecutionError {
    #[error("captured execution timestamp predates the Unix epoch")]
    TimestampBeforeUnixEpoch,
    #[error("generated identifier is nil")]
    NilGeneratedIdentifier,
    #[error("generated interaction/business identifiers are not unique")]
    DuplicateGeneratedIdentifier,
    #[error("generated interaction identifier is invalid: {0:?}")]
    InvalidInteractionId(IdentifierError),
    #[error("interaction definition identifier is invalid: {0:?}")]
    InvalidDefinitionId(IdentifierError),
}

/// Canonical fingerprint v1: domain separator + NUL, schema version u16 BE, definition
/// version u32 BE, RFQ/supplier/quote/budget UUID bytes, quote-set version u64 BE,
/// expected amount i64 BE and a u64-length-prefixed UTF-8 currency code.
pub fn procurement_commit_request_fingerprint(request: &ProcurementCommitRequest) -> Fingerprint {
    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_DOMAIN);
    hasher.update(PROCUREMENT_COMMIT_FINGERPRINT_SCHEMA_VERSION.to_be_bytes());
    hasher.update(PROCUREMENT_COMMIT_INTERACTION_VERSION.to_be_bytes());
    hasher.update(request.selection().rfq_id().as_bytes());
    hasher.update(request.selection().supplier_id().as_bytes());
    hasher.update(request.selection().quote_id().as_bytes());
    hasher.update(request.budget_id().as_bytes());
    hasher.update(
        request
            .selection()
            .expected_quote_set_version()
            .to_be_bytes(),
    );
    hasher.update(request.expected_amount_cents().to_be_bytes());
    let currency = request.currency().as_bytes();
    hasher.update((currency.len() as u64).to_be_bytes());
    hasher.update(currency);
    Fingerprint::from_bytes(hasher.finalize().into())
}

/// Immutable execution metadata and business IDs captured once for a logical interaction.
/// Retain and reuse this value when CCC requires a fresh snapshot/re-preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcurementCommitExecution {
    context: ExecutionContext,
    ids: ProcurementCommitIds,
}

impl ProcurementCommitExecution {
    pub fn context(&self) -> &ExecutionContext {
        &self.context
    }
    pub const fn ids(&self) -> ProcurementCommitIds {
        self.ids
    }
}

pub struct ProcurementCommitExecutionContextFactory<'a> {
    clock: &'a dyn ClockProvider,
    ids: &'a dyn IdProvider,
}

impl<'a> ProcurementCommitExecutionContextFactory<'a> {
    pub fn new(clock: &'a dyn ClockProvider, ids: &'a dyn IdProvider) -> Self {
        Self { clock, ids }
    }

    pub fn create_for_request(
        &self,
        request: &ProcurementCommitRequest,
        correlation_id: Option<CorrelationId>,
    ) -> Result<ProcurementCommitExecution, ProcurementCommitExecutionError> {
        let captured_at = self.clock.now_utc().timestamp_millis();
        let captured_at_unix_ms = u64::try_from(captured_at)
            .map_err(|_| ProcurementCommitExecutionError::TimestampBeforeUnixEpoch)?;

        let interaction_uuid = self.ids.new_uuid();
        let award_id = self.ids.new_uuid();
        let reservation_id = self.ids.new_uuid();
        let purchase_order_id = self.ids.new_uuid();
        if interaction_uuid.is_nil()
            || award_id.is_nil()
            || reservation_id.is_nil()
            || purchase_order_id.is_nil()
        {
            return Err(ProcurementCommitExecutionError::NilGeneratedIdentifier);
        }
        let unique_ids = [
            interaction_uuid,
            award_id,
            reservation_id,
            purchase_order_id,
        ]
        .into_iter()
        .collect::<HashSet<_>>();
        if unique_ids.len() != 4 {
            return Err(ProcurementCommitExecutionError::DuplicateGeneratedIdentifier);
        }

        let interaction_id = InteractionId::new(interaction_uuid.to_string())
            .map_err(ProcurementCommitExecutionError::InvalidInteractionId)?;
        let definition_id = InteractionDefinitionId::new(PROCUREMENT_COMMIT_INTERACTION_ID)
            .map_err(ProcurementCommitExecutionError::InvalidDefinitionId)?;
        let definition_version = std::num::NonZeroU32::new(PROCUREMENT_COMMIT_INTERACTION_VERSION)
            .expect("interaction definition version is non-zero");
        let identity = InteractionIdentity::new(
            interaction_id,
            definition_id,
            definition_version,
            procurement_commit_request_fingerprint(request),
        );
        Ok(ProcurementCommitExecution {
            context: ExecutionContext::new(identity, captured_at_unix_ms, correlation_id),
            ids: ProcurementCommitIds {
                award_id,
                reservation_id,
                purchase_order_id,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    use crate::{
        amri::CorrelationId,
        amri_participants::{
            procurement_commit::ProcurementCommitRequest, rfq_sourcing::AwardQuoteSelection,
        },
        providers::{ClockProvider, IdProvider},
    };

    use super::{ProcurementCommitExecutionContextFactory, procurement_commit_request_fingerprint};

    struct FixedClock(AtomicUsize);
    impl ClockProvider for FixedClock {
        fn now_utc(&self) -> chrono::DateTime<Utc> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Utc.timestamp_millis_opt(1_800_000_000_000)
                .single()
                .unwrap()
        }
    }

    struct FixedIds(AtomicUsize);
    impl IdProvider for FixedIds {
        fn new_uuid(&self) -> Uuid {
            Uuid::from_u128(self.0.fetch_add(1, Ordering::SeqCst) as u128 + 1)
        }
    }

    fn request() -> ProcurementCommitRequest {
        ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                Uuid::from_u128(30),
                40,
            ),
            Uuid::from_u128(50),
            12_500,
            "BRL",
        )
    }

    #[test]
    fn factory_captures_clock_and_four_ids_once_for_reuse_across_retries() {
        let clock = FixedClock(AtomicUsize::new(0));
        let ids = FixedIds(AtomicUsize::new(0));
        let factory = ProcurementCommitExecutionContextFactory::new(&clock, &ids);
        let correlation = CorrelationId::new("trace-amri-stage5c-001").unwrap();
        let execution = factory
            .create_for_request(&request(), Some(correlation.clone()))
            .unwrap();
        let retry = execution.clone();
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
        assert_eq!(ids.0.load(Ordering::SeqCst), 4);
        assert_eq!(execution, retry);
        assert_eq!(execution.context().correlation_id(), Some(&correlation));
        assert_eq!(
            execution.context().identity().request_fingerprint(),
            procurement_commit_request_fingerprint(&request())
        );
    }

    struct DuplicateIds;
    impl IdProvider for DuplicateIds {
        fn new_uuid(&self) -> Uuid {
            Uuid::from_u128(7)
        }
    }

    #[test]
    fn factory_rejects_duplicate_generated_ids_before_exposing_execution() {
        let clock = FixedClock(AtomicUsize::new(0));
        let factory = ProcurementCommitExecutionContextFactory::new(&clock, &DuplicateIds);
        assert_eq!(
            factory.create_for_request(&request(), None).unwrap_err(),
            super::ProcurementCommitExecutionError::DuplicateGeneratedIdentifier,
        );
        assert_eq!(clock.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn fingerprint_has_a_stable_versioned_canonical_vector() {
        assert_eq!(
            procurement_commit_request_fingerprint(&request()).to_hex(),
            "f3996fadf78cd598374553facc30c7cb231dfab841305f9f8b3ee56295aefc83",
        );
    }

    #[test]
    fn fingerprint_changes_when_every_reviewed_field_changes() {
        let baseline = request();
        let expected = procurement_commit_request_fingerprint(&baseline);

        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(11),
                Uuid::from_u128(20),
                Uuid::from_u128(30),
                40,
            ),
            Uuid::from_u128(50),
            12_500,
            "BRL",
        );
        assert_ne!(expected, procurement_commit_request_fingerprint(&changed));

        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(10),
                Uuid::from_u128(21),
                Uuid::from_u128(30),
                40,
            ),
            Uuid::from_u128(50),
            12_500,
            "BRL",
        );
        assert_ne!(expected, procurement_commit_request_fingerprint(&changed));

        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                Uuid::from_u128(31),
                40,
            ),
            Uuid::from_u128(50),
            12_500,
            "BRL",
        );
        assert_ne!(expected, procurement_commit_request_fingerprint(&changed));

        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                Uuid::from_u128(30),
                41,
            ),
            Uuid::from_u128(50),
            12_500,
            "BRL",
        );
        assert_ne!(expected, procurement_commit_request_fingerprint(&changed));

        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                Uuid::from_u128(30),
                40,
            ),
            Uuid::from_u128(51),
            12_500,
            "BRL",
        );
        assert_ne!(expected, procurement_commit_request_fingerprint(&changed));

        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                Uuid::from_u128(30),
                40,
            ),
            Uuid::from_u128(50),
            12_501,
            "BRL",
        );
        assert_ne!(expected, procurement_commit_request_fingerprint(&changed));

        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                Uuid::from_u128(30),
                40,
            ),
            Uuid::from_u128(50),
            12_500,
            "USD",
        );
        assert_ne!(expected, procurement_commit_request_fingerprint(&changed));
    }

    #[test]
    fn fingerprint_changes_when_any_reviewed_input_changes() {
        let baseline = procurement_commit_request_fingerprint(&request());
        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                Uuid::from_u128(30),
                41,
            ),
            Uuid::from_u128(50),
            12_500,
            "BRL",
        );
        assert_ne!(baseline, procurement_commit_request_fingerprint(&changed));
        let changed = ProcurementCommitRequest::new(
            AwardQuoteSelection::new(
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                Uuid::from_u128(30),
                40,
            ),
            Uuid::from_u128(51),
            12_500,
            "BRL",
        );
        assert_ne!(baseline, procurement_commit_request_fingerprint(&changed));
    }
}
