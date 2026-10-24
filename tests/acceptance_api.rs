use std::{net::SocketAddr, sync::Arc};

use chrono::NaiveDate;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use supplier_rfq_rpu::{app, application_state::AppState, database};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tokio::{
    net::TcpListener,
    sync::Barrier,
    task::JoinHandle,
    time::{Duration, Instant, sleep},
};
use uuid::Uuid;

const RFQ_ID: &str = "11111111-1111-4111-8111-111111111111";
const SUPPLIER_ELIGIBLE_ID: &str = "22222222-2222-4222-8222-222222222222";
const SUPPLIER_BLOCKED_ID: &str = "33333333-3333-4333-8333-333333333333";
const ITEM_A_ID: &str = "44444444-4444-4444-8444-444444444441";
const ITEM_B_ID: &str = "44444444-4444-4444-8444-444444444442";

struct TestApp {
    pub base_url: String,
    pub pool: PgPool,
    server_task: JoinHandle<()>,
    // Declared last so the pool and server task are dropped before the container.
    _container: ContainerAsync<Postgres>,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

async fn spawn_app() -> TestApp {
    let container = Postgres::default()
        .with_db_name("supplier_rfq")
        .with_user("rfq_user")
        .with_password("rfq_password")
        .with_tag("16-alpine")
        .start()
        .await
        .expect("PostgreSQL Testcontainer must start; Docker must be available");

    let host = container
        .get_host()
        .await
        .expect("container host must be available");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped PostgreSQL port must be available");
    let database_url = format!("postgres://rfq_user:rfq_password@{host}:{port}/supplier_rfq");
    let pool = database::connect(&database_url)
        .await
        .expect("test PostgreSQL must accept connections");
    database::migrate(&pool)
        .await
        .expect("migrations must apply");
    seed_fixture(&pool).await;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral port must be available");
    let address: SocketAddr = listener
        .local_addr()
        .expect("listener address must be available");
    let api = app::router(AppState::new(pool.clone()));
    let server_task = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, api).await {
            panic!("test API server failed: {error}");
        }
    });

    TestApp {
        base_url: format!("http://{address}"),
        pool,
        server_task,
        _container: container,
    }
}

async fn seed_fixture(pool: &PgPool) {
    sqlx::query("INSERT INTO rfqs (id, reference, status, currency) VALUES ($1, 'RFQ-TEST-001', 'draft', 'BRL')")
        .bind(Uuid::parse_str(RFQ_ID).unwrap())
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO suppliers (id, reference, name, status) VALUES ($1, 'SUP-ELIGIBLE', 'Supplier Eligible', 'eligible'), ($2, 'SUP-BLOCKED', 'Supplier Blocked', 'blocked')")
        .bind(Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap())
        .bind(Uuid::parse_str(SUPPLIER_BLOCKED_ID).unwrap())
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO rfq_items (id, rfq_id, sku, description, required_quantity) VALUES ($1, $2, 'SKU-A', 'Industrial component A', 10), ($3, $2, 'SKU-B', 'Industrial component B', 5)")
        .bind(Uuid::parse_str(ITEM_A_ID).unwrap())
        .bind(Uuid::parse_str(RFQ_ID).unwrap())
        .bind(Uuid::parse_str(ITEM_B_ID).unwrap())
        .execute(pool).await.unwrap();
    sqlx::query("UPDATE rfqs SET status = 'open', opened_at = now() WHERE id = $1")
        .bind(Uuid::parse_str(RFQ_ID).unwrap())
        .execute(pool)
        .await
        .unwrap();
}

fn valid_request(supplier_id: &str) -> Value {
    json!({
        "supplier_id": supplier_id,
        "currency": "BRL",
        "delivery_days": 7,
        "valid_until": NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
        "items": [
            { "rfq_item_id": ITEM_A_ID, "quantity": 10, "unit_price_cents": 1250 },
            { "rfq_item_id": ITEM_B_ID, "quantity": 5, "unit_price_cents": 3000 }
        ]
    })
}

#[tokio::test]
async fn acceptance_submits_quote_and_query_returns_it() {
    let app = spawn_app().await;
    let client = Client::new();
    let submit = client
        .post(format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url))
        .json(&valid_request(SUPPLIER_ELIGIBLE_ID))
        .send()
        .await
        .unwrap();
    assert_eq!(submit.status(), StatusCode::CREATED);
    let body: Value = submit.json().await.unwrap();
    assert_eq!(body["total_amount_cents"], 27_500);
    assert_eq!(body["currency"], "BRL");

    let query = client
        .get(format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(query.status(), StatusCode::OK);
    let quotes: Value = query.json().await.unwrap();
    assert_eq!(quotes.as_array().unwrap().len(), 1);
    assert_eq!(quotes[0]["total_amount_cents"], 27_500);
}

#[tokio::test]
async fn acceptance_rejects_blocked_supplier_and_does_not_write_state() {
    let app = spawn_app().await;
    let client = Client::new();
    let response = client
        .post(format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url))
        .json(&valid_request(SUPPLIER_BLOCKED_ID))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["code"], "supplier_not_eligible");

    let quote_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM supplier_quotes WHERE rfq_id = $1")
            .bind(Uuid::parse_str(RFQ_ID).unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        quote_count, 0,
        "a rejected business command must not mutate Application State"
    );

    // CCC also needs protection against a phantom RFQ item: this domain freezes the item set on opening.
    let attempted_item = sqlx::query("INSERT INTO rfq_items (id, rfq_id, sku, description, required_quantity) VALUES ($1, $2, 'LATE-ITEM', 'Must not be added after opening', 1)")
        .bind(Uuid::new_v4())
        .bind(Uuid::parse_str(RFQ_ID).unwrap())
        .execute(&app.pool).await;
    let db_error =
        attempted_item.expect_err("an open RFQ must reject changes to its required item set");
    assert_eq!(
        db_error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("23514")
    );
}

