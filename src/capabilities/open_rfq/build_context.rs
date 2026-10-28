use crate::application_state::{ApplicationFact, StoredFact};

use super::process_request::OpenRfqError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExistingRfqIdentity {
    pub(super) rfq_id: uuid::Uuid,
    pub(super) reference: String,
}

/// Converte fatos tipados no contexto mínimo de unicidade, sem dependência de storage.
pub(super) fn build_context(
    records: &[StoredFact],
) -> Result<Vec<ExistingRfqIdentity>, OpenRfqError> {
    Ok(records
        .iter()
        .filter_map(|record| match &record.fact {
            ApplicationFact::RfqOpened(fact) => Some(ExistingRfqIdentity {
                rfq_id: fact.rfq_id,
                reference: fact.reference.clone(),
            }),
            _ => None,
        })
        .collect())
}

#[cfg(test)]
mod functional_core_tests {
    use super::*;
    use crate::{
        application_state::ApplicationFact,
        events::{RfqItemFact, RfqOpenedFact, SupplierEligibilityChangedFact, SupplierStatus},
    };
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn opened_fact() -> RfqOpenedFact {
        RfqOpenedFact {
            rfq_id: Uuid::from_u128(801),
            reference: "RFQ-CTX-801".to_owned(),
            currency: "BRL".to_owned(),
            opened_at: Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            items: vec![RfqItemFact {
                id: Uuid::from_u128(802),
                sku: "SKU-802".to_owned(),
                description: "Context builder fixture".to_owned(),
                required_quantity: 1,
            }],
        }
    }

    #[test]
    fn build_context_extracts_open_rfq_and_ignores_other_fact_variants() {
        let opened = opened_fact();
        let unrelated = SupplierEligibilityChangedFact {
            supplier_id: Uuid::from_u128(803),
            status: SupplierStatus::Blocked,
            changed_at: opened.opened_at,
        };
        let context = build_context(&[
            StoredFact::new(1, ApplicationFact::SupplierEligibilityChanged(unrelated)),
            StoredFact::new(2, ApplicationFact::RfqOpened(opened)),
        ])
        .unwrap();
        assert_eq!(context.len(), 1);
        assert_eq!(context[0].rfq_id, Uuid::from_u128(801));
        assert_eq!(context[0].reference, "RFQ-CTX-801");
        assert!(build_context(&[]).unwrap().is_empty());
    }
}
