use ig_client::prelude::*;
use ig_client::utils::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();

    let client = Client::try_new()?;

    info!("=== Historical Prices Display Example ===\n");

    // Example 1: Get recent prices
    info!("📊 Example 1: Recent Prices (last 5 minutes)");
    let params = RecentPricesRequest {
        epic: "CS.D.EURUSD.CFD.IP",
        resolution: Some("MINUTE"),
        max_points: Some(5),
        page_size: None,
        page_number: None,
        from: None,
        to: None,
    };

    let recent_prices = client.get_recent_prices(&params).await?;
    info!("\n{}", recent_prices);

    // Example 2: Get historical prices by count (v2)
    info!("\n📊 Example 2: Historical Prices (last 10 hours)");
    let historical_prices = client
        .get_historical_prices_by_count_v2("CS.D.GBPUSD.CFD.IP", "HOUR", 10)
        .await?;
    info!("\n{}", historical_prices);

    info!("\n=== Examples completed! ===");
    info!("💡 The Display trait automatically formats prices in a table");
    info!("   with BID/ASK columns and summary information");

    Ok(())
}
