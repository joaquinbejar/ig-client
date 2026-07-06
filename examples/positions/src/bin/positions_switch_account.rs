/// Example demonstrating how to get positions from one account, switch to another account,
/// and get positions from the new account
///
/// This example shows:
/// 1. Login to the API
/// 2. Get positions from the current account
/// 3. Switch to a different account (default: ZHH5N, or specify via command line)
/// 4. Get positions from the new account
///
/// To run this example:
/// ```bash
/// # With default account (ZHH5N)
/// cargo run --bin positions_switch_account
///
/// # With a specific account
/// cargo run --bin positions_switch_account YOUR_ACCOUNT_ID
/// ```
use ig_client::prelude::*;
use std::sync::Arc;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    // Set up logging
    setup_logger();

    info!("=== Positions Switch Account Example ===\n");

    // Create configuration with API v3 (OAuth)
    let config = Config {
        api_version: Some(3),
        ..Config::default()
    };

    info!("Configuration loaded:");
    info!("  Base URL: {}", config.rest_api.base_url);
    info!("  API Version: {:?}", config.api_version);

    // Create HTTP client and main client
    let http_client = Arc::new(HttpClient::new(config).await?);
    let client = Client::try_new()?;

    // Step 1: Login
    info!("\n1. Logging in...");
    let session = match http_client.get_session().await {
        Ok(s) => s,
        Err(e) => {
            error!("✗ Login failed: {:?}", e);
            return Err(format!("Login error: {:?}", e).into());
        }
    };

    info!("✓ Login successful");
    info!("  Account ID: {}", session.account_id);
    info!("  Uses OAuth: {}", session.is_oauth());

    // Step 2: Get positions from current account
    info!(
        "\n2. Getting positions from account: {}",
        session.account_id
    );
    match client.get_positions().await {
        Ok(positions) => {
            info!("✓ Successfully retrieved positions");
            info!("  Total positions: {}", positions.positions.len());

            if positions.positions.is_empty() {
                info!("  No open positions in this account");
            } else {
                info!("\n  Open positions:");
                for (i, position) in positions.positions.iter().enumerate() {
                    info!(
                        "  {}. {} - {} @ {} (Size: {})",
                        i + 1,
                        position.market.instrument_name,
                        position.market.epic,
                        position.position.direction,
                        position.position.size
                    );
                }
            }
        }
        Err(e) => {
            error!("✗ Failed to get positions: {:?}", e);
            return Err(format!("Get positions error (account 1): {:?}", e).into());
        }
    }

    // Step 3: Switch to target account (from command line or default to ZHH5N)
    let target_account = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ZHH5N".to_string());

    info!("\n3. Switching to account: {}", target_account);
    info!(
        "   (You can specify a different account as: cargo run --bin positions_switch_account <ACCOUNT_ID>)"
    );

    match http_client
        .switch_account(&target_account, Some(false))
        .await
    {
        Ok(()) => {
            info!("✓ Successfully switched to account: {}", target_account);
        }
        Err(e) => {
            error!("✗ Failed to switch account: {:?}", e);
            error!("  This could happen if:");
            error!("  - The account ID doesn't exist");
            error!("  - You don't have permission to access this account");
            error!("  - The account is not associated with your user");
            return Err(format!("Account switch error: {:?}", e).into());
        }
    }

    let new_session = http_client.get_session().await?;

    // Step 4: Get positions from new account
    info!(
        "\n4. Getting positions from account: {}",
        new_session.account_id
    );
    match client.get_positions().await {
        Ok(positions) => {
            info!("✓ Successfully retrieved positions");
            info!("  Total positions: {}", positions.positions.len());

            if positions.positions.is_empty() {
                info!("  No open positions in this account");
            } else {
                info!("\n  Open positions:");
                for (i, position) in positions.positions.iter().enumerate() {
                    info!(
                        "  {}. {} - {} @ {} (Size: {})",
                        i + 1,
                        position.market.instrument_name,
                        position.market.epic,
                        position.position.direction,
                        position.position.size
                    );
                }
            }
        }
        Err(e) => {
            error!("✗ Failed to get positions: {:?}", e);
            return Err(format!("Get positions error (account 2): {:?}", e).into());
        }
    }

    info!("\n=== Example Complete ===");
    Ok(())
}
