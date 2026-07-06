use ig_client::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    info!("=== IG Working Orders Example ===");

    // Create client
    let client = Client::try_new()?;
    let epic = "DO.D.OTCDDAX.107.IP";
    let epic_info = client.get_market_details(epic).await?;
    let currency: String = epic_info
        .instrument
        .currencies
        .unwrap()
        .first()
        .unwrap()
        .code
        .clone();

    // Step 1: Create a working order
    info!("\n--- Step 1: Creating a working order ---");
    let order_request = CreateWorkingOrderRequest::limit(
        epic.to_string(), // DAX index
        Direction::Buy,
        1.0,
        epic_info.snapshot.low.unwrap(),
        currency,
        epic_info.instrument.expiry,
    )
    .expires_tomorrow();

    info!("Creating working order: {:?}", order_request);
    let create_response = client.create_working_order(&order_request).await?;
    info!(
        "Working order created with deal reference: {}",
        create_response.deal_reference
    );

    // Wait a moment for the order to be processed
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Step 2: Get all working orders
    info!("\n--- Step 2: Fetching all working orders ---");
    let working_orders = client.get_working_orders().await?;

    if working_orders.working_orders.is_empty() {
        info!("No working orders currently");
    } else {
        info!(
            "Found {} working order(s)",
            working_orders.working_orders.len()
        );

        // Display details of each working order
        for (i, order) in working_orders.working_orders.iter().enumerate() {
            info!(
                "Working Order #{}: Deal ID: {}, Epic: {}, Direction: {:?}, Size: {}, Level: {}",
                i + 1,
                order.working_order_data.deal_id,
                order.working_order_data.epic,
                order.working_order_data.direction,
                order.working_order_data.order_size,
                order.working_order_data.order_level
            );
        }

        // Step 3: Delete the first working order
        if let Some(first_order) = working_orders.working_orders.first() {
            let deal_id = &first_order.working_order_data.deal_id;
            info!("\n--- Step 3: Deleting working order ---");
            info!("Deleting working order with deal ID: {}", deal_id);

            client.delete_working_order(deal_id).await?;
            info!("Working order deleted successfully");

            // Verify deletion
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let updated_orders = client.get_working_orders().await?;
            info!(
                "Remaining working orders: {}",
                updated_orders.working_orders.len()
            );
        }
    }

    Ok(())
}
