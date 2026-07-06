/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Update existing working order
//!
//! This example demonstrates how to update an existing working order.
//!
//! Run with:
//! ```bash
//! cargo run --bin working_order_update -- <DEAL_ID> <NEW_LEVEL>
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::order::OrderService;
use ig_client::error::AppError;
use ig_client::model::requests::UpdateWorkingOrderRequest;
use ig_client::presentation::order::{OrderType, TimeInForce};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting working order update example");

    let client = Client::try_new()?;

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: cargo run --bin working_order_update -- <DEAL_ID> <NEW_LEVEL>");
        eprintln!("Example: cargo run --bin working_order_update -- DIAAAABCDEFGH123 1.2500");
        std::process::exit(1);
    }

    let deal_id = &args[1];
    let new_level: f64 = args[2].parse().expect("Invalid level value");

    println!("\n=== Update Working Order ===\n");
    println!("Deal ID: {}", deal_id);
    println!("New Level: {}", new_level);

    let update =
        UpdateWorkingOrderRequest::new(new_level, OrderType::Limit, TimeInForce::GoodTillCancelled);

    match client.update_working_order(deal_id, &update).await {
        Ok(response) => {
            println!("\n--- Update Successful ---");
            println!("Deal Reference: {}", response.deal_reference);

            println!("\nGetting confirmation...");
            match client
                .get_order_confirmation_w_retry(&response.deal_reference, 5, 1000)
                .await
            {
                Ok(confirmation) => {
                    println!("Status: {:?}", confirmation.status);
                    println!("Deal ID: {:?}", confirmation.deal_id);
                    println!("Level: {:?}", confirmation.level);
                }
                Err(e) => {
                    println!("Could not get confirmation: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Error updating working order: {}", e);
            println!("\nMake sure you provide a valid deal ID for an existing working order.");
        }
    }

    Ok(())
}
