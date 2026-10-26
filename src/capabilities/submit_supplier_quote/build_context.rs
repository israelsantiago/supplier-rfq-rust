use crate::events::{RfqOpenedFact, SupplierStatus};

/// Fatos já decodificados pelo shell. Este tipo não contém recursos de IO.
#[derive(Debug, Clone)]
pub(super) enum SubmissionContextFact {
    RfqOpened(RfqOpenedFact),
    RfqClosed,
    SupplierRegistered,
    SupplierEligibilityChanged(SupplierStatus),
    SupplierQuoteSubmitted,
}

#[derive(Debug, Clone)]
pub struct RfqSubmissionContext {
    pub rfq: Option<RfqOpenedFact>,
    pub closed: bool,
    pub supplier_registered: bool,
    pub supplier_status: Option<SupplierStatus>,
    pub already_submitted: bool,
}

/// Core puro: reduz os fatos tipados ao contexto explícito usado pela decisão.
pub(super) fn build_context(facts: &[SubmissionContextFact]) -> RfqSubmissionContext {
    let mut context = RfqSubmissionContext {
        rfq: None,
        closed: false,
        supplier_registered: false,
        supplier_status: None,
        already_submitted: false,
    };

    for fact in facts {
        match fact {
            SubmissionContextFact::RfqOpened(rfq) => context.rfq = Some(rfq.clone()),
            SubmissionContextFact::RfqClosed => context.closed = true,
            SubmissionContextFact::SupplierRegistered => context.supplier_registered = true,
            SubmissionContextFact::SupplierEligibilityChanged(status) => {
                context.supplier_status = Some(*status);
            }
            SubmissionContextFact::SupplierQuoteSubmitted => context.already_submitted = true,
        }
    }

    context
}

#[cfg(test)]
mod functional_core_tests {
    use super::*;
    use crate::events::RfqItemFact;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn rfq() -> RfqOpenedFact {
        RfqOpenedFact {
            rfq_id: Uuid::from_u128(1101),
            reference: "RFQ-CONTEXT-1101".to_owned(),
            currency: "BRL".to_owned(),
            opened_at: Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            items: vec![RfqItemFact {
                id: Uuid::from_u128(1102),
                sku: "SKU-1102".to_owned(),
                description: "Context builder fixture".to_owned(),
                required_quantity: 1,
            }],
        }
    }

    #[test]
    fn empty_facts_produce_the_default_context() {
        let context = build_context(&[]);
        assert!(context.rfq.is_none());
        assert!(!context.closed);
        assert!(!context.supplier_registered);
        assert!(context.supplier_status.is_none());
        assert!(!context.already_submitted);
    }

    #[test]
    fn each_context_fact_updates_its_owned_field() {
        let rfq = rfq();
        let context = build_context(&[
            SubmissionContextFact::RfqOpened(rfq.clone()),
            SubmissionContextFact::RfqClosed,
            SubmissionContextFact::SupplierRegistered,
            SubmissionContextFact::SupplierEligibilityChanged(SupplierStatus::Blocked),
            SubmissionContextFact::SupplierEligibilityChanged(SupplierStatus::Eligible),
            SubmissionContextFact::SupplierQuoteSubmitted,
        ]);
        assert_eq!(context.rfq, Some(rfq));
        assert!(context.closed);
        assert!(context.supplier_registered);
        assert_eq!(context.supplier_status, Some(SupplierStatus::Eligible));
        assert!(context.already_submitted);
        let blocked = build_context(&[SubmissionContextFact::SupplierEligibilityChanged(
            SupplierStatus::Blocked,
        )]);
        assert_eq!(blocked.supplier_status, Some(SupplierStatus::Blocked));
    }
}
