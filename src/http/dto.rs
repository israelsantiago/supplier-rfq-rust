//! DTOs da entrega HTTP.
//!
//! Estes tipos descrevem o protocolo externo. As conversões para os requests
//! internos pertencem à Delivery Mechanism, não às decisões de domínio.

use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use crate::capabilities::{
    open_rfq::OpenRfqRequest,
    register_supplier::RegisterSupplierRequest,
    set_supplier_eligibility::SetSupplierEligibilityRequest,
    submit_supplier_quote::{QuotedItemInput, SubmitSupplierQuoteRequest},
};
use crate::events::{RfqItemFact, SupplierStatus};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RfqItemBody {
    pub id: Uuid,
    pub sku: String,
    pub description: String,
    pub required_quantity: i64,
}

impl From<RfqItemBody> for RfqItemFact {
    fn from(body: RfqItemBody) -> Self {
        Self {
            id: body.id,
            sku: body.sku,
            description: body.description,
            required_quantity: body.required_quantity,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenRfqBody {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub reference: String,
    pub currency: String,
    pub items: Vec<RfqItemBody>,
}

impl From<OpenRfqBody> for OpenRfqRequest {
    fn from(body: OpenRfqBody) -> Self {
        Self {
            id: body.id,
            reference: body.reference,
            currency: body.currency,
            items: body.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterSupplierBody {
    #[serde(default)]
    pub id: Option<Uuid>,
    pub reference: String,
    pub name: String,
}

impl From<RegisterSupplierBody> for RegisterSupplierRequest {
    fn from(body: RegisterSupplierBody) -> Self {
        Self {
            id: body.id,
            reference: body.reference,
            name: body.name,
        }
    }
}

/// Representação do status aceita pela API HTTP.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplierStatusBody {
    Eligible,
    Blocked,
}

impl From<SupplierStatusBody> for SupplierStatus {
    fn from(status: SupplierStatusBody) -> Self {
        match status {
            SupplierStatusBody::Eligible => Self::Eligible,
            SupplierStatusBody::Blocked => Self::Blocked,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetSupplierEligibilityBody {
    pub status: SupplierStatusBody,
}

impl From<SetSupplierEligibilityBody> for SetSupplierEligibilityRequest {
    fn from(body: SetSupplierEligibilityBody) -> Self {
        Self {
            status: body.status.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotedItemBody {
    pub rfq_item_id: Uuid,
    pub quantity: i64,
    pub unit_price_cents: i64,
}

impl From<QuotedItemBody> for QuotedItemInput {
    fn from(body: QuotedItemBody) -> Self {
        Self {
            rfq_item_id: body.rfq_item_id,
            quantity: body.quantity,
            unit_price_cents: body.unit_price_cents,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitSupplierQuoteBody {
    pub supplier_id: Uuid,
    pub currency: String,
    pub delivery_days: i32,
    pub valid_until: NaiveDate,
    pub items: Vec<QuotedItemBody>,
}

impl From<SubmitSupplierQuoteBody> for SubmitSupplierQuoteRequest {
    fn from(body: SubmitSupplierQuoteBody) -> Self {
        Self {
            supplier_id: body.supplier_id,
            currency: body.currency,
            delivery_days: body.delivery_days,
            valid_until: body.valid_until,
            items: body.items.into_iter().map(Into::into).collect(),
        }
    }
}

/// DTO de query string, parte do contrato HTTP.
#[derive(Debug, Deserialize)]
pub struct RfqSummaryListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[cfg(test)]
mod dto_boundary_tests {
    use super::*;
    use chrono::NaiveDate;

    use crate::capabilities::{
        open_rfq::OpenRfqRequest,
        register_supplier::RegisterSupplierRequest,
        set_supplier_eligibility::SetSupplierEligibilityRequest,
        submit_supplier_quote::{QuotedItemInput, SubmitSupplierQuoteRequest},
    };
    use crate::events::{RfqItemFact, SupplierStatus};

    #[test]
    fn open_rfq_http_body_maps_all_item_fields() {
        let rfq_item_id = Uuid::from_u128(1);
        let body = OpenRfqBody {
            id: Some(Uuid::from_u128(2)),
            reference: "RFQ-HTTP-1".to_owned(),
            currency: "BRL".to_owned(),
            items: vec![RfqItemBody {
                id: rfq_item_id,
                sku: "SKU-1".to_owned(),
                description: "Amostra".to_owned(),
                required_quantity: 3,
            }],
        };

        let request: OpenRfqRequest = body.into();
        assert_eq!(request.id, Some(Uuid::from_u128(2)));
        assert_eq!(request.reference, "RFQ-HTTP-1");
        assert_eq!(request.currency, "BRL");
        assert_eq!(
            request.items,
            vec![RfqItemFact {
                id: rfq_item_id,
                sku: "SKU-1".to_owned(),
                description: "Amostra".to_owned(),
                required_quantity: 3,
            }]
        );
    }

    #[test]
    fn register_supplier_http_body_maps_identity_fields() {
        let request: RegisterSupplierRequest = RegisterSupplierBody {
            id: Some(Uuid::from_u128(3)),
            reference: "SUP-HTTP-1".to_owned(),
            name: "Fornecedor".to_owned(),
        }
        .into();

        assert_eq!(request.id, Some(Uuid::from_u128(3)));
        assert_eq!(request.reference, "SUP-HTTP-1");
        assert_eq!(request.name, "Fornecedor");
    }

    #[test]
    fn eligibility_http_status_maps_to_the_internal_status() {
        let eligible: SetSupplierEligibilityRequest = SetSupplierEligibilityBody {
            status: SupplierStatusBody::Eligible,
        }
        .into();
        let blocked: SetSupplierEligibilityRequest = SetSupplierEligibilityBody {
            status: SupplierStatusBody::Blocked,
        }
        .into();

        assert!(matches!(eligible.status, SupplierStatus::Eligible));
        assert!(matches!(blocked.status, SupplierStatus::Blocked));
    }

    #[test]
    fn submit_quote_http_body_maps_quote_and_item_fields() {
        let rfq_item_id = Uuid::from_u128(4);
        let request: SubmitSupplierQuoteRequest = SubmitSupplierQuoteBody {
            supplier_id: Uuid::from_u128(5),
            currency: "BRL".to_owned(),
            delivery_days: 12,
            valid_until: NaiveDate::from_ymd_opt(2099, 12, 31).expect("valid test date"),
            items: vec![QuotedItemBody {
                rfq_item_id,
                quantity: 3,
                unit_price_cents: 1250,
            }],
        }
        .into();

        assert_eq!(request.supplier_id, Uuid::from_u128(5));
        assert_eq!(request.currency, "BRL");
        assert_eq!(request.delivery_days, 12);
        assert_eq!(
            request.valid_until,
            NaiveDate::from_ymd_opt(2099, 12, 31).expect("valid test date")
        );
        assert_eq!(request.items.len(), 1);
        let item: &QuotedItemInput = &request.items[0];
        assert_eq!(item.rfq_item_id, rfq_item_id);
        assert_eq!(item.quantity, 3);
        assert_eq!(item.unit_price_cents, 1250);
    }

    #[test]
    fn http_dto_rejects_unknown_fields_for_command_bodies() {
        let result = serde_json::from_value::<RegisterSupplierBody>(serde_json::json!({
            "reference": "SUP-HTTP-2",
            "name": "Fornecedor",
            "unexpected": true
        }));
        assert!(result.is_err());
    }
}
