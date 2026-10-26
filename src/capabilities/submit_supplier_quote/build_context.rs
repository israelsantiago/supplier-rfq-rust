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
