use ig_client::prelude::*;

use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    info!("=== IG Option Orders Example ===");

    // Create client
    let client = Client::try_new()?;

    let epic = "DO.D.OTCDDAX.68.IP"; // Example epic for testing
    let expiry = Some(
        chrono::Local::now()
            .format("%d-%b-%y")
            .to_string()
            .to_uppercase(),
    );
    let size = 1.25; // Size of the order
    let currency_code = Some("EUR".to_string()); // Example currency code
    let deal_reference = Some(get_id());
    info!("{:?}", deal_reference);
    let create_order = CreateOrderRequest::buy_option_to_market(
        epic.to_string(),
        size,
        expiry.clone(),
        deal_reference,
        currency_code.clone(),
    );

    // Create the position
    let create_result = client.create_order(&create_order).await;
    let deal_id: Option<String> = match create_result {
        Ok(response) => {
            info!(
                "Position created with deal reference: {}",
                response.deal_reference
            );

            // Get the order confirmation to obtain the deal ID
            let confirmation = client
                .get_order_confirmation(&response.deal_reference)
                .await
                .expect("Failed to get order confirmation");

            info!("Order confirmation received:");
            info!("  Deal ID: {:?}", confirmation.deal_id);
            info!("  Status: {:?}", confirmation.status);
            info!("  Reason: {:?}", confirmation.reason);

            // Ensure we have a deal ID
            match (
                confirmation.status == Status::Rejected,
                confirmation.deal_id,
            ) {
                (true, _) => {
                    error!("Order was rejected, cannot continue");
                    None
                }
                (false, Some(id)) => Some(id),
                (false, None) => {
                    error!("No deal ID received, cannot continue");
                    None
                }
            }
        }
        Err(e) => {
            error!("Failed to create position: {:?}", e);
            None
        }
    };

    if let Some(deal_id) = &deal_id {
        info!("Deal ID obtained: {}", deal_id);
    } else {
        error!("No valid deal ID obtained, exiting");
        return Ok(());
    }

    // sleep for a while to simulate some processing time
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    info!("Closing position with deal ID: {:?}", deal_id);
    let close_request = ClosePositionRequest::close_option_to_market_by_id(
        deal_id.unwrap(),
        Direction::Sell, // Assuming we are closing a buy position
        size,
    );
    // let close_request = ClosePositionRequest::close_option_to_market_by_epic(
    //     epic.to_string(),
    //     expiry.clone().unwrap(),
    //     Direction::Sell, // Assuming we are closing a buy position
    //     size,
    // );
    let close_result = client.close_position(&close_request).await;

    match close_result {
        Ok(close_response) => {
            info!(
                "Position closed with deal reference: {}",
                close_response.deal_reference
            );

            // Get the close confirmation
            let close_confirmation = client
                .get_order_confirmation(&close_response.deal_reference)
                .await
                .expect("Failed to get close confirmation");

            info!("Close confirmation received:");
            info!("  Deal ID: {:?}", close_confirmation.deal_id);
            info!("  Status: {:?}", close_confirmation.status);
            info!("  Reason: {:?}", close_confirmation.reason);

            match close_confirmation.status {
                Status::Rejected => {
                    error!("Close order was rejected: {:?}", close_confirmation.reason);
                }
                Status::Open => {
                    error!(
                        "Wrong side, we opened a new position instead of closing: {:?}",
                        close_confirmation.reason
                    );
                }
                Status::Closed => {
                    info!(
                        "Position closed successfully with deal ID: {:?}",
                        close_confirmation.deal_id
                    );
                }
                _ => {
                    warn!("Undefined situation: {:?}", close_confirmation.status);
                }
            }
        }
        Err(e) => {
            info!("Failed to close position: {:?}", e);
        }
    }

    Ok(())
}
