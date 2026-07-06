/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Get historical costs and charges
//!
//! This example demonstrates how to retrieve historical costs and charges
//! for a date range.
//!
//! Run with:
//! ```bash
//! cargo run --bin costs_history
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::costs::CostsService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting costs history example");

    let client = Client::try_new()?;

    let from = "2024-01-01";
    let to = "2024-12-31";

    println!("\n=== Historical Costs and Charges ===\n");
    println!("Period: {} to {}", from, to);

    match client.get_costs_history(from, to).await {
        Ok(history) => {
            println!(
                "\n{:<15} {:<20} {:<25} {:>12} {:>8}",
                "DATE", "DEAL REF", "EPIC", "COST", "CCY"
            );
            println!("{}", "-".repeat(80));

            for cost in &history.costs {
                println!(
                    "{:<15} {:<20} {:<25} {:>12.2} {:>8}",
                    cost.date,
                    cost.deal_reference.as_deref().unwrap_or("-"),
                    cost.epic.as_deref().unwrap_or("-"),
                    cost.total_cost.unwrap_or(0.0),
                    cost.currency.as_deref().unwrap_or("-")
                );
            }

            println!("\nTotal cost entries: {}", history.costs.len());
        }
        Err(e) => {
            println!("Error getting costs history: {}", e);
            println!("\nNote: This endpoint may not be available in all regions.");
        }
    }

    Ok(())
}
