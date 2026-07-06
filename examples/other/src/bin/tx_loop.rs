//! Transaction Loop Example
//!
//! This example demonstrates how to periodically fetch and store transactions
//! from the IG Markets API. It runs in a continuous loop, fetching transactions
//! at regular intervals and storing them in a PostgreSQL database.
//!
//! The example uses environment variables for configuration and implements
//! error handling with exponential backoff.

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
        "Configuration: interval={} hours, page_size={}, lookback={} days",
        config.sleep_hours, config.page_size, config.days_to_look_back
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

    // Set up signal handlers for graceful shutdown
    let ctrl_c = signal::ctrl_c();
    tokio::pin!(ctrl_c);

    let hour_interval = time::interval(StdDuration::from_secs(config.sleep_hours * 3600));
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

                // Create client
                let client = Client::default();

                // Calculate date range
                let to = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
                let from = (Utc::now() - Duration::days(config.days_to_look_back))
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string();

                info!("Fetching transactions from {} to {}", from, to);

                // Fetch first page to get total pages
                let first_page = match client
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

                // Calculate total pages
                let total_pages = first_page.metadata.page_data.total_pages as u32;

                info!("Found {} transactions in page 1 of {}", first_page.transactions.len(), total_pages);

                // Process first page
                let mut all_transactions = first_page.transactions;

                // Fetch remaining pages if any
                for page in 2..=total_pages {
                    info!("Fetching page {} of {}", page, total_pages);

                    // Add a small delay between requests to avoid rate limiting
                    time::sleep(StdDuration::from_millis(500)).await;

                    match client
                        .get_transactions(
                            &from,
                            &to,
                        )
                        .await {
                        Ok(page_data) => {
                            info!("Retrieved {} transactions from page {}", page_data.transactions.len(), page);
                            all_transactions.extend(page_data.transactions);
                        }
                        Err(e) => {
                            error!("Failed to get page {}: {}", page, e);
                            // Continue with the transactions we have so far
                            break;
                        }
                    }
                }

                info!("Total transactions fetched: {}", all_transactions.len());

                // Log transaction details at debug level
                for (i, transaction) in all_transactions.iter().enumerate() {
                    debug!(
                        "Transaction #{}: {}",
                        i + 1,
                        serde_json::to_string_pretty(&serde_json::to_value(transaction).unwrap()).unwrap()
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
