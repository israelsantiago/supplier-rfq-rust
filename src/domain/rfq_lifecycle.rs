use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::events::RfqClosedFact;

/// Explicit lifecycle state consumed by the shared RFQ-closing decision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RfqLifecycleContext {
    opened: bool,
    closed: bool,
}

impl RfqLifecycleContext {
    pub const fn new(opened: bool, closed: bool) -> Self {
        Self { opened, closed }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RfqLifecycleRejection {
    RfqNotFound,
    RfqAlreadyClosed,
}

/// Pure close decision shared by the autonomous CloseRfq RPU and prepared participants.
pub fn decide_close_rfq(
    rfq_id: Uuid,
    context: &RfqLifecycleContext,
    closed_at: DateTime<Utc>,
) -> Result<RfqClosedFact, RfqLifecycleRejection> {
    if !context.opened {
        return Err(RfqLifecycleRejection::RfqNotFound);
    }
    if context.closed {
        return Err(RfqLifecycleRejection::RfqAlreadyClosed);
    }

    Ok(RfqClosedFact { rfq_id, closed_at })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    use super::{RfqLifecycleContext, RfqLifecycleRejection, decide_close_rfq};

    #[test]
    fn opened_and_not_closed_produces_the_expected_fact() {
        let rfq_id = Uuid::from_u128(20);
        let closed_at = Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();
        let context = RfqLifecycleContext::new(true, false);

        let fact = decide_close_rfq(rfq_id, &context, closed_at).unwrap();
        assert_eq!(fact.rfq_id, rfq_id);
        assert_eq!(fact.closed_at, closed_at);
    }

    #[test]
    fn missing_or_already_closed_rfqs_are_rejected() {
        let rfq_id = Uuid::from_u128(21);
        let now = Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();

        assert_eq!(
            decide_close_rfq(rfq_id, &RfqLifecycleContext::new(false, false), now),
            Err(RfqLifecycleRejection::RfqNotFound)
        );
        assert_eq!(
            decide_close_rfq(rfq_id, &RfqLifecycleContext::new(true, true), now),
            Err(RfqLifecycleRejection::RfqAlreadyClosed)
        );
    }
}
