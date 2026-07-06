use ig_client::prelude::*;
use nanoid::nanoid;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    info!("=== IG Close Position Test ===");

    // Create client
    let client = Client::try_new()?;

    let epic = "DO.D.OTCDSTXE.GG.IP"; // Example epic for testing
    let expiry = Some(
        chrono::Local::now()
            .format("%d-%b-%y")
            .to_string()
            .to_uppercase(),
    );
    let size = 1.25; // Size of the order
    let currency_code = Some("EUR".to_string());

    // ===========================================
    // FLOW 1: Create order → Check positions → Close by ID → Verify closure
    // ===========================================
    info!("===== STARTING FLOW 1: Close by ID =====");

    // Step 1: Create and execute market order
    let deal_reference1 = Some(nanoid!(30, &nanoid::alphabet::SAFE));
    info!("Creating order with deal reference: {:?}", deal_reference1);

    let create_order1 = CreateOrderRequest::buy_option_to_market(
        epic.to_string(),
        size,
        expiry.clone(),
        deal_reference1,
        currency_code.clone(),
    );

    let deal_id1 = match client.create_order(&create_order1).await {
        Ok(response) => {
            info!(
                "Order created with deal reference: {}",
                response.deal_reference
            );

            let confirmation = client
                .get_order_confirmation(&response.deal_reference)
                .await?;

            info!(
                "Order confirmation - Status: {:?}, Deal ID: {:?}",
                confirmation.status, confirmation.deal_id
            );

            match (
                confirmation.status == Status::Rejected,
                confirmation.deal_id,
            ) {
                (true, _) => {
                    error!("Order was rejected, skipping Flow 1");
                    None
                }
                (false, Some(id)) => Some(id),
                (false, None) => {
                    error!("No deal ID received, skipping Flow 1");
                    None
                }
            }
        }
        Err(e) => {
            error!("Failed to create order: {:?}", e);
            None
        }
    };

    if let Some(deal_id) = &deal_id1 {
        // Step 2: Check positions to verify order became position
        info!("Checking positions to verify order became position...");
        let positions_before = client.get_positions().await?;
        info!(
            "Positions before close: {}",
            positions_before.positions.len()
        );

        let target_position = positions_before
            .positions
            .iter()
            .find(|p| p.position.deal_id == *deal_id);

        if let Some(pos) = target_position {
            info!(
                "Found target position with deal ID: {}",
                pos.position.deal_id
            );
            info!(
                "Position epic: {}, size: {}",
                pos.market.epic, pos.position.size
            );
        } else {
            warn!("Target position not found in positions list");
        }

        // Step 3: Close position by ID
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        info!("Closing position by ID: {}", deal_id);

        let close_request = ClosePositionRequest::close_option_to_market_by_id(
            deal_id.clone(),
            Direction::Sell,
            size,
        );

        match client.close_position(&close_request).await {
            Ok(close_response) => {
                info!(
                    "Close order created with deal reference: {}",
                    close_response.deal_reference
                );

                let close_confirmation = client
                    .get_order_confirmation(&close_response.deal_reference)
                    .await?;

                info!(
                    "Close confirmation - Status: {:?}",
                    close_confirmation.status
                );

                // Step 4: Verify position is gone
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                let positions_after = client.get_positions().await?;
                info!("Positions after close: {}", positions_after.positions.len());

                let position_exists = positions_after
                    .positions
                    .iter()
                    .any(|p| p.position.deal_id == *deal_id);

                if position_exists {
                    warn!("Position still exists after close attempt");
                } else {
                    info!("✅ Position successfully closed and removed");
                }
            }
            Err(e) => {
                error!("Failed to close position: {:?}", e);
            }
        }
    }

    // Wait between flows
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // ===========================================
    // FLOW 2: Create 2 orders same epic → Check positions → Close by epic → Verify closure
    // ===========================================
    info!("===== STARTING FLOW 2: Close by Epic =====");

    // Step 1: Create first order
    let deal_reference2a = Some(nanoid!(30, &nanoid::alphabet::SAFE));
    let size2a = 1.0;
    info!(
        "Creating first order with deal reference: {:?}",
        deal_reference2a
    );

    let create_order2a = CreateOrderRequest::buy_option_to_market(
        epic.to_string(),
        size2a,
        expiry.clone(),
        deal_reference2a,
        currency_code.clone(),
    );

    let _deal_id2a = match client.create_order(&create_order2a).await {
        Ok(response) => {
            let confirmation = client
                .get_order_confirmation(&response.deal_reference)
                .await?;
            info!(
                "First order - Status: {:?}, Deal ID: {:?}",
                confirmation.status, confirmation.deal_id
            );
            confirmation.deal_id
        }
        Err(e) => {
            error!("Failed to create first order: {:?}", e);
            None
        }
    };

    // Step 2: Create second order (same epic, different volume)
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let deal_reference2b = Some(nanoid!(30, &nanoid::alphabet::SAFE));
    let size2b = 1.5;
    info!(
        "Creating second order with deal reference: {:?}",
        deal_reference2b
    );

    let create_order2b = CreateOrderRequest::buy_option_to_market(
        epic.to_string(),
        size2b,
        expiry.clone(),
        deal_reference2b,
        currency_code.clone(),
    );

    let _deal_id2b = match client.create_order(&create_order2b).await {
        Ok(response) => {
            let confirmation = client
                .get_order_confirmation(&response.deal_reference)
                .await?;
            info!(
                "Second order - Status: {:?}, Deal ID: {:?}",
                confirmation.status, confirmation.deal_id
            );
            confirmation.deal_id
        }
        Err(e) => {
            error!("Failed to create second order: {:?}", e);
            None
        }
    };

    // Step 3: Check positions to verify orders became positions
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    info!("Checking positions to verify orders became positions...");
    let positions_before_flow2 = client.get_positions().await?;
    info!(
        "Total positions before close: {}",
        positions_before_flow2.positions.len()
    );

    let epic_positions: Vec<_> = positions_before_flow2
        .positions
        .iter()
        .filter(|p| p.market.epic == epic)
        .collect();
    info!("Positions for epic {}: {}", epic, epic_positions.len());

    for pos in &epic_positions {
        info!(
            "  Position - Deal ID: {}, Size: {}",
            pos.position.deal_id, pos.position.size
        );
    }

    // Step 4: Close positions by epic (if we have positions)
    if !epic_positions.is_empty() {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        info!("Closing positions by epic: {}", epic);

        let total_size = size2a + size2b;
        let close_request_epic = ClosePositionRequest::close_option_to_market_by_epic(
            epic.to_string(),
            expiry.clone().unwrap(),
            Direction::Sell,
            total_size,
        );

        match client.close_position(&close_request_epic).await {
            Ok(close_response) => {
                info!(
                    "Epic close order created with deal reference: {}",
                    close_response.deal_reference
                );

                let close_confirmation = client
                    .get_order_confirmation(&close_response.deal_reference)
                    .await?;

                info!(
                    "Epic close confirmation - Status: {:?}",
                    close_confirmation.status
                );

                // Step 5: Verify positions are gone
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                let positions_after_flow2 = client.get_positions().await?;
                info!(
                    "Total positions after epic close: {}",
                    positions_after_flow2.positions.len()
                );

                let remaining_epic_positions: Vec<_> = positions_after_flow2
                    .positions
                    .iter()
                    .filter(|p| p.market.epic == epic)
                    .collect();

                if remaining_epic_positions.is_empty() {
                    info!("✅ All positions for epic {} successfully closed", epic);
                } else {
                    warn!(
                        "Some positions for epic {} still exist: {}",
                        epic,
                        remaining_epic_positions.len()
                    );
                    for pos in remaining_epic_positions {
                        info!(
                            "  Remaining position - Deal ID: {}, Size: {}",
                            pos.position.deal_id, pos.position.size
                        );
                    }
                }
            }
            Err(e) => {
                error!("Failed to close positions by epic: {:?}", e);
            }
        }
    } else {
        warn!("No positions found for epic {} to close", epic);
    }

    info!("===== TEST COMPLETED =====");
    Ok(())
}
