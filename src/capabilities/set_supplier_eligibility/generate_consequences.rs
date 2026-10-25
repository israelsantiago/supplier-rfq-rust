use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::events::{SupplierEligibilityChangedFact, SupplierStatus};

use super::{
    build_context::SupplierEligibilityContext, process_request::SetSupplierEligibilityError,
};

/// Decisão pura: somente fornecedor previamente cadastrado pode mudar elegibilidade.
pub(super) fn generate_consequences(
    supplier_id: Uuid,
    status: SupplierStatus,
    changed_at: DateTime<Utc>,
    context: &SupplierEligibilityContext,
) -> Result<SupplierEligibilityChangedFact, SetSupplierEligibilityError> {
    if !context.supplier_registered {
        return Err(SetSupplierEligibilityError::SupplierNotRegistered);
    }

    Ok(SupplierEligibilityChangedFact {
        supplier_id,
        status,
        changed_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::SupplierStatus;

    #[test]
    fn registered_supplier_can_change_eligibility() {
        let context = SupplierEligibilityContext {
            supplier_registered: true,
        };
        let supplier_id = Uuid::from_u128(10);
        let changed_at = Utc::now();

        let fact =
            generate_consequences(supplier_id, SupplierStatus::Eligible, changed_at, &context)
                .expect("registered supplier should produce an eligibility fact");

        assert_eq!(fact.supplier_id, supplier_id);
        assert!(matches!(fact.status, SupplierStatus::Eligible));
        assert_eq!(fact.changed_at, changed_at);
    }

    #[test]
    fn unregistered_supplier_is_rejected_by_the_pure_decision() {
        let context = SupplierEligibilityContext::default();
        let result = generate_consequences(
            Uuid::from_u128(11),
            SupplierStatus::Blocked,
            Utc::now(),
            &context,
        );

        assert!(matches!(
            result,
            Err(SetSupplierEligibilityError::SupplierNotRegistered)
        ));
    }
}
