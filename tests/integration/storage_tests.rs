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

use ig_client::presentation::market::{HistoricalPrice, PricePoint};
use ig_client::presentation::transaction::StoreTransaction;
use ig_client::storage::historical_prices::{
    get_table_statistics, initialize_historical_prices_table, store_historical_prices,
};
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
