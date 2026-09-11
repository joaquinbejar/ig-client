//! Export a parsed summary of every instrument in a complete catalogue.

use ig_client::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    setup_logger();
    let client = Client::try_new()?;
    let markets = client.get_all_markets().await?;
    let rows: Vec<_> = markets
        .iter()
        .map(|market| {
            let parsed = parse_instrument_name(&market.instrument_name);
            serde_json::json!({
                "epic": market.epic,
                "instrument_name": market.instrument_name,
                "expiry": market.expiry,
                "expiry_timestamp": market.expiry_timestamp,
                "asset_name": normalize_text(&parsed.asset_name),
                "strike": parsed.strike,
                "option_type": parsed.option_type,
                "market": market
            })
        })
        .collect();
    let json = serde_json::to_string_pretty(&rows)?;
    tokio::fs::create_dir_all("Data").await?;
    let filename = "Data/market_table.json";
    tokio::fs::write(filename, json).await?;
    info!(
        markets = markets.len(),
        filename, "complete catalogue summary saved"
    );
    Ok(())
}
