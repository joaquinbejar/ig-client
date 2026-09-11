//! Store catalogue instruments accepted by the storage adapter's EPIC-format filter.
//! The name mapping labels symbols; unmatched names are stored as UNKNOWN.
//! Run `market_hierarchy` first, or pass another flat catalogue JSON path.
//! This example writes to PostgreSQL and does not enumerate or repair history.

use ig_client::prelude::*;
use std::collections::HashMap;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    // Set up logging
    setup_logger();

    info!("Starting filtered market storage example...");

    // `market_hierarchy` now exports the complete flat catalogue to this path.
    let json_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Data/market_catalog.json".to_owned());
    info!(path = %json_path, "loading flat market catalogue");
    let json_content = tokio::fs::read_to_string(&json_path).await?;
    let markets: Vec<MarketData> = serde_json::from_str(&json_content)?;
    // Leaf nodes adapt the catalogue to the existing storage API. These are
    // instrument records, not an inferred IG category tree.
    let nodes: Vec<MarketNode> = markets
        .into_iter()
        .map(|market| MarketNode {
            id: market.epic.clone(),
            name: market.instrument_name.clone(),
            children: Vec::new(),
            markets: vec![market],
        })
        .collect();
    info!(
        instruments = nodes.len(),
        "flat catalogue loaded for filtered storage"
    );

    // Create the symbol mapping HashMap as specified
    let symbol_map: HashMap<&str, &str> = HashMap::from([
        ("germany 40", "GER40"),
        ("us 500", "US500"),
        ("wall street", "US30"),
        ("us tech 100", "USTECH"),
        ("france 40", "FRA40"),
        ("eu stocks 50", "EU50"),
        ("japan 225", "NI225"),
        ("uk 100", "UK100"),
        ("ftse", "UK100"),
        ("us crude", "OIL"),
        ("oil", "OIL"),
        ("natural gas", "NATGAS"),
        ("spot gold", "GOLD"),
        ("gold", "GOLD"),
        ("spot silver", "SILVER"),
        ("silver", "SILVER"),
        ("bitcoin", "BITCOIN"),
        ("ether", "ETHEREUM"),
        ("volatility index", "VIX"),
        ("pago mediante tarjeta", "DEPOSIT"),
        ("dinero depositado en su cuenta", "DEPOSIT"),
        ("funds transfer", "DEPOSIT"),
        ("Australia 200", "AUS200"),
        ("Japón 225", "NI225"),
        ("AUDUSD", "AUDUSD"),
        ("EURUSD", "EURUSD"),
        ("GBPUSD", "GBPUSD"),
        ("USDCAD", "USDCAD"),
        ("EURGBP", "EURGBP"),
        ("GBPJPY", "GBPJPY"),
        ("USDJPY", "USDJPY"),
        ("EURJPY", "EURJPY"),
        ("USDCHF", "USDCHF"),
    ]);

    info!("Created symbol mapping with {} entries", symbol_map.len());

    // Set up database connection
    let db_config = create_database_config_from_env()
        .map_err(|e| format!("Failed to create database config: {}", e))?;

    info!("Database config created successfully");

    let pool = create_connection_pool(&db_config)
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    info!("Successfully connected to PostgreSQL database");

    // Create database service
    let db_service = MarketDatabaseService::new(pool, "IG".to_string());

    info!("Database service created successfully");

    // Define the custom table name
    let table_name = "filtered_market_instruments";

    // Store filtered market nodes using the new method
    info!("Storing filtered market nodes to table '{}'...", table_name);

    db_service
        .store_filtered_market_nodes(&nodes, &symbol_map, table_name)
        .await
        .map_err(|e| format!("Failed to store filtered market nodes: {}", e))?;

    info!("✅ Successfully stored filtered market instruments!");
    info!("Filtered market storage example completed successfully!");

    Ok(())
}
