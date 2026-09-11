use ig_client::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();

    let client = Client::try_new()?;

    // Get parameters from command line or use defaults
    let epic = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "CS.D.EURUSD.CFD.IP".to_string());

    let resolution = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "HOUR".to_string());

    let start_date = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "2025-01-01 00:00:00".to_string());

    let end_date = std::env::args()
        .nth(4)
        .unwrap_or_else(|| "2025-01-02 00:00:00".to_string());

    info!("=== Historical Prices by Date Range (API v2) ===");
    info!("EPIC: {}", epic);
    info!("Resolution: {}", resolution);
    info!("Start Date: {}", start_date);
    info!("End Date: {}", end_date);

    // Get historical prices
    let prices = client
        .get_historical_prices_by_date_range(&epic, &resolution, &start_date, &end_date)
        .await?;

    // Display using the Display trait - automatically formatted as a table!
    info!("\n{}", prices);

    // Optionally save to JSON
    let json = serde_json::to_string_pretty(&prices)?;
    let filename = format!(
        "Data/historical_prices_date_range_{}_{}.json",
        epic.replace(".", "_"),
        resolution
    );
    tokio::fs::create_dir_all("Data").await?;
    tokio::fs::write(&filename, &json).await?;
    info!("Results saved to '{}'", filename);

    Ok(())
}
