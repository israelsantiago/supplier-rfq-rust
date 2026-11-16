use std::num::NonZeroU32;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    amri::{
        CorrelationId, ExecutionContext, Fingerprint, IdentifierError, InteractionDefinitionId,
        InteractionId, InteractionIdentity,
    },
    amri_participants::rfq_sourcing::{
        RFQ_SOURCING_INTERACTION_DEFINITION_ID, RFQ_SOURCING_INTERACTION_DEFINITION_VERSION,
    },
    capabilities::finalize_rfq_sourcing::FinalizeRfqSourcingRequest,
    providers::{ClockProvider, IdProvider},
};

pub const RFQ_SOURCING_FINGERPRINT_SCHEMA_VERSION: u16 = 1;

const FINGERPRINT_DOMAIN: &[u8] = b"AMRI/RFQ-SOURCING/FINALIZE-REQUEST\0";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecutionContextFactoryError {
    #[error("generated interaction identifier is invalid: {0:?}")]
    InvalidInteractionId(IdentifierError),
    #[error("interaction definition identifier is invalid: {0:?}")]
    InvalidDefinitionId(IdentifierError),
    #[error("captured execution timestamp predates the Unix epoch")]
    TimestampBeforeUnixEpoch,
}

/// Computes a versioned canonical fingerprint for the RFQ sourcing request.
///
/// Canonical byte contract v1: the ASCII domain separator plus one trailing NUL byte,
/// schema version as unsigned 16-bit big-endian, definition version as unsigned 32-bit
/// big-endian, three UUIDs as their 16 network-order bytes, and quote-set sequence as
/// unsigned 64-bit big-endian. No JSON or platform-dependent serialization is involved.
pub fn finalize_sourcing_request_fingerprint(request: &FinalizeRfqSourcingRequest) -> Fingerprint {
    let mut hasher = Sha256::new();
    hasher.update(FINGERPRINT_DOMAIN);
    hasher.update(RFQ_SOURCING_FINGERPRINT_SCHEMA_VERSION.to_be_bytes());
    hasher.update(RFQ_SOURCING_INTERACTION_DEFINITION_VERSION.to_be_bytes());
    hasher.update(request.rfq_id.as_bytes());
    hasher.update(request.supplier_id.as_bytes());
    hasher.update(request.quote_id.as_bytes());
    hasher.update(request.expected_quote_set_version.to_be_bytes());
    Fingerprint::from_bytes(hasher.finalize().into())
}

/// Application-shell factory. It captures the ClockProvider and IdProvider exactly once.
///
/// The returned ExecutionContext must be retained and reused for every retry of the same
/// logical interaction. Calling this factory again creates a new InteractionId and timestamp.
pub struct RfqSourcingExecutionContextFactory<'a> {
    clock: &'a dyn ClockProvider,
    ids: &'a dyn IdProvider,
}

impl<'a> RfqSourcingExecutionContextFactory<'a> {
    pub fn new(clock: &'a dyn ClockProvider, ids: &'a dyn IdProvider) -> Self {
        Self { clock, ids }
    }

