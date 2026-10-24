use std::{env, error::Error, sync::Arc};

use factstr_postgres::PostgresStore;
use supplier_rfq_rpu_rust::{
    app, application_state::AppState, projections::rfq_summary::RfqSummaryProjection,
};
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let database_url =
        env::var("DATABASE_URL").map_err(|_| std::io::Error::other("DATABASE_URL must be set"))?;
    let bind_address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());

    info!("connecting to FACTSTR/PostgreSQL Event Store");
    let url_for_store = database_url.clone();
    let store = Arc::new(
        tokio::task::spawn_blocking(move || PostgresStore::connect(&url_for_store)).await??,
    );
    info!("starting durable RFQ summary projection; startup replays unapplied facts");
    let projection =
        Arc::new(RfqSummaryProjection::start(&database_url, Arc::clone(&store)).await?);
    let app = app::router(AppState::from_shared_store(store, projection));
    let listener = TcpListener::bind(&bind_address).await?;
    info!(address = %listener.local_addr()?, "Supplier RFQ API listening");

    if let Err(error) = axum::serve(listener, app).await {
        error!(%error, "HTTP server stopped");
        return Err(error.into());
    }
    Ok(())
}
