use crate::domain::rfq_lifecycle::RfqLifecycleContext;

#[derive(Debug, Clone, Copy)]
pub(super) enum RfqLifecycleFact {
    Opened,
    Closed,
}

/// Reduz fatos tipados ao contexto de domínio compartilhado do ciclo de vida da RFQ.
pub(super) fn build_context(facts: &[RfqLifecycleFact]) -> RfqLifecycleContext {
    let mut opened = false;
    let mut closed = false;

    for fact in facts {
        match fact {
            RfqLifecycleFact::Opened => opened = true,
            RfqLifecycleFact::Closed => closed = true,
        }
    }

    RfqLifecycleContext::new(opened, closed)
}

#[cfg(test)]
mod functional_core_tests {
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    use crate::domain::rfq_lifecycle::{RfqLifecycleRejection, decide_close_rfq};

    use super::{RfqLifecycleFact, build_context};

    #[test]
    fn lifecycle_context_reduces_open_and_close_facts_into_decision_outcomes() {
        let rfq_id = Uuid::from_u128(22);
        let now = Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();

        let empty = build_context(&[]);
        assert_eq!(
            decide_close_rfq(rfq_id, &empty, now),
            Err(RfqLifecycleRejection::RfqNotFound)
        );

        let opened = build_context(&[RfqLifecycleFact::Opened]);
        let closed_fact = decide_close_rfq(rfq_id, &opened, now)
            .expect("an opened and not-closed RFQ must be closable");
        assert_eq!(closed_fact.rfq_id, rfq_id);
        assert_eq!(closed_fact.closed_at, now);

        let opened_and_closed =
            build_context(&[RfqLifecycleFact::Opened, RfqLifecycleFact::Closed]);
        assert_eq!(
            decide_close_rfq(rfq_id, &opened_and_closed, now),
            Err(RfqLifecycleRejection::RfqAlreadyClosed)
        );

        let closed_without_open = build_context(&[RfqLifecycleFact::Closed]);
        assert_eq!(
            decide_close_rfq(rfq_id, &closed_without_open, now),
            Err(RfqLifecycleRejection::RfqNotFound)
        );
    }
}
