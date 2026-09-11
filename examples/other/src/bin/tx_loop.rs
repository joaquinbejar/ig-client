//! Transaction Loop Example
//!
//! This example demonstrates how to periodically fetch and store transactions
//! from the IG Markets API. It runs in a continuous loop, fetching transactions
//! at regular intervals and storing them in a PostgreSQL database.
//!
//! The example uses environment variables for configuration. The shared client
//! handles request pacing, finite retries, and transaction pagination. A failed
//! fetch skips storage for that run and is retried at the next scheduled tick.

use chrono::{Duration, Utc};
use ig_client::prelude::*;
use ig_client::presentation::transaction::TransactionList;
use ig_client::storage::utils::store_transactions;
use std::time::Duration as StdDuration;
use tokio::{signal, time};
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    info!("=== Transaction Loop Service ===");

    let config = Config::default();
    info!(
        "Configuration: interval={} hours, lookback={} days",
        config.sleep_hours, config.days_to_look_back
    );
    debug!(
        "Loaded config: database max_connections={}",
        config.database.max_connections
    );

    // Build the Postgres pool once at startup
    let pool = match create_connection_pool(&config.database).await {
        Ok(pool) => {
            info!("Postgres pool established");
            pool
        }
        Err(e) => {
            error!("Failed to establish database connection: {}", e);
            return Err(e);
        }
    };

    // Retain one client so sessions and rate budgets are shared across ticks.
    let client = Client::with_config(config.clone())?;

    // Set up signal handlers for graceful shutdown
    let ctrl_c = signal::ctrl_c();
    tokio::pin!(ctrl_c);

    let interval_seconds = config
        .sleep_hours
        .checked_mul(3600)
        .filter(|seconds| *seconds > 0)
        .ok_or_else(|| {
            AppError::InvalidInput(
                "transaction polling interval must be positive and fit in seconds".to_owned(),
            )
        })?;
    let hour_interval = time::interval(StdDuration::from_secs(interval_seconds));
    tokio::pin!(hour_interval);

    info!(
        "Service started, will fetch transactions every {} hours",
        config.sleep_hours
    );

    // Immediately run once, then continue with the hourly interval
    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                info!("Received shutdown signal, terminating gracefully");
                break;
            }
            _ = hour_interval.tick() => {
                // If this is the first run, the interval will tick immediately
                info!("Starting scheduled transaction fetch");

                // Calculate date range
                let to = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
                let from = (Utc::now() - Duration::days(config.days_to_look_back))
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string();

                info!("Fetching transactions from {} to {}", from, to);

                // This public method already fetches and combines every page.
                let history = match client
                    .get_transactions(
                        &from,
                        &to,
                    )
                    .await {
                    Ok(transactions) => transactions,
                    Err(e) => {
                        error!("Failed to get transactions: {}", e);
                        continue; // Skip this iteration and try again
                    }
                };

                let all_transactions = history.transactions;
                info!("Total transactions fetched: {}", all_transactions.len());

                // Log transaction details at debug level
                for (i, transaction) in all_transactions.iter().enumerate() {
                    debug!(
                        "Transaction #{}: {}",
                        i + 1,
                        serde_json::to_string_pretty(transaction)?
                    );
                }

                // Convert and store transactions
                if !all_transactions.is_empty() {
                    let tx_list = TransactionList::from(&all_transactions);
                    let tx_ref = tx_list.as_ref();

                    match store_transactions(&pool, tx_ref).await {
                        Ok(inserted) => {
                            info!("Successfully stored {} transactions in database", inserted);
                        }
                        Err(e) => {
                            error!("Error storing transactions: {}", e);
                        }
                    };
                } else {
                    info!("No transactions to store for the specified period");
                }
            }
        }
    }

    info!("Service shutting down");
    Ok(())
}