/// Wait until both API requests have reached the real PostgreSQL INSERT and are
/// blocked by the test-held table lock. This proves two independent server-side
/// sessions are at the conflict point before the gate is released.
async fn wait_for_blocked_quote_inserts(pool: &PgPool, expected: i64) {
    let deadline = Instant::now() + Duration::from_secs(15);

    loop {
        let blocked: i64 = sqlx::query_scalar(
            r#"
            SELECT count(DISTINCT pid)
            FROM pg_stat_activity
            WHERE datname = current_database()
              AND usename = current_user
              AND pid <> pg_backend_pid()
              AND state = 'active'
              AND wait_event_type = 'Lock'
              AND wait_event = 'relation'
              AND query LIKE 'INSERT INTO supplier_quotes (%'
            "#,
        )
        .fetch_one(pool)
        .await
        .expect("test role must be able to inspect its PostgreSQL sessions");

        if blocked >= expected {
            return;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected} independent PostgreSQL sessions to block at INSERT INTO supplier_quotes; observed {blocked}. The test must not release its gate before both writers reach the conflict point."
        );
        sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test]
async fn concurrent_duplicate_submissions_commit_exactly_one_quote() {
    let app = spawn_app().await;
    let client = Client::new();
    let barrier = Arc::new(Barrier::new(3));
    let url = format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url);

    // Test-only database choreography: SHARE conflicts with the ROW EXCLUSIVE
    // relation lock required by INSERT, while remaining compatible with the
    // RPU's SELECT ... FOR SHARE locks on rfqs and suppliers. This does not alter
    // production code/schema and is not an application-level mutex.
    let mut insert_gate = app.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE supplier_quotes IN SHARE MODE")
        .execute(&mut *insert_gate)
        .await
        .expect("test must acquire the INSERT gate before starting writers");

    let barrier_one = Arc::clone(&barrier);
    let client_one = client.clone();
    let url_one = url.clone();
    let body_one = valid_request(SUPPLIER_ELIGIBLE_ID);
    let first = tokio::spawn(async move {
        barrier_one.wait().await;
        client_one
            .post(url_one)
            .json(&body_one)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    });

    let barrier_two = Arc::clone(&barrier);
    let second = tokio::spawn(async move {
        barrier_two.wait().await;
        client
            .post(url)
            .json(&valid_request(SUPPLIER_ELIGIBLE_ID))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    });

    // The Barrier only starts both HTTP tasks. The database observation below is
    // the synchronization proof: do not release the gate until two different
    // PostgreSQL sessions are waiting on the quote INSERT relation lock.
    barrier.wait().await;
    wait_for_blocked_quote_inserts(&app.pool, 2).await;

    // Both inserts have reached PostgreSQL. Releasing the gate lets them race on
    // the UNIQUE (rfq_id, supplier_id) index; only the winner can commit.
    insert_gate.commit().await.unwrap();

    let mut statuses = vec![first.await.unwrap(), second.await.unwrap()];
    statuses.sort_unstable();
    assert_eq!(
        statuses,
        vec![201, 409],
        "exactly one concurrent submission must succeed (201) and the duplicate must conflict (409), regardless of which request wins the race)",
    );

    let quote_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM supplier_quotes WHERE rfq_id = $1 AND supplier_id = $2",
    )
    .bind(Uuid::parse_str(RFQ_ID).unwrap())
    .bind(Uuid::parse_str(SUPPLIER_ELIGIBLE_ID).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        quote_count, 1,
        "the Application State must contain exactly one quote for this RFQ/supplier pair",
    );
}

#[tokio::test]
async fn concurrent_close_and_submission_respect_command_context_consistency() {
    let app = spawn_app().await;
    let client = Client::new();
    let barrier = Arc::new(Barrier::new(3));
    let close_url = format!("{}/api/rfqs/{RFQ_ID}/close", app.base_url);
    let quote_url = format!("{}/api/rfqs/{RFQ_ID}/quotes", app.base_url);

    let close_barrier = Arc::clone(&barrier);
    let close_client = client.clone();
    let close = tokio::spawn(async move {
        close_barrier.wait().await;
        close_client
            .post(close_url)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    });

    let quote_barrier = Arc::clone(&barrier);
    let quote = tokio::spawn(async move {
        quote_barrier.wait().await;
        client
            .post(quote_url)
            .json(&valid_request(SUPPLIER_ELIGIBLE_ID))
            .send()
            .await
            .unwrap()
    });

    barrier.wait().await;
    assert_eq!(
        close.await.unwrap(),
        200,
        "the Close RFQ command should win or wait and then close the RFQ"
    );
    let quote_response = quote.await.unwrap();
    let quote_status = quote_response.status();
    assert!(quote_status == StatusCode::CREATED || quote_status == StatusCode::CONFLICT);
    if quote_status == StatusCode::CONFLICT {
        let body: Value = quote_response.json().await.unwrap();
        assert_eq!(body["code"], "rfq_closed");
    }

    let status: String = sqlx::query_scalar("SELECT status FROM rfqs WHERE id = $1")
        .bind(Uuid::parse_str(RFQ_ID).unwrap())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(status, "closed");
    let quote_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM supplier_quotes WHERE rfq_id = $1")
            .bind(Uuid::parse_str(RFQ_ID).unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let expected_count = if quote_status == StatusCode::CREATED {
        1_i64
    } else {
        0_i64
    };
    assert_eq!(quote_count, expected_count);
}
