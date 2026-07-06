use ig_client::prelude::*;
use ig_client::utils::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();

    let client = Client::try_new()?;

    // Get EPIC from command line or use default
    let epic = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "CS.D.EURUSD.CFD.IP".to_string());

    info!("=== Recent Prices (API v3) ===");
    info!("EPIC: {}", epic);

    // Create request parameters
    let params = RecentPricesRequest {
        epic: &epic,
        resolution: Some("MINUTE"),
        max_points: Some(20),
        page_size: None,
        page_number: None,
        from: None,
        to: None,
    };

    // Get recent prices
    let prices = client.get_recent_prices(&params).await?;

    // Display using the Display trait - automatically formatted as a table!
    info!("\n{}", prices);

    // Optionally save to JSON
    let json = serde_json::to_string_pretty(&prices)?;
    let filename = format!("Data/recent_prices_{}.json", epic.replace(".", "_"));
    std::fs::write(&filename, &json)?;
    info!("Results saved to '{}'", filename);

    Ok(())
}
