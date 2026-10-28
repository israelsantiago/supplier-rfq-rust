use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use super::dto::{
    CloseRfqResponse, OpenRfqBody, OpenRfqResponse, RegisterSupplierBody, RegisterSupplierResponse,
    SetSupplierEligibilityBody, SetSupplierEligibilityResponse, SubmitSupplierQuoteBody,
    SubmitSupplierQuoteResponse, SupplierQuoteViewResponse,
};

use crate::{
    api_state::ApiState,
    capabilities::{
        close_rfq::{self, CloseRfqError, CloseRfqRejection},
        get_rfq_summary as get_rfq_summary_rpu, list_rfq_summaries as list_rfq_summaries_rpu,
        list_supplier_quotes,
        open_rfq::OpenRfqError,
        register_supplier::RegisterSupplierError,
        set_supplier_eligibility::SetSupplierEligibilityError,
        submit_supplier_quote::{PrepareError, SubmissionRejection},
    },
};

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

fn error_response(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorBody {
            code,
            message: message.into(),
        }),
    )
        .into_response()
}

fn internal_error(message: impl Into<String>) -> Response {
    error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
}

pub async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok", "api_state": "factstr_postgres" }))
}

pub async fn open_rfq(State(state): State<ApiState>, Json(request): Json<OpenRfqBody>) -> Response {
    let request = request.into();
    let commands = state.commands.clone();
    match tokio::task::spawn_blocking(move || commands.open_rfq(request)).await {
        Ok(Ok(fact)) => (StatusCode::CREATED, Json(OpenRfqResponse::from(fact))).into_response(),
        Ok(Err(error)) => match error {
            OpenRfqError::InvalidInput(message) => {
                error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_rfq", message)
            }
            OpenRfqError::AlreadyExists => error_response(
                StatusCode::CONFLICT,
                "rfq_already_exists",
                "an RFQ with this identifier already exists",
            ),
            OpenRfqError::ReferenceAlreadyExists => error_response(
                StatusCode::CONFLICT,
                "rfq_reference_already_exists",
                "an RFQ with this reference already exists",
            ),
            OpenRfqError::ContextChanged => error_response(
                StatusCode::CONFLICT,
                "command_context_changed",
                "the RFQ identity context changed concurrently; retry the command",
            ),
            OpenRfqError::Store(message) => internal_error(message),
        },
        Err(error) => internal_error(error.to_string()),
    }
}

pub async fn register_supplier(
    State(state): State<ApiState>,
    Json(request): Json<RegisterSupplierBody>,
) -> Response {
    let request = request.into();
    let commands = state.commands.clone();
    match tokio::task::spawn_blocking(move || commands.register_supplier(request)).await {
        Ok(Ok(fact)) => (
            StatusCode::CREATED,
            Json(RegisterSupplierResponse::from(fact)),
        )
            .into_response(),
        Ok(Err(error)) => match error {
            RegisterSupplierError::InvalidInput(message) => error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_supplier",
                message,
            ),
            RegisterSupplierError::AlreadyExists => error_response(
                StatusCode::CONFLICT,
                "supplier_already_exists",
                "a supplier with this identifier already exists",
            ),
            RegisterSupplierError::ReferenceAlreadyExists => error_response(
                StatusCode::CONFLICT,
                "supplier_reference_already_exists",
                "a supplier with this reference already exists",
            ),
            RegisterSupplierError::ContextChanged => error_response(
                StatusCode::CONFLICT,
                "command_context_changed",
                "supplier identity context changed concurrently; retry the command",
            ),
            RegisterSupplierError::Store(message) => internal_error(message),
        },
        Err(error) => internal_error(error.to_string()),
    }
}

