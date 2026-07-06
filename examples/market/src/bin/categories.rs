/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 16/12/25
******************************************************************************/

//! Example: Get all categories of instruments
//!
//! This example demonstrates how to retrieve all categories of instruments
//! enabled for the IG account using the `/categories` endpoint.
//!
//! Note: This endpoint may not be available for all account types or in demo mode.
//! If you receive a 500 error, try using a production account.

use ig_client::prelude::*;
use ig_client::utils::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();

    let client = Client::try_new()?;

    info!("Fetching all categories of instruments...");

    // Get all categories
    let result = client.get_categories().await?;

    info!("Found {} categories:", result.len());

    // Display each category
    for category in result.iter() {
        let tradeable_status = if category.non_tradeable {
            "non-tradeable"
        } else {
            "tradeable"
        };
        info!("  - {} ({})", category.code, tradeable_status);
    }

    // Save the results to JSON
    let json = serde_json::to_string_pretty(&result.categories)?;
    let filename = "Data/categories.json";
    std::fs::write(filename, &json)?;
    info!("Results saved to '{}'", filename);

    Ok(())
}
