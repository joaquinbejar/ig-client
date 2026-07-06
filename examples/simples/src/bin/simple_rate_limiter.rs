use ig_client::application::client::Client;
use ig_client::application::interfaces::market::MarketService;
use ig_client::presentation::market::MarketDetails;
use ig_client::utils::setup_logger;
/// Example demonstrating rate limiting with automatic retry on rate limit exceeded
///
/// This example shows how the rate limiter automatically controls
/// the rate of API requests to comply with IG Markets limits.
///
/// Features:
/// - Automatic rate limiting before each request
/// - Automatic retry with 10 second delay if rate limit is exceeded
/// - Infinite retry until request succeeds
///
/// Configure rate limiting via environment variables:
/// - IG_RATE_LIMIT_MAX_REQUESTS (default: 3)
/// - IG_RATE_LIMIT_PERIOD_SECONDS (default: 8)
/// - IG_RATE_LIMIT_BURST_SIZE (default: 3)
///
/// Run with: cargo run --bin simple_rate_limiter
use std::time::Instant;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    // Initialize logging
    setup_logger();

    info!("Starting rate limiter example");
    info!("This example demonstrates automatic rate limiting");

    // Create client - authentication happens automatically
    info!("Creating client and authenticating...");
    let client = Client::try_new()?;
    let epic = "OP.D.OTCGC3.4050C.IP";
    let start = Instant::now();
    let num_requests = 20;

    for i in 1..=num_requests {
        let request_start = Instant::now();
        let _market: MarketDetails = client.get_market_details(epic).await?;
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
