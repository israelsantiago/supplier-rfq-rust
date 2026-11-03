use chrono::{DateTime, Utc};

use crate::domain::rfq_lifecycle::{RfqLifecycleContext, RfqLifecycleRejection, decide_close_rfq};
use crate::events::RfqClosedFact;

use super::process_request::{CloseRfqError, CloseRfqRejection, CloseRfqRequest};

/// Adapter fino: delega a decisão pura compartilhada e preserva o contrato legado de erro.
pub(super) fn generate_consequences(
    request: CloseRfqRequest,
    context: &RfqLifecycleContext,
    closed_at: DateTime<Utc>,
) -> Result<RfqClosedFact, CloseRfqError> {
    decide_close_rfq(request.rfq_id, context, closed_at).map_err(|rejection| match rejection {
        RfqLifecycleRejection::RfqNotFound => {
            CloseRfqError::Rejected(CloseRfqRejection::RfqNotFound)
        }
        RfqLifecycleRejection::RfqAlreadyClosed => {
            CloseRfqError::Rejected(CloseRfqRejection::RfqAlreadyClosed)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_rfq_produces_a_close_fact() {
        let request = CloseRfqRequest {
            rfq_id: uuid::Uuid::from_u128(20),
        };
        let context = RfqLifecycleContext::new(true, false);
        let closed_at = Utc::now();

        let fact = generate_consequences(request, &context, closed_at)
            .expect("an open RFQ should be closable");
        assert_eq!(fact.rfq_id, request.rfq_id);
        assert_eq!(fact.closed_at, closed_at);
    }

    #[test]
    fn a_missing_or_already_closed_rfq_is_rejected() {
        let request = CloseRfqRequest {
            rfq_id: uuid::Uuid::from_u128(21),
        };
        assert!(matches!(
            generate_consequences(request, &RfqLifecycleContext::new(false, false), Utc::now()),
            Err(CloseRfqError::Rejected(CloseRfqRejection::RfqNotFound))
        ));

        assert!(matches!(
            generate_consequences(request, &RfqLifecycleContext::new(true, true), Utc::now()),
            Err(CloseRfqError::Rejected(CloseRfqRejection::RfqAlreadyClosed))
        ));
    }
}
