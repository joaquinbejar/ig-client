/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Full watchlist CRUD operations
//!
//! This example demonstrates the complete lifecycle of a watchlist:
//! create, read, update (add/remove instruments), and delete.
//!
//! Run with:
//! ```bash
//! cargo run --bin watchlist_crud
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::watchlist::WatchlistService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting watchlist CRUD example");

    let client = Client::try_new()?;

    println!("\n=== Step 1: Create Watchlist ===");
    let result = client.create_watchlist("CRUD Test Watchlist", None).await?;
    let watchlist_id = result.watchlist_id.clone();
    println!("Created watchlist with ID: {}", watchlist_id);

    println!("\n=== Step 2: Add Instruments ===");
    let epics_to_add = [
        "IX.D.DAX.DAILY.IP",
        "CS.D.EURUSD.CFD.IP",
        "IX.D.FTSE.DAILY.IP",
    ];

    for epic in &epics_to_add {
        match client.add_to_watchlist(&watchlist_id, epic).await {
            Ok(status) => println!("Added {}: {}", epic, status.status),
            Err(e) => println!("Failed to add {}: {}", epic, e),
        }
    }

    println!("\n=== Step 3: View Watchlist ===");
    let watchlist = client.get_watchlist(&watchlist_id).await?;
    println!("Watchlist contains {} markets:", watchlist.markets.len());
    for market in &watchlist.markets {
        println!(
            "  - {} ({}) Bid: {:?} Offer: {:?}",
            market.instrument_name, market.epic, market.bid, market.offer
        );
    }

    println!("\n=== Step 4: Remove an Instrument ===");
    let epic_to_remove = "CS.D.EURUSD.CFD.IP";
    match client
        .remove_from_watchlist(&watchlist_id, epic_to_remove)
        .await
    {
        Ok(status) => println!("Removed {}: {}", epic_to_remove, status.status),
        Err(e) => println!("Failed to remove {}: {}", epic_to_remove, e),
    }

    println!("\n=== Step 5: View Updated Watchlist ===");
    let watchlist = client.get_watchlist(&watchlist_id).await?;
    println!(
        "Watchlist now contains {} markets:",
        watchlist.markets.len()
    );
    for market in &watchlist.markets {
        println!("  - {} ({})", market.instrument_name, market.epic);
    }

    println!("\n=== Step 6: Delete Watchlist ===");
    match client.delete_watchlist(&watchlist_id).await {
        Ok(status) => println!("Deleted watchlist: {}", status.status),
        Err(e) => println!("Failed to delete watchlist: {}", e),
    }

    println!("\n=== CRUD Operations Complete ===");

    Ok(())
}
