use crate::error::AppError;
use crate::presentation::market::HistoricalPrice;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use tracing::{info, warn};

/// Initialize the historical_prices table in PostgreSQL
pub async fn initialize_historical_prices_table(pool: &PgPool) -> Result<(), sqlx::Error> {
    info!("Initializing historical_prices table...");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS historical_prices (
            id BIGSERIAL PRIMARY KEY,
            epic VARCHAR(255) NOT NULL,
            resolution VARCHAR(50) NOT NULL,
            snapshot_time TIMESTAMPTZ NOT NULL,
            open_bid DOUBLE PRECISION,
            open_ask DOUBLE PRECISION,
            open_last_traded DOUBLE PRECISION,
            high_bid DOUBLE PRECISION,
            high_ask DOUBLE PRECISION,
            high_last_traded DOUBLE PRECISION,
            low_bid DOUBLE PRECISION,
            low_ask DOUBLE PRECISION,
            low_last_traded DOUBLE PRECISION,
            close_bid DOUBLE PRECISION,
            close_ask DOUBLE PRECISION,
            close_last_traded DOUBLE PRECISION,
            last_traded_volume BIGINT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(epic, resolution, snapshot_time)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Ensure the table schema has 'resolution' for backwards compatibility
    if let Err(e) = sqlx::query(
        r#"
        ALTER TABLE historical_prices 
        ADD COLUMN IF NOT EXISTS resolution VARCHAR(50) NOT NULL DEFAULT 'UNKNOWN'
        "#,
    )
    .execute(pool)
    .await
    {
        info!(
            "Column 'resolution' check/migration skipped or already present: {}",
            e
        );
    }

    // Migrate the unique constraint to (epic, resolution, snapshot_time).
    //
    // Postgres' extended/prepared protocol rejects multiple statements in a
    // single query, so each DDL statement must be issued on its own. Errors
    // are surfaced rather than silently discarded.

    // Drop the legacy two-column constraint if a pre-migration database has it.
    // `IF EXISTS` makes this idempotent. The target three-column constraint is
    // NOT dropped here: `CREATE TABLE ... UNIQUE(epic, resolution, snapshot_time)`
    // above already creates it (auto-named `historical_prices_epic_resolution_snapshot_time_key`),
    // so dropping and re-adding it on every startup would churn an
    // AccessExclusive lock for nothing. The tolerant ADD below handles the
    // fresh case (constraint already present → 42710 → skipped), the migration
    // case (old two-column DB whose legacy constraint was just dropped), and
    // the orphan-index case (an index by the constraint's name exists without
    // an attached constraint → 42P07 → skipped).
    sqlx::query(
        "ALTER TABLE historical_prices \
         DROP CONSTRAINT IF EXISTS historical_prices_epic_snapshot_time_key",
    )
    .execute(pool)
    .await?;

    // Add the three-column unique constraint. There is no `IF NOT EXISTS` for
    // `ADD CONSTRAINT`, so tolerate the "already exists" SQLSTATEs (42710 for
    // the constraint, 42P07 for its backing index) while surfacing any other
    // failure.
    if let Err(e) = sqlx::query(
        "ALTER TABLE historical_prices \
         ADD CONSTRAINT historical_prices_epic_resolution_snapshot_time_key \
         UNIQUE (epic, resolution, snapshot_time)",
    )
    .execute(pool)
    .await
    {
        match e.as_database_error().and_then(|db| db.code()) {
            Some(code) if is_benign_already_exists(&code) => {
                warn!(
                    constraint = "historical_prices_epic_resolution_snapshot_time_key",
                    sqlstate = %code,
                    "unique constraint or its backing index already exists, skipping add"
                );
            }
            _ => return Err(e),
        }
    }

    // Create index for better query performance
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_historical_prices_epic_res_time 
        ON historical_prices(epic, resolution, snapshot_time DESC)
        "#,
    )
    .execute(pool)
    .await?;

    // Create trigger for updating updated_at timestamp
    sqlx::query(
        r#"
        CREATE OR REPLACE FUNCTION update_updated_at_column()
        RETURNS TRIGGER AS $$
        BEGIN
            NEW.updated_at = NOW();
            RETURN NEW;
        END;
        $$ language 'plpgsql'
        "#,
    )
    .execute(pool)
    .await?;

    // Drop existing trigger if it exists
    sqlx::query(
        r#"
        DROP TRIGGER IF EXISTS update_historical_prices_updated_at ON historical_prices
        "#,
    )
    .execute(pool)
    .await?;

    // Create the trigger
    sqlx::query(
        r#"
        CREATE TRIGGER update_historical_prices_updated_at
            BEFORE UPDATE ON historical_prices
            FOR EACH ROW
            EXECUTE FUNCTION update_updated_at_column()
        "#,
    )
    .execute(pool)
    .await?;

    info!("✅ Historical prices table initialized successfully");
    Ok(())
}

