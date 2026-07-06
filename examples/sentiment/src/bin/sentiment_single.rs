/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Get client sentiment for a single market
//!
//! This example demonstrates how to retrieve client sentiment data
//! for a specific market, showing the percentage of clients long vs short.
//!
//! Run with:
//! ```bash
//! cargo run --bin sentiment_single
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::sentiment::SentimentService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting sentiment single market example");

    let client = Client::try_new()?;

    let market_id = "EURUSD";

    println!("\n=== Client Sentiment for {} ===\n", market_id);

    let sentiment = client.get_client_sentiment_by_market(market_id).await?;

    println!("Market ID: {}", sentiment.market_id);
    println!("Long Position:  {:.1}%", sentiment.long_position_percentage);
    println!(
        "Short Position: {:.1}%",
        sentiment.short_position_percentage
    );

    let bar_length = 50;
    let long_bars = (sentiment.long_position_percentage / 100.0 * bar_length as f64) as usize;
    let short_bars = bar_length - long_bars;

    println!("\nSentiment Bar:");
    println!("[{}{}]", "█".repeat(long_bars), "░".repeat(short_bars));
    println!(
        " LONG {:>5.1}%{:>30}SHORT {:>5.1}%",
        sentiment.long_position_percentage, "", sentiment.short_position_percentage
    );

    Ok(())
}
