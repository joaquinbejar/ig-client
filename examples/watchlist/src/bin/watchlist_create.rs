/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Create a new watchlist
//!
//! This example demonstrates how to create a new watchlist with optional instruments.
//!
//! Run with:
//! ```bash
//! cargo run --bin watchlist_create
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::watchlist::WatchlistService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting watchlist create example");

    let client = Client::try_new()?;

    let watchlist_name = "My API Watchlist";
    let initial_epics = vec![
        "IX.D.DAX.DAILY.IP".to_string(),
        "CS.D.EURUSD.CFD.IP".to_string(),
    ];

    println!("\nCreating watchlist: {}", watchlist_name);
    println!("With instruments: {:?}", initial_epics);

    let result = client
        .create_watchlist(watchlist_name, Some(&initial_epics))
        .await?;

    println!("\n=== Watchlist Created ===");
    println!("Watchlist ID: {}", result.watchlist_id);
    println!("Status: {}", result.status);

    let watchlist_details = client.get_watchlist(&result.watchlist_id).await?;
    println!("\nMarkets in watchlist:");
    for market in &watchlist_details.markets {
        println!("  - {} ({})", market.instrument_name, market.epic);
    }

    Ok(())
}
