/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Operations/Application info
//!
//! This example demonstrates how to retrieve API application details.
//!
//! Run with:
//! ```bash
//! cargo run --bin operations_example
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::operations::OperationsService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting operations example");

    let client = Client::try_new()?;

    println!("\n=== API Application Details ===\n");

    match client.get_client_apps().await {
        Ok(app) => {
            println!("API Key: <redacted>");
            println!("Name: {}", app.name.as_deref().unwrap_or("N/A"));
            println!("Status: {}", app.status);
            if let Some(overall) = app.allowance_account_overall {
                println!("Overall Allowance: {}", overall);
            }
            if let Some(trading) = app.allowance_account_trading {
                println!("Trading Allowance: {}", trading);
            }
            if let Some(subs) = app.concurrent_subscriptions_limit {
                println!("Concurrent Subscriptions: {}", subs);
            }
            if let Some(ref created) = app.created_date {
                println!("Created: {}", created);
            }
            println!("{}", "-".repeat(50));
        }
        Err(e) => {
            println!("Error getting application details: {}", e);
            println!("\nNote: This endpoint may have a different response format");
            println!("depending on your API key configuration.");
        }
    }

    Ok(())
}
