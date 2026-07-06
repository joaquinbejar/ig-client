/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Get single position by deal ID
//!
//! This example demonstrates how to retrieve details of a single position.
//!
//! Run with:
//! ```bash
//! cargo run --bin position_single -- <DEAL_ID>
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::order::OrderService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting position single example");

    let client = Client::try_new()?;

    let deal_id = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: cargo run --bin position_single -- <DEAL_ID>");
        eprintln!("Example: cargo run --bin position_single -- DIAAAABCDEFGH123");
        std::process::exit(1);
    });

    println!("\n=== Single Position Details ===\n");
    println!("Deal ID: {}", deal_id);

    match client.get_position(&deal_id).await {
        Ok(response) => {
            let pos_details = &response.position.position;
            let market = &response.market;

            println!("\n--- Position ---");
            println!("Deal ID: {}", pos_details.deal_id);
            println!("Direction: {:?}", pos_details.direction);
            println!("Size: {}", pos_details.size);
            println!("Level: {}", pos_details.level);
            println!("Currency: {}", pos_details.currency);
            println!("Created: {}", pos_details.created_date);

            if let Some(stop) = pos_details.stop_level {
                println!("Stop Level: {}", stop);
            }
            if let Some(limit) = pos_details.limit_level {
                println!("Limit Level: {}", limit);
            }

            println!("\n--- Market ---");
            println!("Instrument: {}", market.instrument_name);
            println!("Epic: {}", market.epic);
            if let Some(bid) = market.bid {
                println!("Bid: {}", bid);
            }
            if let Some(offer) = market.offer {
                println!("Offer: {}", offer);
            }
            println!("Status: {:?}", market.market_status);
        }
        Err(e) => {
            println!("Error getting position: {}", e);
            println!("\nMake sure you provide a valid deal ID for an open position.");
        }
    }

    Ok(())
}
