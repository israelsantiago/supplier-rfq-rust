use uuid::Uuid;

use crate::events::SupplierRegisteredFact;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SupplierIdentity {
    supplier_id: Uuid,
    reference: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct RegisterSupplierContext {
    identities: Vec<SupplierIdentity>,
}

/// Construção pura do contexto mínimo necessário à regra de unicidade.
pub(super) fn build_context(facts: Vec<SupplierRegisteredFact>) -> RegisterSupplierContext {
    RegisterSupplierContext {
        identities: facts
            .into_iter()
            .map(|fact| SupplierIdentity {
                supplier_id: fact.supplier_id,
                reference: fact.reference,
            })
            .collect(),
    }
}

impl RegisterSupplierContext {
    pub(super) fn contains_id(&self, id: Uuid) -> bool {
        self.identities
            .iter()
            .any(|identity| identity.supplier_id == id)
    }

    pub(super) fn contains_reference(&self, reference: &str) -> bool {
        self.identities
            .iter()
            .any(|identity| identity.reference == reference)
    }
}

#[cfg(test)]
mod functional_core_tests {
    use super::*;
    use chrono::Utc;

    fn fact(id: Uuid, reference: &str) -> SupplierRegisteredFact {
        SupplierRegisteredFact {
            supplier_id: id,
            reference: reference.to_owned(),
            name: format!("Supplier {reference}"),
            registered_at: Utc::now(),
        }
    }

    #[test]
    fn identity_queries_cover_present_absent_and_empty_contexts() {
        let id = Uuid::from_u128(901);
        let context = build_context(vec![fact(id, "SUP-901")]);
        assert!(context.contains_id(id));
        assert!(!context.contains_id(Uuid::from_u128(902)));
        assert!(context.contains_reference("SUP-901"));
        assert!(!context.contains_reference("SUP-902"));

        let empty = build_context(vec![]);
        assert!(!empty.contains_id(id));
        assert!(!empty.contains_reference("SUP-901"));
    }
}
