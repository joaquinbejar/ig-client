/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 16/12/25
******************************************************************************/

//! Example: Get instruments for a specific category
//!
//! This example demonstrates how to retrieve all instruments for a given
//! category using the `/categories/{categoryId}/instruments` endpoint.
//! By default, it fetches instruments for the VANILLA_OPTIONS category.
//!
//! Note: This endpoint may not be available for all account types or in demo mode.
//! If you receive a 500 error, try using a production account.

use ig_client::prelude::*;
use ig_client::utils::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();

    let client = Client::default();

    // Get the category ID from command line arguments or use VANILLA_OPTIONS as default
    let category_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "VANILLA_OPTIONS".to_string());

    // Optional pagination parameters
    let page_number: Option<u32> = std::env::args().nth(2).and_then(|s| s.parse().ok());
    let page_size: Option<u32> = std::env::args().nth(3).and_then(|s| s.parse().ok());

    info!(
        "Fetching instruments for category: {} (page: {:?}, size: {:?})",
        category_id, page_number, page_size
    );

    // Get instruments for the category
    let result = client
        .get_category_instruments(&category_id, page_number, page_size)
        .await?;

    info!(
        "Found {} instruments in category '{}':",
        result.len(),
        category_id
    );

    // Display each instrument
    for instrument in result.iter() {
        let bid = instrument
            .bid
            .map(|b| format!("{:.2}", b))
            .unwrap_or_else(|| "-".to_string());
        let offer = instrument
            .offer
            .map(|o| format!("{:.2}", o))
            .unwrap_or_else(|| "-".to_string());

        info!(
            "  - {} | {} | Bid: {} | Offer: {} | Status: {:?}",
            instrument.epic, instrument.instrument_name, bid, offer, instrument.market_status
        );
    }

    // Display pagination metadata if available
    if let Some(metadata) = &result.metadata {
        info!(
            "Page {} with {} items per page",
            metadata.page_number, metadata.page_size
        );
    }

    // Save the results to JSON
    let json = serde_json::to_string_pretty(&result.instruments)?;
    let filename = format!("Data/category_{}_instruments.json", category_id);
    std::fs::write(&filename, &json)?;
    info!("Results saved to '{}'", filename);

    Ok(())
}
