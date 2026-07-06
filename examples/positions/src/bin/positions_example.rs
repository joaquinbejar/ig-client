use ig_client::prelude::*;
use ig_client::utils::finance::calculate_pnl;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    info!("=== IG Positions Example ===");

    // Create client
    let client = Client::try_new()?;

    // Get open positions
    info!("Fetching open positions...");
    let mut positions = client.get_positions().await?;

    if positions.positions.is_empty() {
        info!("No open positions currently");
    } else {
        info!("Open positions: {}", positions.positions.len());

        // Display positions
        for (i, position) in positions.positions.iter_mut().enumerate() {
            // Calculate P&L using the utility function
            position.pnl = calculate_pnl(position);

            // Log the position as pretty JSON
            info!(
                "Position #{}: {}",
                i + 1,
                serde_json::to_string_pretty(&serde_json::to_value(position).unwrap()).unwrap()
            );
        }
    }

    // Get working orders
    info!("Fetching working orders...");
    let working_orders = client.get_working_orders().await?;

    if working_orders.working_orders.is_empty() {
        info!("No working orders currently");
    } else {
        info!("Working orders: {}", working_orders.working_orders.len());

        // Display details of each working order as JSON
        for (i, order) in working_orders.working_orders.iter().enumerate() {
            // Log the working order as pretty JSON
            info!(
                "Working Order #{}: {}",
                i + 1,
                serde_json::to_string_pretty(&serde_json::to_value(order).unwrap()).unwrap()
            );
        }
    }

    Ok(())
}
