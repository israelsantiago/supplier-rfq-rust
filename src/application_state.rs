//! Application State provider: FACTSTR/PostgreSQL is the authoritative store.

use std::sync::Arc;

use factstr_postgres::PostgresStore;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<PostgresStore>,
}

impl AppState {
    pub fn new(store: PostgresStore) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    pub fn from_shared_store(store: Arc<PostgresStore>) -> Self {
        Self { store }
    }
}