pub async fn set_supplier_eligibility(
    State(state): State<ApiState>,
    Path(supplier_id): Path<Uuid>,
    Json(request): Json<SetSupplierEligibilityBody>,
) -> Response {
    let request = request.into();
    let commands = state.commands.clone();
    match tokio::task::spawn_blocking(move || {
        commands.set_supplier_eligibility(supplier_id, request)
    })
    .await
    {
        Ok(Ok(fact)) => (
            StatusCode::OK,
            Json(SetSupplierEligibilityResponse::from(fact)),
        )
            .into_response(),
        Ok(Err(error)) => match error {
            SetSupplierEligibilityError::SupplierNotRegistered => error_response(
                StatusCode::NOT_FOUND,
                "supplier_not_registered",
                "supplier must be registered before setting eligibility",
            ),
            SetSupplierEligibilityError::ContextChanged => error_response(
                StatusCode::CONFLICT,
                "command_context_changed",
                "supplier eligibility context changed concurrently; retry the command",
            ),
            SetSupplierEligibilityError::Store(message) => internal_error(message),
        },
        Err(error) => internal_error(error.to_string()),
    }
}

pub async fn submit_supplier_quote(
    State(state): State<ApiState>,
    Path(rfq_id): Path<Uuid>,
    Json(request): Json<SubmitSupplierQuoteBody>,
) -> Response {
    let request = request.into();
    let commands = state.commands.clone();
    match tokio::task::spawn_blocking(move || commands.submit_supplier_quote(rfq_id, request)).await
    {
        Ok(Ok(quote)) => (
            StatusCode::CREATED,
            Json(SubmitSupplierQuoteResponse::from(quote)),
        )
            .into_response(),
        Ok(Err(PrepareError::Rejected(rejection))) => submission_rejection(rejection),
        Ok(Err(PrepareError::Store(message))) => internal_error(message),
        Err(error) => internal_error(error.to_string()),
    }
}

fn submission_rejection(rejection: SubmissionRejection) -> Response {
    use SubmissionRejection as R;
    let (status, code) = match &rejection {
        R::RfqNotFound => (StatusCode::NOT_FOUND, "rfq_not_found"),
        R::SupplierNotRegistered => (StatusCode::NOT_FOUND, "supplier_not_registered"),
        R::SupplierEligibilityNotSet => (StatusCode::CONFLICT, "supplier_eligibility_not_set"),
        R::RfqClosed => (StatusCode::CONFLICT, "rfq_closed"),
        R::SupplierNotEligible => (StatusCode::CONFLICT, "supplier_not_eligible"),
        R::DuplicateSupplierQuote => (StatusCode::CONFLICT, "duplicate_supplier_quote"),
        R::CommandContextChanged => (StatusCode::CONFLICT, "command_context_changed"),
        R::CurrencyMismatch => (StatusCode::UNPROCESSABLE_ENTITY, "currency_mismatch"),
        R::InvalidDeliveryDays => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_delivery_days"),
        R::ValidityInPast => (StatusCode::UNPROCESSABLE_ENTITY, "validity_in_past"),
        R::EmptyQuote => (StatusCode::UNPROCESSABLE_ENTITY, "empty_quote"),
        R::DuplicateQuotedItem => (StatusCode::UNPROCESSABLE_ENTITY, "duplicate_quoted_item"),
        R::ItemSetMismatch => (StatusCode::UNPROCESSABLE_ENTITY, "item_set_mismatch"),
        R::QuantityMismatch => (StatusCode::UNPROCESSABLE_ENTITY, "quantity_mismatch"),
        R::InvalidUnitPrice => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_unit_price"),
        R::AmountOverflow => (StatusCode::UNPROCESSABLE_ENTITY, "amount_overflow"),
    };
    error_response(status, code, rejection.to_string())
}

