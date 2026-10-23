//! Request Processing Unit (RPU): read protected context, decide, record, commit.
use chrono::Utc;
use uuid::Uuid;

use super::{
    decision::decide,
    request::SubmitSupplierQuote,
    result::{SubmissionRejection, SubmitSupplierQuoteOutcome},
    state_access::{self, StateAccessError},
};
use crate::application_state::AppState;

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    StateAccess(#[from] StateAccessError),
}

pub async fn process(
    rfq_id: Uuid,
    request: SubmitSupplierQuote,
    state: &AppState,
) -> Result<SubmitSupplierQuoteOutcome, ProcessError> {
    let mut tx = state.pool.begin().await?;
    let context =
        match state_access::load_submission_context(&mut tx, rfq_id, request.supplier_id).await {
            Ok(context) => context,
            Err(error) => {
                tx.rollback().await?;
                return Err(error.into());
            }
        };

    let quote = match decide(&request, &context, Uuid::new_v4(), Utc::now()) {
        Ok(quote) => quote,
        Err(rejection) => {
            tx.rollback().await?;
            return Ok(SubmitSupplierQuoteOutcome::Rejected(rejection));
        }
    };

    if let Err(error) = state_access::record_quote(&mut tx, &quote).await {
        if is_supplier_quote_unique_conflict(&error) {
            tx.rollback().await?;
            return Ok(SubmitSupplierQuoteOutcome::Rejected(
                SubmissionRejection::DuplicateSupplierQuote,
            ));
        }
        tx.rollback().await?;
        return Err(error.into());
    }
    tx.commit().await?;
    Ok(SubmitSupplierQuoteOutcome::Accepted(quote))
}

fn is_supplier_quote_unique_conflict(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => {
            database_error.code().as_deref() == Some("23505")
                && database_error.constraint() == Some("supplier_quotes_rfq_supplier_key")
        }
        _ => false,
    }
}
