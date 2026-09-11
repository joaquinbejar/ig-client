use ig_client::prelude::*;
use tabled::{Table, Tabled};
use tracing::{error, info};

// Struct for displaying DBEntry data in a table format
#[derive(Tabled)]
struct DBEntryDisplay {
    #[tabled(rename = "Symbol")]
    symbol: String,
    #[tabled(rename = "Epic")]
    epic: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    instrument_type: String,
    #[tabled(rename = "Exchange")]
    exchange: String,
    #[tabled(rename = "Expiry")]
    expiry: String,
    #[tabled(rename = "Last Update")]
    last_update: String,
}

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    info!("=== IG Vec DB Entries Table Example ===");

    // Create client
    let client = Client::try_new()?;

    // Get vec DB entries
    info!("\n=== Fetching Vec DB Entries ===");
    match client.get_vec_db_entries().await {
        Ok(db_entries) => {
            if db_entries.is_empty() {
                info!("📊 No DB entries found");
            } else {
                info!("📊 Found {} DB entries", db_entries.len());

                // Convert DBEntry to DBEntryDisplay for table formatting
                let display_entries: Vec<DBEntryDisplay> = db_entries
                    .iter()
                    .map(|entry| DBEntryDisplay {
                        symbol: entry.symbol.clone(),
                        epic: truncate_display(&entry.epic, 25),
                        name: truncate_display(&entry.name, 30),
                        instrument_type: format!("{:?}", entry.instrument_type),
                        exchange: entry.exchange.clone(),
                        expiry: if entry.expiry.trim().is_empty() {
                            "N/A".to_string()
                        } else {
                            entry.expiry.clone()
                        },
                        last_update: entry.last_update.format("%Y-%m-%d %H:%M:%S").to_string(),
                    })
                    .collect();

                // Create and display the table
                let table = Table::new(display_entries)
                    .with(tabled::settings::Style::rounded())
                    .with(
                        tabled::settings::Modify::new(tabled::settings::object::Rows::first())
                            .with(tabled::settings::Alignment::center()),
                    )
                    .to_string();

                info!("\n🎯 Market DB Entries Table:\n{table}");

                // Display summary statistics
                info!("\n📈 Summary Statistics:");
                let unique_symbols: std::collections::HashSet<String> = db_entries
                    .iter()
                    .map(|e| e.symbol.clone())
                    .filter(|s| !s.is_empty())
                    .collect();
                info!("  Total entries: {}", db_entries.len());
                info!("  Unique symbols: {}", unique_symbols.len());

                let instrument_types: std::collections::HashMap<String, usize> = db_entries
                    .iter()
                    .fold(std::collections::HashMap::new(), |mut acc, entry| {
                        let type_str = format!("{:?}", entry.instrument_type);
                        *acc.entry(type_str).or_insert(0) += 1;
                        acc
                    });

                info!("  Instrument types:");
                for (instrument_type, count) in instrument_types {
                    info!("    {}: {}", instrument_type, count);
                }

                let with_expiry = db_entries
                    .iter()
                    .filter(|e| !e.expiry.trim().is_empty())
                    .count();
                let without_expiry = db_entries.len() - with_expiry;
                info!("  With listed expiry text: {}", with_expiry);
                info!("  Without listed expiry text: {}", without_expiry);
            }
        }
        Err(e) => {
            error!("❌ Failed to fetch vec DB entries: {:?}", e);
            error!("This could be due to:");
            error!("  - Session expired (try re-running)");
            error!("  - Network connectivity issues");
            error!("  - API rate limiting");
            error!("  - Account permissions");
            return Err(e);
        }
    }

    info!("\n=== Example completed successfully! ===");
    Ok(())
}

/// Truncate at Unicode scalar boundaries; reserve space for the ellipsis.
#[must_use]
fn truncate_display(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    // A display limit shorter than the ellipsis reserves no content characters.
    let visible = max_chars.saturating_sub(3);
    let mut shortened: String = value.chars().take(visible).collect();
    shortened.extend("...".chars().take(max_chars));
    shortened
}

#[cfg(test)]
mod tests {
    use super::truncate_display;

    #[test]
    fn test_display_truncation_handles_synthetic_unicode_and_short_limits() {
        assert_eq!(truncate_display("Synthetic市場価格ééé", 12), "Synthetic...");
        assert_eq!(truncate_display("ééééé", 4), "é...");
        assert_eq!(truncate_display("市場", 2), "市場");
        assert_eq!(truncate_display("市場価格", 2), "..");
        assert_eq!(truncate_display("市場価格", 0), "");
    }
}
