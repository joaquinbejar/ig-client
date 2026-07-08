//! Env-gated storage integration tests (issue #41 storage correctness).
//!
//! These tests require a live PostgreSQL and are marked `#[ignore]` so they
//! never run in the default CI matrix. Run them explicitly against a
//! disposable test database:
//!
//! ```bash
//! DATABASE_URL=postgres://user:pass@localhost/ig_test \
//!     cargo test --test integration_tests storage_tests -- --ignored
//! ```
//!
//! Each test skips silently when `DATABASE_URL` is unset.

use ig_client::presentation::instrument::InstrumentType;
use ig_client::presentation::market::{HistoricalPrice, MarketData, MarketNode, PricePoint};
use ig_client::presentation::transaction::StoreTransaction;
use ig_client::storage::historical_prices::{
    get_table_statistics, initialize_historical_prices_table, store_historical_prices,
};
use ig_client::storage::market_database::MarketDatabaseService;
use ig_client::storage::utils::{initialize_ig_options_table, store_transactions};
use sqlx::{PgPool, Row};

/// Returns a pool when `DATABASE_URL` is set, otherwise `None` so the test
/// skips instead of failing on CI machines with no database.
///
/// If `DATABASE_URL` **is** set but the connection fails, this panics rather
/// than returning `None` — a broken DB configuration must surface as a failure
/// when the ignored tests are run with `--ignored`, not silently pass.
async fn test_pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("DATABASE_URL is set but the Postgres connection failed");
    Some(pool)
}

fn sample_price(snapshot: &str) -> HistoricalPrice {
    HistoricalPrice {
        snapshot_time: snapshot.to_string(),
        snapshot_time_utc: None,
        open_price: PricePoint {
            bid: Some(1.0),
            ask: Some(1.1),
            last_traded: None,
        },
        high_price: PricePoint {
            bid: Some(2.0),
            ask: Some(2.1),
            last_traded: None,
        },
        low_price: PricePoint {
            bid: Some(0.5),
            ask: Some(0.6),
            last_traded: None,
        },
        close_price: PricePoint {
            bid: Some(1.5),
            ask: Some(1.6),
            last_traded: None,
        },
        last_traded_volume: Some(100),
    }
}

