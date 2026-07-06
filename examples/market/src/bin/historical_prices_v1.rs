use ig_client::prelude::*;
use ig_client::utils::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();

    let client = Client::default();

    // Get EPIC from command line or use default
    let epic = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "CS.D.EURUSD.CFD.IP".to_string());

    let resolution = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "HOUR".to_string());

    let num_points: u32 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    info!("=== Historical Prices by Count (API v1) ===");
    info!("EPIC: {}", epic);
    info!("Resolution: {}", resolution);
    info!("Number of points: {}", num_points);

    // Get historical prices
    let prices = client
        .get_historical_prices_by_count_v1(&epic, &resolution, num_points)
        .await?;

    // Display using the Display trait - automatically formatted as a table!
    info!("\n{}", prices);

    // Optionally save to JSON
    let json = serde_json::to_string_pretty(&prices)?;
    let filename = format!(
        "Data/historical_prices_v1_{}_{}.json",
        epic.replace(".", "_"),
        resolution
    );
    std::fs::write(&filename, &json)?;
    info!("Results saved to '{}'", filename);

    Ok(())
}
