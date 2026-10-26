use uuid::Uuid;

#[derive(Debug, Clone)]
pub(super) enum EligibilityContextFact {
    Registered(Uuid),
    EligibilityChanged,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SupplierEligibilityContext {
    pub(super) supplier_registered: bool,
}

/// Função pura: reduz os fatos tipados ao contexto relevante para a decisão.
pub(super) fn build_context(
    facts: &[EligibilityContextFact],
    supplier_id: Uuid,
) -> SupplierEligibilityContext {
    let supplier_registered = facts.iter().any(|fact| match fact {
        EligibilityContextFact::Registered(id) => *id == supplier_id,
        // Elegibility facts are still loaded into the protected CCC context, but do not
        // establish registration and therefore cannot satisfy this predicate.
        EligibilityContextFact::EligibilityChanged => false,
    });

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
            !build_context(
                &[
                    EligibilityContextFact::EligibilityChanged,
                    EligibilityContextFact::Registered(other),
                ],
                target
            )
            .supplier_registered
        );
        assert!(
            build_context(
                &[
                    EligibilityContextFact::EligibilityChanged,
                    EligibilityContextFact::Registered(target),
                ],
                target
            )
            .supplier_registered
        );
    }
}
