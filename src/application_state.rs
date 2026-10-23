use sqlx::PgPool;

/// Application State: shared infrastructure only. Business interpretation stays in each capability.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
