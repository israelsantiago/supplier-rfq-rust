use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    application_state::AppState,
    capabilities::{
        close_rfq::{self, CloseRfqOutcome, CloseRfqRejection},
        list_supplier_quotes,
        submit_supplier_quote::{
            process,
            request::SubmitSupplierQuote,
            result::{QuoteSubmitted, SubmissionRejection, SubmitSupplierQuoteOutcome},
        },
    },
};

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

fn business_error(code: &str, message: &str, status: StatusCode) -> impl IntoResponse {
    (
        status,
        Json(ApiError {
            code: code.to_owned(),
            message: message.to_owned(),
        }),
    )
}

fn quote_json(quote: QuoteSubmitted) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "quote_id": quote.id,
            "rfq_id": quote.rfq_id,
            "supplier_id": quote.supplier_id,
            "currency": quote.currency,
            "total_amount_cents": quote.total_amount_cents,
            "delivery_days": quote.delivery_days,
            "valid_until": quote.valid_until,
            "submitted_at": quote.submitted_at,
            "items": quote.items.iter().zip(&quote.line_totals_cents).map(|(item, line_total_cents)| serde_json::json!({
                "rfq_item_id": item.rfq_item_id,
                "quantity": item.quantity,
                "unit_price_cents": item.unit_price_cents,
                "line_total_cents": line_total_cents,
            })).collect::<Vec<_>>(),
        })),
    )
}

pub async fn submit_supplier_quote(
    State(state): State<AppState>,
    Path(rfq_id): Path<Uuid>,
    Json(request): Json<SubmitSupplierQuote>,
) -> impl IntoResponse {
    match process::process(rfq_id, request, &state).await {
        Ok(SubmitSupplierQuoteOutcome::Accepted(quote)) => quote_json(quote).into_response(),
        Ok(SubmitSupplierQuoteOutcome::Rejected(rejection)) => {
            submission_rejection(rejection).into_response()
        }
        Err(error) => {
            tracing::error!(error = %error, rfq_id = %rfq_id, "SubmitSupplierQuote RPU failed");
            business_error(
                "internal_error",
                "The request could not be processed.",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .into_response()
        }
    }
}

fn submission_rejection(rejection: SubmissionRejection) -> impl IntoResponse {
    business_error(
        rejection.code(),
        rejection.message(),
        rejection.http_status(),
    )
}

pub async fn close_rfq(
    State(state): State<AppState>,
    Path(rfq_id): Path<Uuid>,
) -> impl IntoResponse {
    match close_rfq::process(rfq_id, &state).await {
        Ok(CloseRfqOutcome::Closed { rfq_id, closed_at }) => (
            StatusCode::OK,
            Json(
                serde_json::json!({ "rfq_id": rfq_id, "status": "closed", "closed_at": closed_at }),
            ),
        )
            .into_response(),
        Ok(CloseRfqOutcome::Rejected(CloseRfqRejection::RfqNotFound)) => {
            business_error("rfq_not_found", "RFQ was not found.", StatusCode::NOT_FOUND)
                .into_response()
        }
        Ok(CloseRfqOutcome::Rejected(CloseRfqRejection::RfqNotOpen)) => business_error(
            "rfq_not_open",
            "Only an open RFQ can be closed.",
            StatusCode::CONFLICT,
        )
        .into_response(),
        Ok(CloseRfqOutcome::Rejected(CloseRfqRejection::RfqAlreadyClosed)) => business_error(
            "rfq_already_closed",
            "The RFQ is already closed.",
            StatusCode::CONFLICT,
        )
        .into_response(),
        Err(error) => {
            tracing::error!(error = %error, rfq_id = %rfq_id, "CloseRfq RPU failed");
            business_error(
                "internal_error",
                "The request could not be processed.",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .into_response()
        }
    }
}

pub async fn list_supplier_quotes(
    State(state): State<AppState>,
    Path(rfq_id): Path<Uuid>,
) -> impl IntoResponse {
    match list_supplier_quotes::process(rfq_id, &state).await {
        Ok(Some(quotes)) => (StatusCode::OK, Json(quotes)).into_response(),
        Ok(None) => business_error("rfq_not_found", "RFQ was not found.", StatusCode::NOT_FOUND)
            .into_response(),
        Err(error) => {
            tracing::error!(error = %error, rfq_id = %rfq_id, "ListSupplierQuotes query failed");
            business_error(
                "internal_error",
                "The request could not be processed.",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .into_response()
        }
    }
}
