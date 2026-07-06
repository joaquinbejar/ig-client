/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Example: Get client sentiment for multiple markets
//!
//! This example demonstrates how to retrieve client sentiment data
//! for multiple markets in a single request.
//!
//! Run with:
//! ```bash
//! cargo run --bin sentiment_multiple
//! ```

use ig_client::application::client::Client;
use ig_client::application::interfaces::sentiment::SentimentService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    info!("Starting sentiment multiple markets example");

    let client = Client::try_new()?;

    let market_ids = vec![
        "EURUSD".to_string(),
        "GBPUSD".to_string(),
        "USDJPY".to_string(),
        "GOLD".to_string(),
        "OIL_CRUDE".to_string(),
    ];

    println!("\n=== Client Sentiment for Multiple Markets ===\n");

    let sentiments = client.get_client_sentiment(&market_ids).await?;

    println!(
        "{:<15} {:>10} {:>10} {:>20}",
        "MARKET", "LONG %", "SHORT %", "SENTIMENT"
    );
    println!("{}", "-".repeat(55));

    for sentiment in &sentiments.client_sentiments {
        let bias = if sentiment.long_position_percentage > 60.0 {
            "BULLISH"
        } else if sentiment.short_position_percentage > 60.0 {
            "BEARISH"
        } else {
            "NEUTRAL"
        };

        println!(
            "{:<15} {:>9.1}% {:>9.1}% {:>20}",
            sentiment.market_id,
            sentiment.long_position_percentage,
            sentiment.short_position_percentage,
            bias
        );
    }

    println!(
        "\nTotal markets analyzed: {}",
        sentiments.client_sentiments.len()
    );

    Ok(())
}
