//! Export the complete flat catalogue; the historical binary name is retained.
//! No IG navigation tree is inferred from the category listing.

use ig_client::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    setup_logger();
    let client = Client::try_new()?;
    let markets = client.get_all_markets().await?;
    let json = serde_json::to_string_pretty(&markets)?;
    tokio::fs::create_dir_all("Data").await?;
    let filename = "Data/market_catalog.json";
    tokio::fs::write(filename, json).await?;
    info!(
        markets = markets.len(),
        filename, "complete flat market catalogue saved"
    );
    Ok(())
}
