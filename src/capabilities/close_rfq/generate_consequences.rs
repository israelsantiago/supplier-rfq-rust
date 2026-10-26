use chrono::{DateTime, Utc};

use crate::events::RfqClosedFact;

use super::{
    build_context::RfqLifecycleContext,
    process_request::{CloseRfqError, CloseRfqRejection, CloseRfqRequest},
};

/// Decisão pura: verifica o ciclo de vida e constrói o fato com horário fornecido pelo shell.
pub(super) fn generate_consequences(
    request: CloseRfqRequest,
    context: &RfqLifecycleContext,
    closed_at: DateTime<Utc>,
) -> Result<RfqClosedFact, CloseRfqError> {
    if !context.opened {
        return Err(CloseRfqRejection::RfqNotFound.into());
    }
    if context.closed {
        return Err(CloseRfqRejection::RfqAlreadyClosed.into());
    }

    Ok(RfqClosedFact {
        rfq_id: request.rfq_id,
        closed_at,
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
        let context = RfqLifecycleContext {
            opened: true,
            closed: false,
        };
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
            generate_consequences(request, &RfqLifecycleContext::default(), Utc::now()),
            Err(CloseRfqError::Rejected(CloseRfqRejection::RfqNotFound))
        ));

        let closed = RfqLifecycleContext {
            opened: true,
            closed: true,
        };
        assert!(matches!(
            generate_consequences(request, &closed, Utc::now()),
            Err(CloseRfqError::Rejected(CloseRfqRejection::RfqAlreadyClosed))
        ));
    }
}
