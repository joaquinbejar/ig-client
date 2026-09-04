use crate::error::AppError;
use crate::presentation::transaction::StoreTransaction;
use crate::storage::config::DatabaseConfig;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json;
use sqlx::{Executor, PgPool};
use tracing::info;

/// Initializes the `ig_options` table used by [`store_transactions`].
///
/// Ships the DDL in code (like the other storage tables) so persistence works
/// against a fresh database. The `raw_hash` column is the deduplication key:
/// it stores the MD5 digest of the raw transaction JSON and carries a `UNIQUE`
/// constraint so re-ingesting an identical payload is a no-op. Call this once
/// before [`store_transactions`].
///
/// # Arguments
/// * `pool` - PostgreSQL connection pool
///
/// # Errors
/// Returns [`AppError`] if the `CREATE TABLE` statement fails.
#[must_use = "table initialization can fail and must be handled"]
pub async fn initialize_ig_options_table(pool: &PgPool) -> Result<(), AppError> {
    info!("Initializing ig_options table...");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS ig_options (
            id BIGSERIAL PRIMARY KEY,
            reference TEXT NOT NULL,
            deal_date TIMESTAMPTZ NOT NULL,
            underlying TEXT,
            strike DOUBLE PRECISION,
            option_type TEXT,
            expiry DATE,
            transaction_type TEXT NOT NULL,
            pnl_eur DOUBLE PRECISION NOT NULL,
            is_fee BOOLEAN NOT NULL,
            raw TEXT NOT NULL,
            raw_hash TEXT NOT NULL UNIQUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ig_options_reference ON ig_options(reference)")
        .execute(pool)
        .await?;

    info!("ig_options table initialized successfully");
    Ok(())
}

/// Stores a list of transactions in the database.
///
/// Requires the `ig_options` table to exist; call
/// [`initialize_ig_options_table`] first. Deduplication is keyed on
/// `raw_hash`, which is computed database-side as `md5(raw)` from the bound
/// raw JSON (a stable, dependency-free digest); `ON CONFLICT (raw_hash) DO
/// NOTHING` makes re-ingesting an identical payload a no-op.
///
/// # Arguments
/// * `pool` - PostgreSQL connection pool
/// * `txs` - List of transactions to store
///
/// # Returns
/// * `Result<usize, AppError>` - Number of transactions inserted or an error
#[must_use = "storage writes can fail and the inserted count must be handled"]
pub async fn store_transactions(
    pool: &sqlx::PgPool,
    txs: &[StoreTransaction],
) -> Result<usize, AppError> {
    let mut tx = pool.begin().await?;
    let mut inserted = 0;

    for t in txs {
        // `raw_hash` is derived from the raw JSON via Postgres' built-in
        // `md5()` on the bound `$10` value (no string interpolation), giving a
        // deterministic dedup key without an extra dependency.
        let result = tx
            .execute(
                sqlx::query(
                    r#"
                    INSERT INTO ig_options (
                        reference, deal_date, underlying, strike,
                        option_type, expiry, transaction_type, pnl_eur, is_fee, raw,
                        raw_hash
                    )
                    VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10, md5($10))
                    ON CONFLICT (raw_hash) DO NOTHING
                    "#,
                )
                .bind(&t.reference)
                .bind(t.deal_date)
                .bind(&t.underlying)
                .bind(t.strike)
                .bind(&t.option_type)
                .bind(t.expiry)
                .bind(&t.transaction_type)
                .bind(t.pnl_eur)
                .bind(t.is_fee)
                .bind(&t.raw_json),
            )
            .await?;

        inserted += result.rows_affected() as usize;
    }

    tx.commit().await?;
    Ok(inserted)
}

/// Serializes a value to a JSON string
pub fn serialize_to_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

/// Deserializes a JSON string into a value
pub fn deserialize_from_json<T: DeserializeOwned>(s: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(s)
}

/// Creates a PostgreSQL connection pool from database configuration
///
/// # Arguments
/// * `config` - Database configuration containing URL and max connections
///
/// # Returns
/// * `Result<PgPool, AppError>` - Connection pool or an error
pub async fn create_connection_pool(config: &DatabaseConfig) -> Result<PgPool, AppError> {
    info!(
        "Creating PostgreSQL connection pool with max {} connections",
        config.max_connections
    );

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.url)
        .await
        .map_err(AppError::Db)?;

    info!("PostgreSQL connection pool created successfully");
    Ok(pool)
}

