use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::events::{RfqItemFact, RfqOpenedFact};

use super::build_context::ExistingRfqIdentity;
use super::process_request::{OpenRfqError, OpenRfqRequest};

#[derive(Debug, Clone)]
pub(super) struct PreparedOpenRfq {
    pub(super) rfq_id: Uuid,
    pub(super) reference: String,
    pub(super) currency: String,
    pub(super) items: Vec<RfqItemFact>,
}

/// Validação e normalização puras, antes de carregar o contexto de identidade.
pub(super) fn prepare(
    request: OpenRfqRequest,
    assigned_id: Uuid,
) -> Result<PreparedOpenRfq, OpenRfqError> {
    let reference = request.reference.trim().to_owned();
    let currency = request.currency.trim().to_ascii_uppercase();

    if reference.is_empty() {
        return Err(OpenRfqError::InvalidInput("reference must not be blank"));
    }
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(OpenRfqError::InvalidInput(
            "currency must be a three-letter ISO-style code",
        ));
    }
    if request.items.is_empty() {
        return Err(OpenRfqError::InvalidInput(
            "an RFQ must contain at least one item",
        ));
    }

    let mut item_ids = std::collections::HashSet::with_capacity(request.items.len());
    let mut skus = std::collections::HashSet::with_capacity(request.items.len());
    for item in &request.items {
        if !item_ids.insert(item.id) {
            return Err(OpenRfqError::InvalidInput(
                "RFQ item identifiers must be unique",
            ));
        }
        if item.sku.trim().is_empty() || item.description.trim().is_empty() {
            return Err(OpenRfqError::InvalidInput(
                "item SKU and description must not be blank",
            ));
        }
        if item.required_quantity <= 0 {
            return Err(OpenRfqError::InvalidInput(
                "required item quantity must be positive",
            ));
        }
        if !skus.insert(item.sku.trim().to_ascii_uppercase()) {
            return Err(OpenRfqError::InvalidInput("RFQ item SKUs must be unique"));
        }
    }

    Ok(PreparedOpenRfq {
        rfq_id: assigned_id,
        reference,
        currency,
        items: request.items,
    })
}

/// Rejection logic shared by the initial decision and CCC conflict classification.
pub(super) fn duplicate_rejection(
    context: &[ExistingRfqIdentity],
    rfq_id: Uuid,
    reference: &str,
) -> Option<OpenRfqError> {
    for existing in context {
        if existing.rfq_id == rfq_id {
            return Some(OpenRfqError::AlreadyExists);
        }
        if existing.reference == reference {
            return Some(OpenRfqError::ReferenceAlreadyExists);
        }
    }
    None
}

