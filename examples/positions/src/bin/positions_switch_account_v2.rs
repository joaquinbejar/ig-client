/// Demonstrate API v2 account switching using one client for both position reads.
/// Run with `cargo run -p examples_positions --bin positions_switch_account_v2 -- YOUR_ACCOUNT_ID`.
use ig_client::prelude::*;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    info!("=== Positions Switch Account Example (API v2) ===\n");

    let target_account = std::env::args()
        .nth(1)
        .filter(|account| !account.trim().is_empty())
        .ok_or_else(|| {
            AppError::InvalidInput("usage: positions_switch_account_v2 <account-id>".to_owned())
        })?;

    // Create configuration with API v2 (CST authentication)
    let config = Config {
        api_version: Some(2),
        ..Config::default()
    };

    info!("Configuration loaded:");
    info!("  Base URL: {}", config.rest_api.base_url);
    info!("  API Version: {:?}", config.api_version);

    // Apply API v2 to the client that owns account selection and position reads.
    let initial_account = config.credentials.account_id.clone();
    let client = Client::with_config(config)?;
    info!("\n1. Client configured for API v2; the first request logs in automatically");

    // Step 2: Get positions from current account
    info!("\n2. Getting positions from account: {}", initial_account);
    match client.get_positions().await {
        Ok(positions) => {
            info!("✓ Successfully retrieved positions");
            info!("  Total positions: {}", positions.positions.len());

            if !positions.positions.is_empty() {
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
            return Err(e);
        }
    }

    // Step 4: Get positions from new account
    info!("\n4. Getting positions from account: {}", target_account);
    match client.get_positions().await {
        Ok(positions) => {
            info!("✓ Successfully retrieved positions");
            info!("  Total positions: {}", positions.positions.len());

            if !positions.positions.is_empty() {
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
