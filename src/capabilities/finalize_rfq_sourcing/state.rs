//! Persistence port owned by the atomic sourcing capability.
//! The FACTSTR adapter is in `application_state::atomic_sourcing_factstr`.

use thiserror::Error;

use super::{FinalizeRfqSourcingRequest, SourcingContext, SourcingDecision};

/// Opaque consistency token. The capability does not interpret storage sequence numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcingContextVersion(Option<String>);

impl SourcingContextVersion {
    /// Allows alternative test providers to represent their own context token.
    pub fn from_opaque(token: impl Into<String>) -> Self {
        Self(Some(token.into()))
    }

    pub fn as_opaque(&self) -> Option<&str> {
        self.0.as_deref()
    }

    pub(crate) fn from_factstr(version: Option<u64>) -> Self {
        Self(version.map(|value| format!("factstr:{value}")))
    }

    pub(crate) fn to_factstr(&self) -> Result<Option<u64>, SourcingStateError> {
        let Some(token) = self.0.as_deref() else {
            return Ok(None);
        };
        token
            .strip_prefix("factstr:")
            .ok_or_else(|| {
                SourcingStateError::Store("context token belongs to another provider".to_owned())
            })?
            .parse::<u64>()
            .map(Some)
            .map_err(|error| {
                SourcingStateError::Store(format!("invalid FACTSTR context token: {error}"))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcingContextSnapshot {
    pub context: SourcingContext,
    /// Opaque to the decision core; adapter returns the token used for its conditional append.
    pub expected_version: SourcingContextVersion,
}

#[derive(Debug, Error)]
pub enum SourcingStateError {
    #[error("the atomic sourcing context changed before commit")]
    ContextChanged,
    #[error("atomic sourcing state operation failed: {0}")]
    Store(String),
}

pub trait AtomicSourcingStateProvider: Send + Sync {
    fn read_context(
        &self,
        request: &FinalizeRfqSourcingRequest,
    ) -> Result<SourcingContextSnapshot, SourcingStateError>;

    fn append_if(
        &self,
        request: &FinalizeRfqSourcingRequest,
        expected_version: &SourcingContextVersion,
        decision: &SourcingDecision,
    ) -> Result<(), SourcingStateError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factstr_context_version_round_trips_empty_and_present_versions() {
        assert_eq!(
            SourcingContextVersion::from_factstr(None)
                .to_factstr()
                .unwrap(),
            None
        );
        assert_eq!(
            SourcingContextVersion::from_factstr(Some(42))
                .to_factstr()
                .unwrap(),
            Some(42)
        );
    }

    #[test]
    fn factstr_adapter_rejects_a_token_from_another_provider() {
        let foreign = SourcingContextVersion::from_opaque("other-store:42");
        assert!(matches!(
            foreign.to_factstr(),
            Err(SourcingStateError::Store(_))
        ));
    }

    #[test]
    fn factstr_adapter_rejects_a_malformed_version_token() {
        let malformed = SourcingContextVersion::from_opaque("factstr:not-a-number");
        assert!(matches!(
            malformed.to_factstr(),
            Err(SourcingStateError::Store(_))
        ));
    }
}
