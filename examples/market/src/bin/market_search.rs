use ig_client::prelude::*;
use ig_client::utils::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();

    let client = Client::try_new()?;

    // Get the search term from command line arguments or use a default
    let search_term = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Daily Germany 40".to_string());

    info!("Searching for markets matching: '{}'", search_term);

    // Search for markets
    let result = client.search_markets(&search_term).await?;

    // Display using the Display trait - automatically formatted as a table!
    info!("\n{}", result);

    // Optionally save the results to JSON
    let json = serde_json::to_string_pretty(&result.markets)?;
    let filename = format!("Data/market_search_{}.json", search_term.replace(" ", "_"));
    std::fs::write(&filename, &json)?;
    info!("Results saved to '{}'", filename);

    Ok(())
}