/// Creates a database configuration from environment variables
///
/// # Returns
/// * `Result<DatabaseConfig, AppError>` - Database configuration or an error
pub fn create_database_config_from_env() -> Result<DatabaseConfig, AppError> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").map_err(|_| {
        AppError::InvalidInput("DATABASE_URL environment variable is required".to_string())
    })?;

    let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
        .unwrap_or_else(|_| "10".to_string())
        .parse::<u32>()
        .map_err(|e| {
            AppError::InvalidInput(format!("Invalid DATABASE_MAX_CONNECTIONS value: {e}"))
        })?;

    Ok(DatabaseConfig {
        url,
        max_connections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestStruct {
        name: String,
        value: i32,
        optional: Option<f64>,
    }

    #[test]
    fn test_serialize_to_json_simple_struct() {
        let test_data = TestStruct {
            name: "test".to_string(),
            value: 42,
            optional: Some(3.15),
        };

        let result = serialize_to_json(&test_data);
        assert!(result.is_ok());

        let json = result.expect("should serialize");
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"value\":42"));
        assert!(json.contains("\"optional\":3.15"));
    }

    #[test]
    fn test_serialize_to_json_with_none() {
        let test_data = TestStruct {
            name: "none_test".to_string(),
            value: 0,
            optional: None,
        };

        let result = serialize_to_json(&test_data);
        assert!(result.is_ok());

        let json = result.expect("should serialize");
        assert!(json.contains("\"optional\":null"));
    }

    #[test]
    fn test_deserialize_from_json_valid() {
        let json = r#"{"name":"deserialized","value":100,"optional":2.5}"#;

        let result: Result<TestStruct, _> = deserialize_from_json(json);
        assert!(result.is_ok());

        let data = result.expect("should deserialize");
        assert_eq!(data.name, "deserialized");
        assert_eq!(data.value, 100);
        assert_eq!(data.optional, Some(2.5));
    }

    #[test]
    fn test_deserialize_from_json_with_null() {
        let json = r#"{"name":"null_test","value":50,"optional":null}"#;

        let result: Result<TestStruct, _> = deserialize_from_json(json);
        assert!(result.is_ok());

        let data = result.expect("should deserialize");
        assert_eq!(data.name, "null_test");
        assert_eq!(data.value, 50);
        assert!(data.optional.is_none());
    }

    #[test]
    fn test_deserialize_from_json_invalid() {
        let json = r#"{"invalid": "json for TestStruct"}"#;

        let result: Result<TestStruct, _> = deserialize_from_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_from_json_malformed() {
        let json = r#"{"name": "incomplete"#;

        let result: Result<TestStruct, _> = deserialize_from_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let original = TestStruct {
            name: "roundtrip".to_string(),
            value: 999,
            optional: Some(1.234),
        };

        let json = serialize_to_json(&original).expect("should serialize");
        let deserialized: TestStruct = deserialize_from_json(&json).expect("should deserialize");

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_serialize_vec() {
        let vec = vec![
            TestStruct {
                name: "first".to_string(),
                value: 1,
                optional: None,
            },
            TestStruct {
                name: "second".to_string(),
                value: 2,
                optional: Some(2.0),
            },
        ];

        let result = serialize_to_json(&vec);
        assert!(result.is_ok());

        let json = result.expect("should serialize");
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
        assert!(json.contains("\"first\""));
        assert!(json.contains("\"second\""));
    }

    #[test]
    fn test_deserialize_vec() {
        let json =
            r#"[{"name":"a","value":1,"optional":null},{"name":"b","value":2,"optional":3.0}]"#;

        let result: Result<Vec<TestStruct>, _> = deserialize_from_json(json);
        assert!(result.is_ok());

        let vec = result.expect("should deserialize");
        assert_eq!(vec.len(), 2);
        assert_eq!(vec[0].name, "a");
        assert_eq!(vec[1].name, "b");
    }

    #[test]
    fn test_serialize_empty_string() {
        let test_data = TestStruct {
            name: String::new(),
            value: 0,
            optional: None,
        };

        let result = serialize_to_json(&test_data);
        assert!(result.is_ok());

        let json = result.expect("should serialize");
        assert!(json.contains("\"name\":\"\""));
    }

    #[test]
    fn test_serialize_special_characters() {
        let test_data = TestStruct {
            name: "test\"with\\special\nchars".to_string(),
            value: 0,
            optional: None,
        };

        let result = serialize_to_json(&test_data);
        assert!(result.is_ok());

        let json = result.expect("should serialize");
        // JSON should escape special characters
        assert!(json.contains("\\\""));
        assert!(json.contains("\\\\"));
        assert!(json.contains("\\n"));
    }

    #[test]
    fn test_database_config_creation() {
        let config = DatabaseConfig {
            url: "postgres://localhost/test".to_string(),
            max_connections: 5,
        };
        assert_eq!(config.url, "postgres://localhost/test");
        assert_eq!(config.max_connections, 5);
    }
}
