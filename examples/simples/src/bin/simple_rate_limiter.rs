//! Demonstrate request pacing and finite retry with exponential backoff and jitter.
//!
//! Configure pacing with IG_RATE_LIMIT_MAX_REQUESTS (default: 4),
//! IG_RATE_LIMIT_PERIOD_SECONDS (default: 12), and IG_RATE_LIMIT_BURST_SIZE (default: 3).
//! MAX_RETRY_COUNT and RETRY_DELAY_SECS control the retry cap and base delay.
//! Permanent failures and exhausted retry budgets propagate to the caller.
//!
//! Run with an accessible EPIC from the catalogue:
//! `cargo run -p examples_simples --bin simple_rate_limiter -- EPIC`.

use ig_client::application::client::Client;
use ig_client::application::interfaces::market::MarketService;
use ig_client::error::AppError;
use ig_client::presentation::market::MarketDetails;
use ig_client::utils::setup_logger;
use std::time::Instant;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    // Initialize logging
    setup_logger();

    let mut args = std::env::args().skip(1);
    let epic = args
        .next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::InvalidInput("Usage: simple_rate_limiter EPIC".to_string()))?;
    if args.next().is_some() {
        return Err(AppError::InvalidInput(
            "Usage: simple_rate_limiter EPIC".to_string(),
        ));
    }

    info!("Starting rate limiter example");
    info!("This example demonstrates automatic rate limiting");

    // Create client - authentication happens automatically
    info!("Creating client and authenticating...");
    let client = Client::try_new()?;
    let start = Instant::now();
    let num_requests = 20;

    for i in 1..=num_requests {
        let request_start = Instant::now();
        let _market: MarketDetails = client.get_market_details(&epic).await?;
        let request_duration = request_start.elapsed();

        info!(
            "Request {}/{} completed in {:.2}ms (total elapsed: {:.2}s)",
            i,
            num_requests,
            request_duration.as_secs_f64() * 1000.0,
            start.elapsed().as_secs_f64()
        );
    }

    let total_duration = start.elapsed();
    let avg_rate = num_requests as f64 / total_duration.as_secs_f64();

    info!("\n=== Rate Limiter Statistics ===");
    info!("Total requests: {}", num_requests);
    info!("Total time: {:.2}s", total_duration.as_secs_f64());
    info!("Average rate: {:.2} requests/second", avg_rate);
    info!("Average rate: {:.2} requests/minute", avg_rate * 60.0);

    Ok(())
}