/// Returns `true` for the PostgreSQL SQLSTATEs that mean the unique
/// constraint (or its backing index) already exists: `42710`
/// (duplicate_object — the constraint is already present) and `42P07`
/// (duplicate_table — `ADD CONSTRAINT ... UNIQUE` implicitly creates a
/// backing index of the same name, so a pre-existing relation by that name
/// surfaces as "relation already exists").
#[must_use]
#[inline]
fn is_benign_already_exists(code: &str) -> bool {
    matches!(code, "42710" | "42P07")
}

/// Storage statistics for tracking insert/update operations
#[derive(Debug, Default)]
pub struct StorageStats {
    /// Number of new records inserted into the database
    pub inserted: usize,
    /// Number of existing records updated in the database
    pub updated: usize,
    /// Number of records skipped due to errors or validation issues
    pub skipped: usize,
    /// Total number of records processed (inserted + updated + skipped)
    pub total_processed: usize,
}

/// Store historical prices in PostgreSQL with UPSERT logic
pub async fn store_historical_prices(
    pool: &PgPool,
    epic: &str,
    resolution: &str,
    prices: &[HistoricalPrice],
) -> Result<StorageStats, sqlx::Error> {
    let mut stats = StorageStats::default();
    let mut tx = pool.begin().await?;

    info!(
        "Processing {} price records for epic: {}",
        prices.len(),
        epic
    );

    for (i, price) in prices.iter().enumerate() {
        stats.total_processed += 1;

        // Parse snapshot time. Prefer the UTC value IG provides in
        // `snapshotTimeUTC`; the plain `snapshotTime` is in the account's
        // timezone, so storing it would shift every row by the account offset.
        let raw_snapshot_time = price
            .snapshot_time_utc
            .as_deref()
            .unwrap_or(&price.snapshot_time);
        let snapshot_time = match parse_snapshot_time(raw_snapshot_time) {
            Ok(time) => time,
            Err(e) => {
                warn!(
                    "⚠️  Skipping record {}: Invalid timestamp '{}': {}",
                    i + 1,
                    raw_snapshot_time,
                    e
                );
                stats.skipped += 1;
                continue;
            }
        };

        // Use UPSERT (INSERT ... ON CONFLICT ... DO UPDATE) and classify the
        // outcome from the SAME statement via `RETURNING (xmax = 0)`:
        // `xmax = 0` on the returned tuple means a fresh insert, while a
        // non-zero `xmax` means the conflicting row was updated. This avoids a
        // second round trip and is correct for same-batch duplicates (the older
        // `created_at = updated_at` heuristic double-counted them as inserts).
        let row = sqlx::query(
            r#"
            INSERT INTO historical_prices (
                epic, resolution, snapshot_time,
                open_bid, open_ask, open_last_traded,
                high_bid, high_ask, high_last_traded,
                low_bid, low_ask, low_last_traded,
                close_bid, close_ask, close_last_traded,
                last_traded_volume
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT (epic, resolution, snapshot_time)
            DO UPDATE SET
                open_bid = EXCLUDED.open_bid,
                open_ask = EXCLUDED.open_ask,
                open_last_traded = EXCLUDED.open_last_traded,
                high_bid = EXCLUDED.high_bid,
                high_ask = EXCLUDED.high_ask,
                high_last_traded = EXCLUDED.high_last_traded,
                low_bid = EXCLUDED.low_bid,
                low_ask = EXCLUDED.low_ask,
                low_last_traded = EXCLUDED.low_last_traded,
                close_bid = EXCLUDED.close_bid,
                close_ask = EXCLUDED.close_ask,
                close_last_traded = EXCLUDED.close_last_traded,
                last_traded_volume = EXCLUDED.last_traded_volume,
                updated_at = NOW()
            RETURNING (xmax = 0) AS inserted
            "#,
        )
        .bind(epic)
        .bind(resolution)
        .bind(snapshot_time)
        .bind(price.open_price.bid)
        .bind(price.open_price.ask)
        .bind(price.open_price.last_traded)
        .bind(price.high_price.bid)
        .bind(price.high_price.ask)
        .bind(price.high_price.last_traded)
        .bind(price.low_price.bid)
        .bind(price.low_price.ask)
        .bind(price.low_price.last_traded)
        .bind(price.close_price.bid)
        .bind(price.close_price.ask)
        .bind(price.close_price.last_traded)
        .bind(price.last_traded_volume)
        .fetch_one(&mut *tx)
        .await?;

        // `DO UPDATE` always returns exactly one row, so classification is
        // unambiguous.
        if row.get::<bool, _>("inserted") {
            stats.inserted += 1;
        } else {
            stats.updated += 1;
        }

        // Log progress every 100 records
        if (i + 1) % 100 == 0 {
            info!("  Processed {}/{} records...", i + 1, prices.len());
        }
    }

    tx.commit().await?;
    info!("✅ Transaction committed successfully");

    Ok(stats)
}

/// Parse snapshot time from IG format to `DateTime<Utc>`
///
/// # Errors
///
/// Returns `AppError::Generic` if the timestamp cannot be parsed with any supported format.
pub fn parse_snapshot_time(snapshot_time: &str) -> Result<DateTime<Utc>, AppError> {
    // IG formats: "yyyy/MM/dd hh:mm:ss" / "yyyy-MM-dd hh:mm:ss" (snapshotTime),
    // and the ISO-8601 "yyyy-MM-ddTHH:mm:ss" that snapshotTimeUTC uses.
    let formats = [
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y/%m/%d %H:%M",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ];

    for format in &formats {
        if let Ok(naive_dt) = chrono::NaiveDateTime::parse_from_str(snapshot_time, format) {
            return Ok(DateTime::from_naive_utc_and_offset(naive_dt, Utc));
        }
    }

    Err(AppError::Generic(format!(
        "Unable to parse timestamp: {}",
        snapshot_time
    )))
}

/// Database statistics for a specific epic
#[derive(Debug)]
pub struct TableStats {
    /// Total number of records in the database for this epic
    pub total_records: i64,
    /// Earliest date in the dataset (formatted as string)
    pub earliest_date: String,
    /// Latest date in the dataset (formatted as string)
    pub latest_date: String,
    /// Average closing price across all records
    pub avg_close_price: f64,
    /// Minimum price (lowest of all low prices) in the dataset
    pub min_price: f64,
    /// Maximum price (highest of all high prices) in the dataset
    pub max_price: f64,
}

/// Get statistics for the historical_prices table
pub async fn get_table_statistics(
    pool: &PgPool,
    epic: &str,
    resolution: Option<&str>,
) -> Result<TableStats, sqlx::Error> {
    let row = if let Some(res) = resolution {
        sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_records,
                MIN(snapshot_time)::text as earliest_date,
                MAX(snapshot_time)::text as latest_date,
                AVG(close_bid) as avg_close_price,
                MIN(LEAST(low_bid, low_ask)) as min_price,
                MAX(GREATEST(high_bid, high_ask)) as max_price
            FROM historical_prices 
            WHERE epic = $1 AND resolution = $2
            "#,
        )
        .bind(epic)
        .bind(res)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_records,
                MIN(snapshot_time)::text as earliest_date,
                MAX(snapshot_time)::text as latest_date,
                AVG(close_bid) as avg_close_price,
                MIN(LEAST(low_bid, low_ask)) as min_price,
                MAX(GREATEST(high_bid, high_ask)) as max_price
            FROM historical_prices 
            WHERE epic = $1
            "#,
        )
        .bind(epic)
        .fetch_one(pool)
        .await?
    };

    // `COUNT(*)` is never NULL, but the aggregate columns
    // (`MIN`/`MAX`/`AVG`, and the `::text` date bounds) are all NULL for an
    // epic with no rows. Read every nullable column as an `Option` so a
    // zero-row epic returns zeroed stats instead of panicking in `Row::get`.
    let total_records: i64 = row.get("total_records");
    if total_records == 0 {
        return Ok(TableStats {
            total_records: 0,
            earliest_date: String::new(),
            latest_date: String::new(),
            avg_close_price: 0.0,
            min_price: 0.0,
            max_price: 0.0,
        });
    }

    Ok(TableStats {
        total_records,
        earliest_date: row
            .get::<Option<String>, _>("earliest_date")
            .unwrap_or_default(),
        latest_date: row
            .get::<Option<String>, _>("latest_date")
            .unwrap_or_default(),
        avg_close_price: row.get::<Option<f64>, _>("avg_close_price").unwrap_or(0.0),
        min_price: row.get::<Option<f64>, _>("min_price").unwrap_or(0.0),
        max_price: row.get::<Option<f64>, _>("max_price").unwrap_or(0.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_snapshot_time_slash_format() {
        let result = parse_snapshot_time("2024/01/15 14:30:00");
        assert!(result.is_ok());
        let dt = result.expect("should parse");
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2024-01-15 14:30:00"
        );
    }

    #[test]
    fn test_parse_snapshot_time_dash_format() {
        let result = parse_snapshot_time("2024-01-15 14:30:00");
        assert!(result.is_ok());
        let dt = result.expect("should parse");
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2024-01-15 14:30:00"
        );
    }

    #[test]
    fn test_parse_snapshot_time_iso_utc_format() {
        // snapshotTimeUTC uses the ISO-8601 `T` separator, with or without `Z`.
        for input in ["2024-01-15T14:30:00", "2024-01-15T14:30:00Z"] {
            let dt =
                parse_snapshot_time(input).unwrap_or_else(|e| panic!("should parse {input}: {e}"));
            assert_eq!(
                dt.format("%Y-%m-%d %H:%M:%S").to_string(),
                "2024-01-15 14:30:00"
            );
        }
    }

    #[test]
    fn test_parse_snapshot_time_without_seconds_slash() {
        let result = parse_snapshot_time("2024/01/15 14:30");
        assert!(result.is_ok());
        let dt = result.expect("should parse");
        assert_eq!(dt.format("%Y-%m-%d %H:%M").to_string(), "2024-01-15 14:30");
    }

    #[test]
    fn test_parse_snapshot_time_without_seconds_dash() {
        let result = parse_snapshot_time("2024-01-15 14:30");
        assert!(result.is_ok());
        let dt = result.expect("should parse");
        assert_eq!(dt.format("%Y-%m-%d %H:%M").to_string(), "2024-01-15 14:30");
    }

    #[test]
    fn test_parse_snapshot_time_invalid_format() {
        let result = parse_snapshot_time("invalid-date");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_snapshot_time_rejects_out_of_range_and_wrong_shapes() {
        // Out-of-range components and unsupported separators/orderings must all
        // fail rather than silently coerce.
        for bad in [
            "2025/13/01 00:00:00",       // invalid month
            "2025-10-32 00:00:00",       // invalid day
            "20-10-2025 00:00:00",       // day-month-year order (unsupported)
            "2025-10-20 19:22:33+02:00", // numeric offset (unsupported)
        ] {
            assert!(
                parse_snapshot_time(bad).is_err(),
                "should fail for input: {bad}"
            );
        }
    }

    #[test]
    fn test_parse_snapshot_time_empty_string() {
        let result = parse_snapshot_time("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_snapshot_time_partial_date() {
        let result = parse_snapshot_time("2024-01-15");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_snapshot_time_midnight() {
        let result = parse_snapshot_time("2024/12/31 00:00:00");
        assert!(result.is_ok());
        let dt = result.expect("should parse");
        assert_eq!(dt.format("%H:%M:%S").to_string(), "00:00:00");
    }

    #[test]
    fn test_parse_snapshot_time_end_of_day() {
        let result = parse_snapshot_time("2024/12/31 23:59:59");
        assert!(result.is_ok());
        let dt = result.expect("should parse");
        assert_eq!(dt.format("%H:%M:%S").to_string(), "23:59:59");
    }

    #[test]
    fn test_storage_stats_default() {
        let stats = StorageStats::default();
        assert_eq!(stats.inserted, 0);
        assert_eq!(stats.updated, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.total_processed, 0);
    }

    #[test]
    fn test_storage_stats_creation() {
        let stats = StorageStats {
            inserted: 10,
            updated: 5,
            skipped: 2,
            total_processed: 17,
        };
        assert_eq!(stats.inserted, 10);
        assert_eq!(stats.updated, 5);
        assert_eq!(stats.skipped, 2);
        assert_eq!(stats.total_processed, 17);
    }

    #[test]
    fn test_table_stats_creation() {
        let stats = TableStats {
            total_records: 100,
            earliest_date: "2024-01-01".to_string(),
            latest_date: "2024-12-31".to_string(),
            avg_close_price: 150.5,
            min_price: 100.0,
            max_price: 200.0,
        };
        assert_eq!(stats.total_records, 100);
        assert_eq!(stats.earliest_date, "2024-01-01");
        assert_eq!(stats.latest_date, "2024-12-31");
        assert!((stats.avg_close_price - 150.5).abs() < f64::EPSILON);
        assert!((stats.min_price - 100.0).abs() < f64::EPSILON);
        assert!((stats.max_price - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_snapshot_time_different_years() {
        let years = ["2020", "2021", "2022", "2023", "2024", "2025"];
        for year in years {
            let timestamp = format!("{}/06/15 12:00:00", year);
            let result = parse_snapshot_time(&timestamp);
            assert!(result.is_ok(), "Failed for year: {}", year);
        }
    }

    #[test]
    fn test_parse_snapshot_time_all_months() {
        for month in 1..=12 {
            let timestamp = format!("2024/{:02}/15 12:00:00", month);
            let result = parse_snapshot_time(&timestamp);
            assert!(result.is_ok(), "Failed for month: {}", month);
        }
    }

    #[test]
    fn test_is_benign_already_exists_duplicate_object_true() {
        // 42710 = duplicate_object: the constraint itself already exists.
        assert!(is_benign_already_exists("42710"));
    }

    #[test]
    fn test_is_benign_already_exists_duplicate_relation_true() {
        // 42P07 = duplicate_table: the backing index name already exists.
        assert!(is_benign_already_exists("42P07"));
    }

    #[test]
    fn test_is_benign_already_exists_other_code_false() {
        // Unrelated SQLSTATEs must keep propagating as errors.
        assert!(!is_benign_already_exists("23505")); // unique_violation
        assert!(!is_benign_already_exists("42P01")); // undefined_table
        assert!(!is_benign_already_exists(""));
    }
}