    pub fn create_for_request(
        &self,
        request: &FinalizeRfqSourcingRequest,
        correlation_id: Option<CorrelationId>,
    ) -> Result<ExecutionContext, ExecutionContextFactoryError> {
        let captured_at = self.clock.now_utc().timestamp_millis();
        let captured_at_unix_ms = u64::try_from(captured_at)
            .map_err(|_| ExecutionContextFactoryError::TimestampBeforeUnixEpoch)?;

        let interaction_id = InteractionId::new(self.ids.new_uuid().to_string())
            .map_err(ExecutionContextFactoryError::InvalidInteractionId)?;
        let definition_id = InteractionDefinitionId::new(RFQ_SOURCING_INTERACTION_DEFINITION_ID)
            .map_err(ExecutionContextFactoryError::InvalidDefinitionId)?;
        let definition_version = NonZeroU32::new(RFQ_SOURCING_INTERACTION_DEFINITION_VERSION)
            .expect("the RFQ sourcing interaction definition version is non-zero");
        let fingerprint = finalize_sourcing_request_fingerprint(request);

        let identity = InteractionIdentity::new(
            interaction_id,
            definition_id,
            definition_version,
            fingerprint,
        );

        Ok(ExecutionContext::new(
            identity,
            captured_at_unix_ms,
            correlation_id,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    use super::*;

    struct FixedClock {
        timestamp_millis: i64,
        calls: AtomicUsize,
    }

    impl FixedClock {
        fn new(timestamp_millis: i64) -> Self {
            Self {
                timestamp_millis,
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl ClockProvider for FixedClock {
        fn now_utc(&self) -> chrono::DateTime<Utc> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Utc.timestamp_millis_opt(self.timestamp_millis)
                .single()
                .expect("the fixture timestamp is valid")
        }
    }

    struct FixedIds {
        value: Uuid,
        calls: AtomicUsize,
    }

    impl FixedIds {
        fn new(value: Uuid) -> Self {
            Self {
                value,
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl IdProvider for FixedIds {
        fn new_uuid(&self) -> Uuid {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.value
        }
    }

    fn request() -> FinalizeRfqSourcingRequest {
        FinalizeRfqSourcingRequest {
            rfq_id: Uuid::from_u128(1),
            supplier_id: Uuid::from_u128(2),
            quote_id: Uuid::from_u128(3),
            expected_quote_set_version: 40,
        }
    }

    #[test]
    fn fingerprint_has_a_stable_versioned_canonical_vector() {
        let fingerprint = finalize_sourcing_request_fingerprint(&request());
        assert_eq!(
            fingerprint.to_hex(),
            "ad5a9d957931d5a2d488293a482ea190beb5b16403af76c53e5f4a4b08742f6a"
        );
    }

    #[test]
    fn fingerprint_changes_when_any_request_field_changes() {
        let baseline = finalize_sourcing_request_fingerprint(&request());

        let mut changed = request();
        changed.rfq_id = Uuid::from_u128(10);
        assert_ne!(baseline, finalize_sourcing_request_fingerprint(&changed));

        let mut changed = request();
        changed.supplier_id = Uuid::from_u128(10);
        assert_ne!(baseline, finalize_sourcing_request_fingerprint(&changed));

        let mut changed = request();
        changed.quote_id = Uuid::from_u128(10);
        assert_ne!(baseline, finalize_sourcing_request_fingerprint(&changed));

        let mut changed = request();
        changed.expected_quote_set_version += 1;
        assert_ne!(baseline, finalize_sourcing_request_fingerprint(&changed));
    }

    #[test]
    fn identical_requests_produce_identical_fingerprints() {
        assert_eq!(
            finalize_sourcing_request_fingerprint(&request()),
            finalize_sourcing_request_fingerprint(&request())
        );
    }

    #[test]
    fn factory_captures_clock_and_id_once_and_preserves_correlation() {
        let clock = FixedClock::new(1_794_830_400_000);
        let ids = FixedIds::new(Uuid::from_u128(99));
        let correlation = CorrelationId::new("trace-2026-11-16-001").unwrap();
        let factory = RfqSourcingExecutionContextFactory::new(&clock, &ids);

        let context = factory
            .create_for_request(&request(), Some(correlation.clone()))
            .unwrap();
        let retry_context = context.clone();

        assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
        assert_eq!(ids.calls.load(Ordering::SeqCst), 1);
        assert_eq!(context, retry_context);
        assert_eq!(
            context.identity().interaction_id().as_str(),
            Uuid::from_u128(99).to_string()
        );
        assert_eq!(
            context.identity().definition_id().as_str(),
            RFQ_SOURCING_INTERACTION_DEFINITION_ID
        );
        assert_eq!(context.identity().definition_version().get(), 1);
        assert_eq!(
            context.identity().request_fingerprint(),
            finalize_sourcing_request_fingerprint(&request())
        );
        assert_eq!(context.correlation_id(), Some(&correlation));
        assert_eq!(context.captured_at_unix_ms(), 1_794_830_400_000);
    }

    #[test]
    fn pre_epoch_clock_is_rejected_before_consuming_an_interaction_id() {
        let clock = FixedClock::new(-1);
        let ids = FixedIds::new(Uuid::from_u128(99));
        let factory = RfqSourcingExecutionContextFactory::new(&clock, &ids);

        assert_eq!(
            factory.create_for_request(&request(), None).unwrap_err(),
            ExecutionContextFactoryError::TimestampBeforeUnixEpoch
        );
        assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
        assert_eq!(ids.calls.load(Ordering::SeqCst), 0);
    }
}