/// Removes any rows for a test epic so the test starts from a clean slate.
async fn delete_epic(pool: &PgPool, epic: &str) {
    let _ = sqlx::query("DELETE FROM historical_prices WHERE epic = $1")
        .bind(epic)
        .execute(pool)
        .await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL via DATABASE_URL"]
async fn test_initialize_historical_prices_table_creates_unique_constraint() {
    let Some(pool) = test_pool().await else {
        return;
    };

    initialize_historical_prices_table(&pool)
        .await
        .expect("table initialization should succeed");

    // The (epic, resolution, snapshot_time) unique constraint must now exist.
    let row = sqlx::query(
        "SELECT COUNT(*) AS n FROM pg_constraint \
         WHERE conname = 'historical_prices_epic_resolution_snapshot_time_key'",
    )
    .fetch_one(&pool)
    .await
    .expect("constraint lookup should succeed");
    let count: i64 = row.get("n");
    assert_eq!(count, 1, "the three-column unique constraint must exist");

    // Re-running initialization must remain idempotent.
    initialize_historical_prices_table(&pool)
        .await
        .expect("re-initialization should be idempotent");
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL via DATABASE_URL"]
async fn test_initialize_tolerates_orphan_backing_index() {
    let Some(pool) = test_pool().await else {
        return;
    };

    initialize_historical_prices_table(&pool)
        .await
        .expect("baseline initialization should succeed");

    // Reproduce issue #79: an index with the constraint's name exists but no
    // constraint is attached (manually created index, pg_restore, partial
    // migration). `ADD CONSTRAINT ... UNIQUE` then fails with SQLSTATE 42P07
    // ("relation already exists") while trying to create the backing index.
    // All statements run in one transaction so concurrent ignored tests
    // never observe a window without the unique index. `IF EXISTS` on both
    // drops makes the setup idempotent when a prior failed run left the
    // schema in the orphan-index state: dropping the constraint also drops
    // its backing index, and the second drop clears a leftover orphan index.
    let mut tx = pool.begin().await.expect("transaction should start");
    sqlx::query(
        "ALTER TABLE historical_prices \
         DROP CONSTRAINT IF EXISTS historical_prices_epic_resolution_snapshot_time_key",
    )
    .execute(&mut *tx)
    .await
    .expect("dropping the constraint should succeed");
    sqlx::query("DROP INDEX IF EXISTS historical_prices_epic_resolution_snapshot_time_key")
        .execute(&mut *tx)
        .await
        .expect("dropping a leftover orphan index should succeed");
    sqlx::query(
        "CREATE UNIQUE INDEX historical_prices_epic_resolution_snapshot_time_key \
         ON historical_prices (epic, resolution, snapshot_time)",
    )
    .execute(&mut *tx)
    .await
    .expect("creating the orphan index should succeed");
    tx.commit().await.expect("transaction should commit");

    // The migration must tolerate the orphan backing index instead of
    // failing on every startup. Capture the result instead of asserting here
    // so the restore below always runs — a panic at this point would leave
    // the shared schema without its constraint and cascade into the other
    // ignored tests.
    let tolerate_result = initialize_historical_prices_table(&pool).await;

    // Restore the canonical constraint-backed state for the other tests.
    // Dropping the constraint (if any) before the index keeps the cleanup
    // robust even if a future `initialize_historical_prices_table()` were to
    // attach a constraint to the orphan index — a bare `DROP INDEX` would
    // then fail on the dependency.
    let mut tx = pool.begin().await.expect("transaction should start");
    sqlx::query(
        "ALTER TABLE historical_prices \
         DROP CONSTRAINT IF EXISTS historical_prices_epic_resolution_snapshot_time_key",
    )
    .execute(&mut *tx)
    .await
    .expect("dropping a constraint attached to the index should succeed");
    sqlx::query("DROP INDEX IF EXISTS historical_prices_epic_resolution_snapshot_time_key")
        .execute(&mut *tx)
        .await
        .expect("dropping the orphan index should succeed");
    sqlx::query(
        "ALTER TABLE historical_prices \
         ADD CONSTRAINT historical_prices_epic_resolution_snapshot_time_key \
         UNIQUE (epic, resolution, snapshot_time)",
    )
    .execute(&mut *tx)
    .await
    .expect("re-adding the constraint should succeed");
    tx.commit().await.expect("transaction should commit");

    let row = sqlx::query(
        "SELECT COUNT(*) AS n FROM pg_constraint \
         WHERE conname = 'historical_prices_epic_resolution_snapshot_time_key'",
    )
    .fetch_one(&pool)
    .await
    .expect("constraint lookup should succeed");
    let count: i64 = row.get("n");
    assert_eq!(count, 1, "the canonical unique constraint must be restored");

    assert!(
        tolerate_result.is_ok(),
        "initialization must tolerate an orphan backing index (42P07): {:?}",
        tolerate_result.err()
    );
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL via DATABASE_URL"]
async fn test_store_historical_prices_upsert_is_idempotent() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let epic = "TEST.ISSUE41.UPSERT";
    let resolution = "MINUTE";

    initialize_historical_prices_table(&pool)
        .await
        .expect("table initialization should succeed");
    delete_epic(&pool, epic).await;

    let prices = vec![sample_price("2024/01/15 14:30:00")];

    // First write: exactly one insert.
    let first = store_historical_prices(&pool, epic, resolution, &prices)
        .await
        .expect("first store should succeed");
    assert_eq!(first.inserted, 1, "first write must insert");
    assert_eq!(first.updated, 0);
    assert_eq!(first.skipped, 0);

    // Second write with the same conflict key: exactly one update, no insert.
    let second = store_historical_prices(&pool, epic, resolution, &prices)
        .await
        .expect("second store should succeed");
    assert_eq!(second.inserted, 0, "second write must not re-insert");
    assert_eq!(second.updated, 1, "second write must update");
    assert_eq!(second.skipped, 0);

    // Exactly one physical row survives (no duplicate, no delete).
    let row = sqlx::query("SELECT COUNT(*) AS n FROM historical_prices WHERE epic = $1")
        .bind(epic)
        .fetch_one(&pool)
        .await
        .expect("count should succeed");
    let count: i64 = row.get("n");
    assert_eq!(count, 1, "upsert must not create duplicate rows");

    delete_epic(&pool, epic).await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL via DATABASE_URL"]
async fn test_get_table_statistics_empty_epic_returns_zeroed_stats() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let epic = "TEST.ISSUE41.EMPTY";

    initialize_historical_prices_table(&pool)
        .await
        .expect("table initialization should succeed");
    delete_epic(&pool, epic).await;

    // Must not panic on NULL MIN/MAX/AVG aggregates for a zero-row epic.
    let stats = get_table_statistics(&pool, epic, None)
        .await
        .expect("statistics for an empty epic should succeed");

    assert_eq!(stats.total_records, 0);
    assert!(stats.earliest_date.is_empty());
    assert!(stats.latest_date.is_empty());
    assert!((stats.avg_close_price - 0.0).abs() < f64::EPSILON);
    assert!((stats.min_price - 0.0).abs() < f64::EPSILON);
    assert!((stats.max_price - 0.0).abs() < f64::EPSILON);
}

fn market(epic: &str, name: &str) -> MarketData {
    MarketData {
        epic: epic.to_string(),
        instrument_name: name.to_string(),
        instrument_type: InstrumentType::Shares,
        expiry: "DFB".to_string(),
        high_limit_price: Some(100.0),
        low_limit_price: Some(50.0),
        market_status: "TRADEABLE".to_string(),
        net_change: Some(1.0),
        percentage_change: Some(0.5),
        update_time: Some("2024-01-01T00:00:00Z".to_string()),
        update_time_utc: Some("2024-01-01T00:00:00Z".to_string()),
        bid: Some(75.0),
        offer: Some(76.0),
    }
}

/// Verifies the batched (`UNNEST`) `store_market_hierarchy` stores the right
/// rows: it flattens a nested hierarchy, dedupes a duplicate epic to a single
/// instrument, and a re-run (full refresh) is idempotent (no duplicates).
#[tokio::test]
#[ignore = "requires a live PostgreSQL via DATABASE_URL"]
async fn test_store_market_hierarchy_batched_dedupe_and_refresh() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let exchange = "TEST_ISSUE46";
    let service = MarketDatabaseService::new(pool.clone(), exchange.to_string());

    service
        .initialize_database()
        .await
        .expect("schema initialization should succeed");

    // n1 (E1) -> n1c (E2); n2 (E1 duplicate epic, node_id last-wins = n2).
    let child = MarketNode {
        id: "TEST_ISSUE46_n1c".to_string(),
        name: "Child".to_string(),
        children: vec![],
        markets: vec![market("TEST.ISSUE46.E2", "E2")],
    };
    let n1 = MarketNode {
        id: "TEST_ISSUE46_n1".to_string(),
        name: "Node 1".to_string(),
        children: vec![child],
        markets: vec![market("TEST.ISSUE46.E1", "E1")],
    };
    let n2 = MarketNode {
        id: "TEST_ISSUE46_n2".to_string(),
        name: "Node 2".to_string(),
        children: vec![],
        markets: vec![market("TEST.ISSUE46.E1", "E1 updated")],
    };
    let hierarchy = vec![n1, n2];

    service
        .store_market_hierarchy(&hierarchy)
        .await
        .expect("first store should succeed");

    let stats = service
        .get_statistics()
        .await
        .expect("statistics should succeed");
    assert_eq!(stats.node_count, 3, "n1 + n1c + n2");
    assert_eq!(stats.instrument_count, 2, "E1 deduped + E2");

    // Duplicate epic collapsed to one row, last occurrence's data wins.
    let row = sqlx::query(
        "SELECT node_id, instrument_name FROM market_instruments \
         WHERE epic = $1 AND exchange = $2",
    )
    .bind("TEST.ISSUE46.E1")
    .bind(exchange)
    .fetch_one(&pool)
    .await
    .expect("the deduped instrument row should exist exactly once");
    assert_eq!(row.get::<String, _>("node_id"), "TEST_ISSUE46_n2");
    assert_eq!(row.get::<String, _>("instrument_name"), "E1 updated");

    // Re-run: full refresh must not accumulate duplicates.
    service
        .store_market_hierarchy(&hierarchy)
        .await
        .expect("second store (refresh) should succeed");
    let stats2 = service
        .get_statistics()
        .await
        .expect("statistics should succeed");
    assert_eq!(stats2.node_count, 3, "refresh must not duplicate nodes");
    assert_eq!(
        stats2.instrument_count, 2,
        "refresh must not duplicate instruments"
    );

    // Clean up (instruments first for the node_id FK).
    let _ = sqlx::query("DELETE FROM market_instruments WHERE exchange = $1")
        .bind(exchange)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM market_hierarchy_nodes WHERE exchange = $1")
        .bind(exchange)
        .execute(&pool)
        .await;
}

#[tokio::test]
#[ignore = "requires a live PostgreSQL via DATABASE_URL"]
async fn test_store_transactions_dedupes_on_raw_hash() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let reference = "TEST-ISSUE41-DEDUP";

    initialize_ig_options_table(&pool)
        .await
        .expect("ig_options initialization should succeed");
    let _ = sqlx::query("DELETE FROM ig_options WHERE reference = $1")
        .bind(reference)
        .execute(&pool)
        .await;

    let tx = StoreTransaction {
        transaction_type: "DEAL".to_string(),
        pnl_eur: 12.5,
        reference: reference.to_string(),
        raw_json: r#"{"reference":"TEST-ISSUE41-DEDUP","amount":12.5}"#.to_string(),
        ..Default::default()
    };
    let batch = vec![tx];

    // First ingest inserts the row.
    let first = store_transactions(&pool, &batch)
        .await
        .expect("first ingest should succeed");
    assert_eq!(first, 1, "first ingest must insert one row");

    // Re-ingesting the identical raw payload is a no-op (deduped on raw_hash).
    let second = store_transactions(&pool, &batch)
        .await
        .expect("second ingest should succeed");
    assert_eq!(second, 0, "identical payload must be deduplicated");

    let _ = sqlx::query("DELETE FROM ig_options WHERE reference = $1")
        .bind(reference)
        .execute(&pool)
        .await;
}
