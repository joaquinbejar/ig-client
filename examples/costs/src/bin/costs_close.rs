/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Get indicative costs for closing a position
//!
//! This example demonstrates how to retrieve indicative costs and charges
//! for closing an existing position.
//!
//! Run with:
//! ```bash
//! cargo run --bin costs_close
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::costs::CostsService;
use ig_client::error::AppError;
use ig_client::model::requests::CloseCostsRequest;
use ig_client::presentation::order::Direction;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting costs close example");

    let client = Client::try_new()?;

    let deal_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "DEAL_ID_HERE".to_string());

    let request = CloseCostsRequest::new(&deal_id, Direction::Sell, 1.0);

    println!("\n=== Indicative Costs for Closing Position ===\n");
    println!("Deal ID: {}", request.deal_id);
    println!("Direction: {:?}", request.direction);
    println!("Size: {}", request.size);

    match client.get_indicative_costs_close(&request).await {
        Ok(costs) => {
            println!("\n--- Costs Breakdown ---");
            println!("Quote Reference: {}", costs.indicative_quote_reference);

            let charges = &costs.costs_and_charges;
            if let Some(total) = charges.total_cost_percentage {
                println!("Total Cost: {:.2}%", total);
            }
            if let Some(amount) = charges.total_cost_amount {
                println!(
                    "Total Amount: {:.2} {}",
                    amount,
                    charges.currency.as_deref().unwrap_or("N/A")
                );
            }
        }
        Err(e) => {
            println!("Error getting costs: {}", e);
            println!("\nNote: Provide a valid deal ID as argument:");
            println!("  cargo run --bin costs_close -- <DEAL_ID>");
        }
    }

    Ok(())
}
