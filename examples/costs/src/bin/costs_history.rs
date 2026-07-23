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
                "\n{:<25} {:<8} {:<10} {:<30} {:<38}",
                "CREATED", "TYPE", "DIRECTION", "INSTRUMENT", "QUOTE REFERENCE"
            );
            println!("{}", "-".repeat(115));

            for entry in &history.costs_and_charges_history {
                println!(
                    "{:<25} {:<8} {:<10} {:<30} {:<38}",
                    entry.created_timestamp.as_deref().unwrap_or("-"),
                    entry.entry_type.as_deref().unwrap_or("-"),
                    entry.direction.as_deref().unwrap_or("-"),
                    entry.instrument_name.as_deref().unwrap_or("-"),
                    entry.indicative_quote_reference.as_deref().unwrap_or("-")
                );
            }

            println!(
                "\nTotal cost entries: {} ({} pages upstream)",
                history.costs_and_charges_history.len(),
                history.pagination.total_pages
            );
            println!(
                "Amounts are not inline: fetch each disclosure document via \
                 get_durable_medium(quote_reference)."
            );
        }
        Err(e) => {
            println!("Error getting costs history: {}", e);
            println!("\nNote: This endpoint may not be available in all regions.");
        }
    }

    Ok(())
}
