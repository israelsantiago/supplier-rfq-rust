//! Shared PostgreSQL Testcontainers fixture for integration-test executables.
//!
//! A PostgreSQL 16 container is shared while at least one TestDatabase fixture is alive.
//! Every call creates a distinct database. The cache stores only Weak, so the last fixture's
//! Drop can signal the owner thread, await removal, and avoid process-global resource leaks.

use std::{
    sync::{Arc, OnceLock, Weak, mpsc},
    thread::{self, JoinHandle},
};

use sqlx::postgres::PgPoolOptions;
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ImageExt, runners::AsyncRunner},
};
use tokio::sync::{Semaphore, SemaphorePermit};
use uuid::Uuid;

const POSTGRES_USER: &str = "rfq_test_user";
const POSTGRES_PASSWORD: &str = "rfq_test_password";
const BOOTSTRAP_DATABASE: &str = "rfq_test_bootstrap";
const MAX_CONCURRENT_DATABASE_TESTS: usize = 4;
const FIXTURE_LABEL_KEY: &str = "io.github.israelsantiago.supplier-rfq.test-fixture";
const FIXTURE_LABEL_VALUE: &str = "shared-postgres-v1";

static SHARED_POSTGRES: OnceLock<tokio::sync::Mutex<Weak<SharedPostgres>>> = OnceLock::new();
static DATABASE_TEST_PERMITS: Semaphore = Semaphore::const_new(MAX_CONCURRENT_DATABASE_TESTS);
static CREATE_DATABASE_PERMITS: Semaphore = Semaphore::const_new(1);

struct SharedPostgresEndpoint {
    host: String,
    port: u16,
}

/// Owns the shutdown channel and joins the thread which owns ContainerAsync and its runtime.
/// The actual container is dropped/removes inside that runtime, not on an arbitrary test thread.
struct SharedPostgres {
    endpoint: SharedPostgresEndpoint,
    shutdown_sender: mpsc::Sender<()>,
    owner_thread: Option<JoinHandle<()>>,
}

impl Drop for SharedPostgres {
    fn drop(&mut self) {
        // The thread performs `container.rm().await` while its Tokio runtime is entered.
        // Joining makes normal fixture teardown deterministic; no C exit hook is required.
        let _ = self.shutdown_sender.send(());
        if let Some(handle) = self.owner_thread.take() {
            if handle.join().is_err() {
                eprintln!(
                    "ERROR: shared PostgreSQL Testcontainers owner thread panicked during cleanup"
                );
            }
        }
    }
}

async fn shared_postgres() -> Arc<SharedPostgres> {
    let cache = SHARED_POSTGRES.get_or_init(|| tokio::sync::Mutex::new(Weak::new()));
    let mut cached_owner = cache.lock().await;
    if let Some(existing) = cached_owner.upgrade() {
        return existing;
    }

    // Start and wait for the owner thread in Tokio's blocking pool; never block a test-local
    // async worker while the container is starting and publishing its random host port.
    let (endpoint, shutdown_sender, owner_thread) = tokio::task::spawn_blocking(|| {
        let (endpoint_sender, endpoint_receiver) = mpsc::sync_channel(1);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();

        let owner_thread = thread::Builder::new()
            .name("shared-testcontainers-postgres-owner".to_owned())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .expect("dedicated Testcontainers runtime must build");

                let (container, endpoint) = runtime.block_on(async move {
                    let container = Postgres::default()
                        .with_db_name(BOOTSTRAP_DATABASE)
                        .with_user(POSTGRES_USER)
                        .with_password(POSTGRES_PASSWORD)
                        .with_tag("16-alpine")
                        .with_label(FIXTURE_LABEL_KEY, FIXTURE_LABEL_VALUE)
                        .start()
                        .await
                        .expect(
                            "shared PostgreSQL Testcontainer must start; Docker must be available",
                        );
                    let host = container
                        .get_host()
                        .await
                        .expect("shared PostgreSQL container host must be available")
                        .to_string();
                    let port = container
                        .get_host_port_ipv4(5432)
                        .await
                        .expect("shared PostgreSQL container port must be available");
                    (container, SharedPostgresEndpoint { host, port })
                });

                endpoint_sender
                    .send(endpoint)
                    .expect("test fixture must receive the shared PostgreSQL endpoint");
                // The owner retains the container and runtime until the last fixture is dropped.
                let _ = shutdown_receiver.recv();
                runtime.block_on(async move {
                    container.rm().await.expect(
                        "shared PostgreSQL Testcontainer must be removed during fixture Drop",
                    );
                });
            })
            .expect("dedicated Testcontainers owner thread must start");

        let endpoint = endpoint_receiver
            .recv()
            .expect("shared PostgreSQL owner thread must publish its endpoint");
        (endpoint, shutdown_sender, owner_thread)
    })
    .await
    .expect("shared PostgreSQL startup task must complete");

    let shared = Arc::new(SharedPostgres {
        endpoint,
        shutdown_sender,
        owner_thread: Some(owner_thread),
    });
    *cached_owner = Arc::downgrade(&shared);
    shared
}

/// One isolated PostgreSQL database allocated for one test fixture.
pub struct TestDatabase {
    // This lease keeps the shared container alive for the complete fixture lifetime and is
    // declared last so Rust drops it after the remaining fields.
    database_name: String,
    pub database_url: String,
    server_address: String,
    _permit: SemaphorePermit<'static>,
    // The lease is last so the container outlives the other fixture fields during destruction.
    _shared_postgres: Arc<SharedPostgres>,
}

impl TestDatabase {
    pub fn database_name(&self) -> &str {
        &self.database_name
    }

    pub fn server_address(&self) -> &str {
        &self.server_address
    }
}

/// Creates a distinct database in the currently shared PostgreSQL container.
pub async fn create_isolated_database() -> TestDatabase {
    let permit = DATABASE_TEST_PERMITS
        .acquire()
        .await
        .expect("Testcontainers database-test semaphore must remain open");
    let shared_postgres = shared_postgres().await;
    let host = shared_postgres.endpoint.host.clone();
    let port = shared_postgres.endpoint.port;

    let create_permit = CREATE_DATABASE_PERMITS
        .acquire()
        .await
        .expect("database creation semaphore must remain open");
    let admin_url =
        format!("postgres://{POSTGRES_USER}:{POSTGRES_PASSWORD}@{host}:{port}/postgres");
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .expect("admin pool must connect to the shared PostgreSQL container");

    // UUID simple format contains only hexadecimal digits, so this generated SQL identifier is safe.
    let database_name = format!("rfq_test_{}", Uuid::new_v4().simple());
    let create_sql = format!("CREATE DATABASE \"{database_name}\"");
    sqlx::query(&create_sql)
        .execute(&admin_pool)
        .await
        .unwrap_or_else(|error| panic!("could not create isolated test database: {error}"));
    admin_pool.close().await;
    drop(create_permit);

    TestDatabase {
        database_url: format!(
            "postgres://{POSTGRES_USER}:{POSTGRES_PASSWORD}@{host}:{port}/{database_name}"
        ),
        database_name,
        server_address: format!("{host}:{port}"),
        _permit: permit,
        _shared_postgres: shared_postgres,
    }
}

#[cfg(test)]
mod tests {
    use super::create_isolated_database;

    #[tokio::test]
    async fn each_fixture_gets_a_unique_database_on_the_same_container() {
        let first = create_isolated_database().await;
        let second = create_isolated_database().await;
        assert_ne!(first.database_name(), second.database_name());
        assert_ne!(first.database_url, second.database_url);
        assert_eq!(first.server_address(), second.server_address());
    }
}
