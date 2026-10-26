use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::events::SupplierRegisteredFact;

use super::{
    build_context::RegisterSupplierContext,
    process_request::{RegisterSupplierError, RegisterSupplierRequest},
};

#[derive(Debug, Clone)]
pub(super) struct PreparedRegistration {
    pub(super) supplier_id: Uuid,
    pub(super) reference: String,
    pub(super) name: String,
}

pub(super) fn prepare(
    request: RegisterSupplierRequest,
    supplier_id: Uuid,
) -> Result<PreparedRegistration, RegisterSupplierError> {
    let reference = request.reference.trim().to_owned();
    let name = request.name.trim().to_owned();

    if reference.is_empty() {
        return Err(RegisterSupplierError::InvalidInput(
            "supplier reference must not be blank",
        ));
    }
    if name.is_empty() {
        return Err(RegisterSupplierError::InvalidInput(
            "supplier name must not be blank",
        ));
    }

    Ok(PreparedRegistration {
        supplier_id,
        reference,
        name,
    })
}

pub(super) fn duplicate_rejection(
    context: &RegisterSupplierContext,
    supplier_id: Uuid,
    reference: &str,
) -> Option<RegisterSupplierError> {
    if context.contains_id(supplier_id) {
        return Some(RegisterSupplierError::AlreadyExists);
    }
    if context.contains_reference(reference) {
        return Some(RegisterSupplierError::ReferenceAlreadyExists);
    }
    None
}

pub(super) fn generate_consequences(
    prepared: &PreparedRegistration,
    registered_at: DateTime<Utc>,
) -> SupplierRegisteredFact {
    SupplierRegisteredFact {
        supplier_id: prepared.supplier_id,
        reference: prepared.reference.clone(),
        name: prepared.name.clone(),
        registered_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::SupplierRegisteredFact;

    #[test]
    fn prepare_trims_registration_fields() {
        let id = Uuid::from_u128(1);
        let prepared = prepare(
            RegisterSupplierRequest {
                id: Some(id),
                reference: " SUP-1 ".to_owned(),
                name: " Fornecedor 1 ".to_owned(),
            },
            id,
        )
        .expect("valid registration should be prepared");

        assert_eq!(prepared.supplier_id, id);
        assert_eq!(prepared.reference, "SUP-1");
        assert_eq!(prepared.name, "Fornecedor 1");
    }

    #[test]
    fn prepare_rejects_blank_reference_and_name() {
        let id = Uuid::from_u128(2);
        let blank_reference = prepare(
            RegisterSupplierRequest {
                id: Some(id),
                reference: "   ".to_owned(),
                name: "Fornecedor".to_owned(),
            },
            id,
        );
        assert!(matches!(
            blank_reference,
            Err(RegisterSupplierError::InvalidInput(_))
        ));

        let blank_name = prepare(
            RegisterSupplierRequest {
                id: Some(id),
                reference: "SUP-2".to_owned(),
                name: "   ".to_owned(),
            },
            id,
        );
        assert!(matches!(
            blank_name,
            Err(RegisterSupplierError::InvalidInput(_))
        ));
    }

    #[test]
    fn duplicate_rejection_distinguishes_id_from_reference() {
        let id = Uuid::from_u128(3);
        let context = super::super::build_context::build_context(vec![SupplierRegisteredFact {
            supplier_id: id,
            reference: "SUP-3".to_owned(),
            name: "Fornecedor".to_owned(),
            registered_at: chrono::Utc::now(),
        }]);

        assert!(matches!(
            duplicate_rejection(&context, id, "OTHER"),
            Some(RegisterSupplierError::AlreadyExists)
        ));
        assert!(matches!(
            duplicate_rejection(&context, Uuid::from_u128(4), "SUP-3"),
            Some(RegisterSupplierError::ReferenceAlreadyExists)
        ));
    }

    #[test]
    fn duplicate_rejection_returns_none_for_a_new_identity() {
        let context = RegisterSupplierContext::default();
        assert!(duplicate_rejection(&context, Uuid::from_u128(700), "SUP-NEW").is_none());
    }
    #[test]
    fn generate_consequences_uses_prepared_identity_and_supplied_timestamp() {
        let id = Uuid::from_u128(930);
        let prepared = prepare(
            RegisterSupplierRequest {
                id: Some(id),
                reference: " SUP-930 ".to_owned(),
                name: " Fornecedor 930 ".to_owned(),
            },
            id,
        )
        .expect("valid supplier registration should be prepared");
        let registered_at = chrono::DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
            .expect("fixture timestamp must be valid")
            .with_timezone(&chrono::Utc);

        let fact = generate_consequences(&prepared, registered_at);
        assert_eq!(fact.supplier_id, id);
        assert_eq!(fact.reference, "SUP-930");
        assert_eq!(fact.name, "Fornecedor 930");
        assert_eq!(fact.registered_at, registered_at);
    }
}
