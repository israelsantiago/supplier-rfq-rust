//! Close RFQ is a command capability and has its own RPU.
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::application_state::AppState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseRfqRejection {
    RfqNotFound,
    RfqNotOpen,
    RfqAlreadyClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseRfqOutcome {
    Closed {
        rfq_id: Uuid,
        closed_at: chrono::DateTime<Utc>,
    },
    Rejected(CloseRfqRejection),
}

#[derive(Debug, thiserror::Error)]
pub enum CloseRfqError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid stored RFQ status: {0}")]
    InvalidStoredStatus(String),
}

/// RPU: UPDATE lock on the RFQ is mutually exclusive with SubmitSupplierQuote's FOR SHARE lock.
pub async fn process(rfq_id: Uuid, state: &AppState) -> Result<CloseRfqOutcome, CloseRfqError> {
    let mut tx = state.pool.begin().await?;
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM rfqs WHERE id = $1 FOR UPDATE")
            .bind(rfq_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(status) = status else {
        tx.rollback().await?;
        return Ok(CloseRfqOutcome::Rejected(CloseRfqRejection::RfqNotFound));
    };
    match status.as_str() {
        "draft" => {
            tx.rollback().await?;
            return Ok(CloseRfqOutcome::Rejected(CloseRfqRejection::RfqNotOpen));
        }
        "closed" => {
            tx.rollback().await?;
            return Ok(CloseRfqOutcome::Rejected(
                CloseRfqRejection::RfqAlreadyClosed,
            ));
        }
        "open" => {}
        _ => {
            tx.rollback().await?;
            return Err(CloseRfqError::InvalidStoredStatus(status));
        }
    }

    let closed_at = Utc::now();
    sqlx::query("UPDATE rfqs SET status = 'closed', closed_at = $2 WHERE id = $1")
        .bind(rfq_id)
        .bind(closed_at)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(CloseRfqOutcome::Closed { rfq_id, closed_at })
}