pub async fn close_rfq(State(state): State<ApiState>, Path(rfq_id): Path<Uuid>) -> Response {
    let commands = state.commands.clone();
    match tokio::task::spawn_blocking(move || {
        commands.close_rfq(close_rfq::CloseRfqRequest { rfq_id })
    })
    .await
    {
        Ok(Ok(fact)) => (StatusCode::OK, Json(CloseRfqResponse::from(fact))).into_response(),
        Ok(Err(CloseRfqError::Rejected(rejection))) => {
            let (status, code) = match &rejection {
                CloseRfqRejection::RfqNotFound => (StatusCode::NOT_FOUND, "rfq_not_found"),
                CloseRfqRejection::RfqAlreadyClosed => (StatusCode::CONFLICT, "rfq_already_closed"),
                CloseRfqRejection::CommandContextChanged => {
                    (StatusCode::CONFLICT, "command_context_changed")
                }
            };
            error_response(status, code, rejection.to_string())
        }
        Ok(Err(CloseRfqError::Store(message))) => internal_error(message),
        Err(error) => internal_error(error.to_string()),
    }
}

pub use super::dto::RfqSummaryListQuery;

pub async fn get_rfq_summary(State(state): State<ApiState>, Path(rfq_id): Path<Uuid>) -> Response {
    let request = get_rfq_summary_rpu::GetRfqSummaryRequest { rfq_id };

    match get_rfq_summary_rpu::process(state.rfq_summary_projection.as_ref(), request).await {
        Ok(summary) => (StatusCode::OK, Json(summary)).into_response(),
        Err(get_rfq_summary_rpu::GetRfqSummaryError::NotAvailableYet) => error_response(
            StatusCode::NOT_FOUND,
            "rfq_summary_not_found",
            "RFQ summary is not available in the read projection yet; retry if the RFQ was just created",
        ),
        Err(get_rfq_summary_rpu::GetRfqSummaryError::Projection(message)) => {
            internal_error(message)
        }
    }
}

pub async fn list_rfq_summaries(
    State(state): State<ApiState>,
    Query(query): Query<RfqSummaryListQuery>,
) -> Response {
    let request = list_rfq_summaries_rpu::ListRfqSummariesRequest {
        status: query.status,
        limit: query.limit,
        offset: query.offset,
    };

    match list_rfq_summaries_rpu::process(state.rfq_summary_projection.as_ref(), request).await {
        Ok(page) => (StatusCode::OK, Json(page)).into_response(),
        Err(list_rfq_summaries_rpu::ListRfqSummariesError::InvalidLimit) => error_response(
            StatusCode::BAD_REQUEST,
            "invalid_limit",
            "limit must be between 1 and 100",
        ),
        Err(list_rfq_summaries_rpu::ListRfqSummariesError::InvalidOffset) => error_response(
            StatusCode::BAD_REQUEST,
            "invalid_offset",
            "offset must be zero or greater",
        ),
        Err(list_rfq_summaries_rpu::ListRfqSummariesError::InvalidStatus) => error_response(
            StatusCode::BAD_REQUEST,
            "invalid_status",
            "status must be either open or closed",
        ),
        Err(list_rfq_summaries_rpu::ListRfqSummariesError::Projection(message)) => {
            internal_error(message)
        }
    }
}

pub async fn list_supplier_quotes(
    State(state): State<ApiState>,
    Path(rfq_id): Path<Uuid>,
) -> Response {
    let store = state.event_store.clone();
    match tokio::task::spawn_blocking(move || {
        list_supplier_quotes::process(
            store.as_ref(),
            list_supplier_quotes::ListSupplierQuotesRequest { rfq_id },
        )
    })
    .await
    {
        Ok(Ok(Some(quotes))) => {
            let responses = quotes
                .into_iter()
                .map(SupplierQuoteViewResponse::from)
                .collect::<Vec<_>>();
            (StatusCode::OK, Json(json!({ "quotes": responses }))).into_response()
        }
        Ok(Ok(None)) => error_response(StatusCode::NOT_FOUND, "rfq_not_found", "RFQ was not found"),
        Ok(Err(error)) => internal_error(error.to_string()),
        Err(error) => internal_error(error.to_string()),
    }
}
