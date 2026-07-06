/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Account preferences GET/PUT
//!
//! This example demonstrates how to retrieve and update account preferences.
//!
//! Run with:
//! ```bash
//! cargo run --bin preferences_example
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::account::AccountService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting preferences example");

    let client = Client::try_new()?;

    println!("\n=== Account Preferences ===\n");

    println!("Getting current preferences...");
    let prefs = client.get_preferences().await?;
    println!(
        "Trailing Stops Enabled: {}",
        if prefs.trailing_stops_enabled {
            "Yes"
        } else {
            "No"
        }
    );

    let new_value = !prefs.trailing_stops_enabled;
    println!("\nToggling trailing stops to: {}", new_value);

    match client.update_preferences(new_value).await {
        Ok(()) => {
            println!("Preferences updated successfully!");

            println!("\nVerifying update...");
            let updated_prefs = client.get_preferences().await?;
            println!(
                "Trailing Stops Enabled: {}",
                if updated_prefs.trailing_stops_enabled {
                    "Yes"
                } else {
                    "No"
                }
            );

            println!("\nRestoring original value...");
            client
                .update_preferences(prefs.trailing_stops_enabled)
                .await?;
            println!("Original preferences restored.");
        }
        Err(e) => {
            println!("Failed to update preferences: {}", e);
        }
    }

    Ok(())
}
