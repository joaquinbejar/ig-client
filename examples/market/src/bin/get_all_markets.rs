use ig_client::prelude::*;
use ig_client::utils::setup_logger;
use tracing::info;

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();

    let client = Client::try_new()?;

    info!("\n=== Getting All Markets from Account Categories ===");

    // Get all markets using the traversal method
    let all_markets = client.get_all_markets().await?;

    info!(
        "✅ Found {} unique markets across all account categories",
        all_markets.len()
    );

    // Show some sample markets
    info!("\n📊 Sample of markets found:");
    for (i, market) in all_markets.iter().take(10).enumerate() {
        info!("  {}. {} ({})", i + 1, market.instrument_name, market.epic);
    }

    if all_markets.len() > 10 {
        info!("  ... and {} more markets", all_markets.len() - 10);
    }

    // Group by instrument type
    let mut type_counts = std::collections::HashMap::new();
    for market in &all_markets {
        let type_str = format!("{:?}", market.instrument_type);
        *type_counts.entry(type_str).or_insert(0) += 1;
    }

    info!("\n📈 Markets by instrument type:");
    let mut types: Vec<_> = type_counts.iter().collect();
    types.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
    for (instrument_type, count) in types {
        info!("  {}: {}", instrument_type, count);
    }

    info!("\n=== Example completed successfully! ===");
    info!("💡 Each category is paged until a validated empty response");
    info!("   Any failed, repeated, inconsistent or safety-truncated traversal returns an error");

    Ok(())
}
