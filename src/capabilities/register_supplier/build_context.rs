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