/// Functional core: identical prepared input, context and timestamp produce the same fact.
pub(super) fn generate_consequences(
    prepared: &PreparedOpenRfq,
    context: &[ExistingRfqIdentity],
    opened_at: DateTime<Utc>,
) -> Result<RfqOpenedFact, OpenRfqError> {
    if let Some(rejection) = duplicate_rejection(context, prepared.rfq_id, &prepared.reference) {
        return Err(rejection);
    }

    Ok(RfqOpenedFact {
        rfq_id: prepared.rfq_id,
        reference: prepared.reference.clone(),
        currency: prepared.currency.clone(),
        opened_at,
        items: prepared.items.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::RfqItemFact;

    fn item(id: Uuid, sku: &str) -> RfqItemFact {
        RfqItemFact {
            id,
            sku: sku.to_owned(),
            description: format!("Item {sku}"),
            required_quantity: 1,
        }
    }

    fn request(reference: &str, currency: &str, items: Vec<RfqItemFact>) -> OpenRfqRequest {
        OpenRfqRequest {
            id: None,
            reference: reference.to_owned(),
            currency: currency.to_owned(),
            items,
        }
    }

    #[test]
    fn prepare_normalizes_reference_and_currency() {
        let id = Uuid::new_v4();
        let prepared = prepare(
            request("  RFQ-001  ", " brl ", vec![item(Uuid::new_v4(), "SKU-1")]),
            id,
        )
        .unwrap();
        assert_eq!(prepared.rfq_id, id);
        assert_eq!(prepared.reference, "RFQ-001");
        assert_eq!(prepared.currency, "BRL");
    }

    #[test]
    fn prepare_rejects_duplicate_item_ids() {
        let id = Uuid::new_v4();
        assert!(matches!(
            prepare(
                request("RFQ-001", "BRL", vec![item(id, "SKU-1"), item(id, "SKU-2")]),
                Uuid::new_v4()
            ),
            Err(OpenRfqError::InvalidInput(
                "RFQ item identifiers must be unique"
            ))
        ));
    }

    #[test]
    fn prepare_rejects_duplicate_skus_case_insensitively() {
        assert!(matches!(
            prepare(
                request(
                    "RFQ-001",
                    "BRL",
                    vec![item(Uuid::new_v4(), "sku-1"), item(Uuid::new_v4(), "SKU-1")]
                ),
                Uuid::new_v4()
            ),
            Err(OpenRfqError::InvalidInput("RFQ item SKUs must be unique"))
        ));
    }

    #[test]
    fn duplicate_rejection_distinguishes_identifier_and_reference() {
        let existing_id = Uuid::new_v4();
        let context = vec![ExistingRfqIdentity {
            rfq_id: existing_id,
            reference: "RFQ-EXISTING".to_owned(),
        }];
        assert!(matches!(
            duplicate_rejection(&context, existing_id, "RFQ-OTHER"),
            Some(OpenRfqError::AlreadyExists)
        ));
        assert!(matches!(
            duplicate_rejection(&context, Uuid::new_v4(), "RFQ-EXISTING"),
            Some(OpenRfqError::ReferenceAlreadyExists)
        ));
    }

    #[test]
    fn generated_fact_uses_the_supplied_timestamp() {
        let id = Uuid::new_v4();
        let prepared = prepare(
            request("RFQ-001", "BRL", vec![item(Uuid::new_v4(), "SKU-1")]),
            id,
        )
        .unwrap();
        let opened_at = DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
            .unwrap()
            .with_timezone(&Utc);
        let fact = generate_consequences(&prepared, &[], opened_at).unwrap();
        assert_eq!(fact.rfq_id, id);
        assert_eq!(fact.opened_at, opened_at);
        assert_eq!(fact.reference, "RFQ-001");
        assert_eq!(fact.currency, "BRL");
    }

    #[test]
    fn prepare_rejects_blank_reference_invalid_currency_empty_items_and_invalid_item_fields() {
        let id = Uuid::from_u128(90);
        let valid_item = item(Uuid::from_u128(91), "SKU-91");

        assert!(matches!(
            prepare(request("   ", "BRL", vec![valid_item.clone()]), id),
            Err(OpenRfqError::InvalidInput("reference must not be blank"))
        ));
        assert!(matches!(
            prepare(request("RFQ-90", "US", vec![valid_item.clone()]), id),
            Err(OpenRfqError::InvalidInput(
                "currency must be a three-letter ISO-style code"
            ))
        ));
        assert!(matches!(
            prepare(request("RFQ-90", "b!r", vec![valid_item.clone()]), id),
            Err(OpenRfqError::InvalidInput(
                "currency must be a three-letter ISO-style code"
            ))
        ));
        assert!(matches!(
            prepare(request("RFQ-90", "BRL", vec![]), id),
            Err(OpenRfqError::InvalidInput(
                "an RFQ must contain at least one item"
            ))
        ));
        assert!(matches!(
            prepare(
                request("RFQ-90", "BRL", vec![item(Uuid::from_u128(92), "   ")]),
                id
            ),
            Err(OpenRfqError::InvalidInput(
                "item SKU and description must not be blank"
            ))
        ));
        let mut blank_description = valid_item.clone();
        blank_description.description = "   ".to_owned();
        assert!(matches!(
            prepare(request("RFQ-90", "BRL", vec![blank_description]), id),
            Err(OpenRfqError::InvalidInput(
                "item SKU and description must not be blank"
            ))
        ));
        let mut zero_quantity = valid_item;
        zero_quantity.required_quantity = 0;
        assert!(matches!(
            prepare(request("RFQ-90", "BRL", vec![zero_quantity]), id),
            Err(OpenRfqError::InvalidInput(
                "required item quantity must be positive"
            ))
        ));
    }
    #[test]
    fn duplicate_rejection_returns_none_when_id_and_reference_are_new() {
        let context = vec![ExistingRfqIdentity {
            rfq_id: Uuid::from_u128(910),
            reference: "RFQ-EXISTING-910".to_owned(),
        }];

        assert!(duplicate_rejection(&context, Uuid::from_u128(911), "RFQ-NEW-911").is_none());
    }

    #[test]
    fn generate_consequences_rejects_duplicate_identity() {
        let rfq_id = Uuid::from_u128(920);
        let prepared = prepare(
            request(
                "RFQ-920",
                "BRL",
                vec![item(Uuid::from_u128(921), "SKU-920")],
            ),
            rfq_id,
        )
        .expect("valid RFQ should be prepared");
        let context = vec![ExistingRfqIdentity {
            rfq_id,
            reference: "RFQ-OTHER-920".to_owned(),
        }];
        let opened_at = chrono::DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
            .expect("fixture timestamp must be valid")
            .with_timezone(&chrono::Utc);

        assert!(matches!(
            generate_consequences(&prepared, &context, opened_at),
            Err(OpenRfqError::AlreadyExists)
        ));
    }
}
