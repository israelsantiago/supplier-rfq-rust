//! Query capability: read only; it has no command consistency protocol because it changes no state.
pub mod state_access;

use crate::application_state::AppState;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SupplierQuoteListing {
    pub id: Uuid,
    pub rfq_id: Uuid,
    pub supplier_id: Uuid,
    pub currency: String,
    pub total_amount_cents: i64,
    pub delivery_days: i32,
    pub valid_until: chrono::NaiveDate,
    pub submitted_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum ListSupplierQuotesError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub async fn process(
    rfq_id: Uuid,
    state: &AppState,
) -> Result<Option<Vec<SupplierQuoteListing>>, ListSupplierQuotesError> {
    Ok(state_access::list_quotes(&state.pool, rfq_id).await?)
}
