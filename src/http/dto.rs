//! DTOs da entrega HTTP.
//!
//! Estes tipos descrevem o protocolo externo. As conversões para os requests
//! internos pertencem à Delivery Mechanism, não às decisões de domínio.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::capabilities::list_supplier_quotes::QuoteView;
use crate::capabilities::{
    open_rfq::OpenRfqRequest,
    register_supplier::RegisterSupplierRequest,
    set_supplier_eligibility::SetSupplierEligibilityRequest,
    submit_supplier_quote::{QuotedItemInput, SubmitSupplierQuoteRequest},
};
use crate::events::{
    QuotedItemFact, RfqClosedFact, RfqItemFact, RfqOpenedFact, SupplierEligibilityChangedFact,
    SupplierQuoteSubmittedFact, SupplierRegisteredFact, SupplierStatus,
};

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

/// Outbound DTOs are API contracts, not persisted event payload types.
#[derive(Debug, Serialize)]
pub struct RfqItemResponse {
    pub id: Uuid,
    pub sku: String,
    pub description: String,
    pub required_quantity: i64,
}

impl From<RfqItemFact> for RfqItemResponse {
    fn from(item: RfqItemFact) -> Self {
        Self {
            id: item.id,
            sku: item.sku,
            description: item.description,
            required_quantity: item.required_quantity,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OpenRfqResponse {
    pub rfq_id: Uuid,
    pub reference: String,
    pub currency: String,
    pub opened_at: DateTime<Utc>,
    pub items: Vec<RfqItemResponse>,
}

impl From<RfqOpenedFact> for OpenRfqResponse {
    fn from(fact: RfqOpenedFact) -> Self {
        Self {
            rfq_id: fact.rfq_id,
            reference: fact.reference,
            currency: fact.currency,
            opened_at: fact.opened_at,
            items: fact.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RegisterSupplierResponse {
    pub supplier_id: Uuid,
    pub reference: String,
    pub name: String,
    pub registered_at: DateTime<Utc>,
}

impl From<SupplierRegisteredFact> for RegisterSupplierResponse {
    fn from(fact: SupplierRegisteredFact) -> Self {
        Self {
            supplier_id: fact.supplier_id,
            reference: fact.reference,
            name: fact.name,
            registered_at: fact.registered_at,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplierStatusResponse {
    Eligible,
    Blocked,
}

impl From<SupplierStatus> for SupplierStatusResponse {
    fn from(status: SupplierStatus) -> Self {
        match status {
            SupplierStatus::Eligible => Self::Eligible,
            SupplierStatus::Blocked => Self::Blocked,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SetSupplierEligibilityResponse {
    pub supplier_id: Uuid,
    pub status: SupplierStatusResponse,
    pub changed_at: DateTime<Utc>,
}

impl From<SupplierEligibilityChangedFact> for SetSupplierEligibilityResponse {
    fn from(fact: SupplierEligibilityChangedFact) -> Self {
        Self {
            supplier_id: fact.supplier_id,
            status: fact.status.into(),
            changed_at: fact.changed_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct QuotedItemResponse {
    pub rfq_item_id: Uuid,
    pub quantity: i64,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
}

impl From<QuotedItemFact> for QuotedItemResponse {
    fn from(item: QuotedItemFact) -> Self {
        Self {
            rfq_item_id: item.rfq_item_id,
            quantity: item.quantity,
            unit_price_cents: item.unit_price_cents,
            line_total_cents: item.line_total_cents,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SubmitSupplierQuoteResponse {
    pub quote_id: Uuid,
    pub rfq_id: Uuid,
    pub supplier_id: Uuid,
    pub currency: String,
    pub total_amount_cents: i64,
    pub delivery_days: i32,
    pub valid_until: NaiveDate,
    pub submitted_at: DateTime<Utc>,
    pub items: Vec<QuotedItemResponse>,
}

impl From<SupplierQuoteSubmittedFact> for SubmitSupplierQuoteResponse {
    fn from(fact: SupplierQuoteSubmittedFact) -> Self {
        Self {
            quote_id: fact.quote_id,
            rfq_id: fact.rfq_id,
            supplier_id: fact.supplier_id,
            currency: fact.currency,
            total_amount_cents: fact.total_amount_cents,
            delivery_days: fact.delivery_days,
            valid_until: fact.valid_until,
            submitted_at: fact.submitted_at,
            items: fact.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CloseRfqResponse {
    pub rfq_id: Uuid,
    pub closed_at: DateTime<Utc>,
}

impl From<RfqClosedFact> for CloseRfqResponse {
    fn from(fact: RfqClosedFact) -> Self {
        Self {
            rfq_id: fact.rfq_id,
            closed_at: fact.closed_at,
        }
    }
}

/// Preserves the existing flattened quote-list JSON shape while decoupling it from event facts.
#[derive(Debug, Serialize)]
pub struct SupplierQuoteViewResponse {
    pub sequence_number: u64,
    #[serde(flatten)]
    pub quote: SubmitSupplierQuoteResponse,
}

impl From<QuoteView> for SupplierQuoteViewResponse {
    fn from(view: QuoteView) -> Self {
        Self {
            sequence_number: view.sequence_number,
            quote: view.quote.into(),
        }
    }
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
    fn open_rfq_json_defaults_optional_id_and_rejects_unknown_item_fields() {
        let body: OpenRfqBody = serde_json::from_value(serde_json::json!({
            "reference": "RFQ-JSON-1",
            "currency": "BRL",
            "items": [{
                "id": "00000000-0000-0000-0000-000000000011",
                "sku": "SKU-1",
                "description": "Item",
                "required_quantity": 2
            }]
        }))
        .expect("valid HTTP body without optional id should deserialize");
        assert_eq!(body.id, None);
        assert_eq!(body.items.len(), 1);
        assert_eq!(body.items[0].required_quantity, 2);

        let result = serde_json::from_value::<OpenRfqBody>(serde_json::json!({
            "reference": "RFQ-JSON-2",
            "currency": "BRL",
            "items": [{
                "id": "00000000-0000-0000-0000-000000000012",
                "sku": "SKU-2",
                "description": "Item",
                "required_quantity": 1,
                "unexpected": true
            }]
        }));
        assert!(
            result.is_err(),
            "unknown nested item fields must be rejected"
        );
    }

    #[test]
    fn eligibility_json_accepts_only_known_snake_case_statuses() {
        let eligible = serde_json::from_value::<SetSupplierEligibilityBody>(
            serde_json::json!({"status": "eligible"}),
        )
        .expect("eligible must be a valid HTTP status");
        let blocked = serde_json::from_value::<SetSupplierEligibilityBody>(
            serde_json::json!({"status": "blocked"}),
        )
        .expect("blocked must be a valid HTTP status");
        assert!(matches!(eligible.status, SupplierStatusBody::Eligible));
        assert!(matches!(blocked.status, SupplierStatusBody::Blocked));

        let unknown = serde_json::from_value::<SetSupplierEligibilityBody>(
            serde_json::json!({"status": "pending"}),
        );
        assert!(
            unknown.is_err(),
            "unknown status must not silently map to a known state"
        );
    }

    #[test]
    fn quote_json_requires_iso_date_and_rejects_unknown_fields() {
        let valid = serde_json::from_value::<SubmitSupplierQuoteBody>(serde_json::json!({
            "supplier_id": "00000000-0000-0000-0000-000000000021",
            "currency": "BRL",
            "delivery_days": 5,
            "valid_until": "2099-12-31",
            "items": [{
                "rfq_item_id": "00000000-0000-0000-0000-000000000022",
                "quantity": 3,
                "unit_price_cents": 1250
            }]
        }))
        .expect("valid quote JSON should deserialize");
        assert_eq!(valid.delivery_days, 5);
        assert_eq!(valid.items[0].unit_price_cents, 1250);

        let invalid_date = serde_json::from_value::<SubmitSupplierQuoteBody>(serde_json::json!({
            "supplier_id": "00000000-0000-0000-0000-000000000021",
            "currency": "BRL",
            "delivery_days": 5,
            "valid_until": "31-12-2099",
            "items": []
        }));
        assert!(
            invalid_date.is_err(),
            "date must use the documented ISO date shape"
        );

        let unknown_field = serde_json::from_value::<SubmitSupplierQuoteBody>(serde_json::json!({
            "supplier_id": "00000000-0000-0000-0000-000000000021",
            "currency": "BRL",
            "delivery_days": 5,
            "valid_until": "2099-12-31",
            "items": [],
            "unexpected": true
        }));
        assert!(
            unknown_field.is_err(),
            "unknown quote fields must be rejected"
        );
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

#[cfg(test)]
mod response_dto_tests {
    use super::*;
    use crate::events::SupplierQuoteSubmittedFact;

    #[test]
    fn open_rfq_response_keeps_api_shape_without_serializing_the_event_type() {
        let rfq_id = Uuid::from_u128(9901);
        let item_id = Uuid::from_u128(9902);
        let fact = RfqOpenedFact {
            rfq_id,
            reference: "RFQ-DTO-1".to_owned(),
            currency: "BRL".to_owned(),
            opened_at: Utc::now(),
            items: vec![RfqItemFact {
                id: item_id,
                sku: "SKU-DTO".to_owned(),
                description: "API response item".to_owned(),
                required_quantity: 2,
            }],
        };
        let value = serde_json::to_value(OpenRfqResponse::from(fact)).unwrap();
        assert_eq!(value["rfq_id"], rfq_id.to_string());
        assert_eq!(value["items"][0]["id"], item_id.to_string());
        assert_eq!(value["items"][0]["required_quantity"], 2);
        assert_eq!(value["reference"], "RFQ-DTO-1");
    }

    #[test]
    fn quote_view_response_keeps_flattened_quote_json_shape() {
        let quote_id = Uuid::from_u128(9911);
        let rfq_id = Uuid::from_u128(9912);
        let supplier_id = Uuid::from_u128(9913);
        let item_id = Uuid::from_u128(9914);
        let fact = SupplierQuoteSubmittedFact {
            quote_id,
            rfq_id,
            supplier_id,
            currency: "BRL".to_owned(),
            total_amount_cents: 2500,
            delivery_days: 3,
            valid_until: NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
            submitted_at: Utc::now(),
            items: vec![QuotedItemFact {
                rfq_item_id: item_id,
                quantity: 2,
                unit_price_cents: 1250,
                line_total_cents: 2500,
            }],
        };
        let view = QuoteView {
            sequence_number: 42,
            quote: fact,
        };
        let value = serde_json::to_value(SupplierQuoteViewResponse::from(view)).unwrap();
        assert_eq!(value["sequence_number"], 42);
        assert_eq!(value["quote_id"], quote_id.to_string());
        assert_eq!(value["items"][0]["line_total_cents"], 2500);
        assert!(
            value.get("quote").is_none(),
            "existing quote fields stay flattened"
        );
    }
}
