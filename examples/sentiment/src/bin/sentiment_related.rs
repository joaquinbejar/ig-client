/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Get related market sentiments
//!
//! This example demonstrates how to retrieve client sentiment data
//! for markets related to a specific market.
//!
//! Run with:
//! ```bash
//! cargo run --bin sentiment_related
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::sentiment::SentimentService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting sentiment related markets example");

    let client = Client::try_new()?;

    let market_id = "EURUSD";

    println!("\n=== Related Market Sentiments for {} ===\n", market_id);

    let related = client.get_related_sentiment(market_id).await?;

    println!(
        "{:<20} {:>10} {:>10} {:>15}",
        "RELATED MARKET", "LONG %", "SHORT %", "CORRELATION"
    );
    println!("{}", "-".repeat(55));

    for sentiment in &related.client_sentiments {
        let correlation = if sentiment.long_position_percentage > 50.0 {
            "Similar"
        } else {
            "Inverse"
        };

        println!(
            "{:<20} {:>9.1}% {:>9.1}% {:>15}",
            sentiment.market_id,
            sentiment.long_position_percentage,
            sentiment.short_position_percentage,
            correlation
        );
    }

    println!(
        "\nFound {} related markets",
        related.client_sentiments.len()
    );

    Ok(())
}
