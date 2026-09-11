/// Example demonstrating how to get positions from one account, switch to another account,
/// and get positions from the new account
///
/// This example shows:
/// 1. Create one client configured for API v3; login is automatic
/// 2. Get positions from the current account
/// 3. Switch that client to the account specified on the command line
/// 4. Get positions from the new account
///
/// To run this example:
/// ```bash
/// cargo run -p examples_positions --bin positions_switch_account -- YOUR_ACCOUNT_ID
/// ```
use ig_client::prelude::*;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    // Set up logging
    setup_logger();

    info!("=== Positions Switch Account Example ===\n");

    let target_account = std::env::args()
        .nth(1)
        .filter(|account| !account.trim().is_empty())
        .ok_or_else(|| {
            AppError::InvalidInput("usage: positions_switch_account <account-id>".to_owned())
        })?;

    // Create configuration with API v3 (OAuth)
    let config = Config {
        api_version: Some(3),
        ..Config::default()
    };

    info!("Configuration loaded:");
    info!("  Base URL: {}", config.rest_api.base_url);
    info!("  API Version: {:?}", config.api_version);

    // This client owns authentication, account selection, and both position reads.
    let initial_account = config.credentials.account_id.clone();
    let client = Client::with_config(config)?;
    info!("\n1. Client configured for OAuth; the first request logs in automatically");

    // Step 2: Get positions from current account
    info!("\n2. Getting positions from account: {}", initial_account);
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
            return Err(e);
        }
    }

    // Step 3: Switch the same client before the next position read.
    info!("\n3. Switching to account: {}", target_account);
    match client.switch_account(&target_account, Some(false)).await {
        Ok(()) => {
            info!("✓ Successfully switched to account: {}", target_account);
        }
        Err(e) => {
            error!("✗ Failed to switch account: {:?}", e);
            error!("  This could happen if:");
            error!("  - The account ID doesn't exist");
            error!("  - You don't have permission to access this account");
            error!("  - The account is not associated with your user");
            return Err(e);
        }
    }

    // Step 4: Get positions from new account
    info!("\n4. Getting positions from account: {}", target_account);
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
            return Err(e);
        }
    }

    info!("\n=== Example Complete ===");
    Ok(())
}
