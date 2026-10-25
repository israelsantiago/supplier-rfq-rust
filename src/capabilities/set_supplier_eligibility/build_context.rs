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
