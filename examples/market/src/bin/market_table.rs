use ig_client::prelude::*;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    // Configure logger
    setup_logger();

    info!("=== IG Market Table Example ===");

    // Create client
    let client = Client::try_new()?;

    // Create a directory for the output file if it doesn't exist
    std::fs::create_dir_all("Data")?;

    // Build the complete market hierarchy
    info!("Building market hierarchy...");
    let hierarchy = match build_market_hierarchy(&client, None, 0).await {
        Ok(h) => {
            info!(
                "Successfully built hierarchy with {} top-level nodes",
                h.len()
            );
            h
        }
        Err(e) => {
            error!("Error building complete hierarchy: {:?}", e);
            return Err(e);
        }
    };

    // Extract all markets from the hierarchy into a flat list
    let markets = extract_markets_from_hierarchy(&hierarchy);
    info!("Extracted {} markets from the hierarchy", markets.len());

    // Save the complete data to a JSON file
    let json_data = markets
        .iter()
        .map(|market| {
            let parsed_info = parse_instrument_name(&market.instrument_name);
            let normalized_asset_name = normalize_text(&parsed_info.asset_name);

            // Create a JSON object with all fields
            serde_json::json!({
                "epic": market.epic,
                "instrument_name": market.instrument_name,
                "expiry": market.expiry,
                "asset_name": normalized_asset_name,
                "strike": parsed_info.strike,
                "option_type": parsed_info.option_type
            })
        })
        .collect::<Vec<_>>();

    let json = match serde_json::to_string_pretty(&json_data) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize to JSON: {:?}", e);
            return Err(e.into());
        }
    };

    let filename = "Data/market_table.json";
    std::fs::write(filename, &json)?;
    info!("✅ Complete market data saved to '{}'", filename);
    Ok(())
}
