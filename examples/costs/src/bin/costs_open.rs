/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Get indicative costs for opening a position
//!
//! This example demonstrates how to retrieve indicative costs and charges
//! for opening a new position.
//!
//! Run with:
//! ```bash
//! cargo run --bin costs_open
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::costs::CostsService;
use ig_client::error::AppError;
use ig_client::model::requests::OpenCostsRequest;
use ig_client::presentation::order::Direction;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting costs open example");

    let client = Client::try_new()?;

    let request = OpenCostsRequest::new("CS.D.EURUSD.CFD.IP", Direction::Buy, 1.0, "EUR");

    println!("\n=== Indicative Costs for Opening Position ===\n");
    println!("Epic: {}", request.epic);
    println!("Direction: {:?}", request.direction);
    println!("Size: {}", request.size);
    println!("Currency: {}", request.currency_code);

    match client.get_indicative_costs_open(&request).await {
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

            if let Some(ref one_off) = charges.one_off_costs {
                println!("\nOne-off Costs:");
                if let Some(pct) = one_off.percentage {
                    println!("  Percentage: {:.2}%", pct);
                }
                if let Some(amt) = one_off.amount {
                    println!("  Amount: {:.2}", amt);
                }
            }

            if let Some(ref ongoing) = charges.ongoing_costs {
                println!("\nOngoing Costs:");
                if let Some(pct) = ongoing.percentage {
                    println!("  Percentage: {:.2}%", pct);
                }
                if let Some(amt) = ongoing.amount {
                    println!("  Amount: {:.2}", amt);
                }
            }
        }
        Err(e) => {
            println!("Error getting costs: {}", e);
            println!("\nNote: This endpoint may not be available in all regions.");
        }
    }

    Ok(())
}
