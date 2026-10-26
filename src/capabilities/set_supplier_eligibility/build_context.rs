use uuid::Uuid;

#[derive(Debug, Clone)]
pub(super) enum EligibilityContextFact {
    Registered(Uuid),
}

#[derive(Debug, Clone, Default)]
pub(super) struct SupplierEligibilityContext {
    pub(super) supplier_registered: bool,
}

/// Core puro: a atribuição só depende da existência do fornecedor.
pub(super) fn build_context(
    facts: &[EligibilityContextFact],
    supplier_id: Uuid,
) -> SupplierEligibilityContext {
    let supplier_registered = facts
        .iter()
        .any(|fact| matches!(fact, EligibilityContextFact::Registered(id) if *id == supplier_id));

    SupplierEligibilityContext {
        supplier_registered,
    }
}

#[cfg(test)]
mod functional_core_tests {
    use super::*;

    #[test]
    fn registration_fact_for_requested_supplier_is_required() {
        let target = Uuid::from_u128(1001);
        let other = Uuid::from_u128(1002);
        assert!(!build_context(&[], target).supplier_registered);
        assert!(
            !build_context(&[EligibilityContextFact::Registered(other)], target)
                .supplier_registered
        );
        assert!(
            build_context(&[EligibilityContextFact::Registered(target)], target)
                .supplier_registered
        );
    }
}
