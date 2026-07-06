use ig_client::prelude::*;
use ig_client::utils::setup_logger;
use tracing::{debug, error, info};

// Constants for API request handling
const BATCH_SIZE: usize = 25; // Number of EPICs to process before saving results

#[tokio::main]
async fn main() -> IgResult<()> {
    // Set up logging
    setup_logger();
    let client = Client::try_new()?;

    // Get the EPICs from command line arguments or use the default range
    let epics_arg = std::env::args().nth(1);

    let epics = match epics_arg {
        Some(arg) => {
            // Parse comma-separated list of EPICs
            arg.split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<String>>()
        }
        None => {
            // Generate the default range of EPICs
            info!(
                "No EPICs provided, using default range from DO.D.OTCDDAX.1.IP to DO.D.OTCDDAX.196.IP"
            );
            (1..=196)
                .map(|i| format!("DO.D.OTCDDAX.{}.IP", i))
                .collect::<Vec<String>>()
        }
    };

    info!("Fetching market details for {} EPICs", epics.len());

    // Create a vector to store all the market details
    let mut market_details_vec = Vec::new();

    // Process EPICs in batches to use the API more efficiently
    let mut processed_count = 0;
    let total_epics = epics.len();

    // Process EPICs in batches of BATCH_SIZE
    for chunk_start in (0..epics.len()).step_by(BATCH_SIZE) {
        let chunk_end = std::cmp::min(chunk_start + BATCH_SIZE, epics.len());
        let epics_chunk = &epics[chunk_start..chunk_end];

        info!(
            "Fetching market details for batch {}/{} (EPICs {}-{} of {})",
            (chunk_start / BATCH_SIZE) + 1,
            total_epics.div_ceil(BATCH_SIZE),
            chunk_start + 1,
            chunk_end,
            total_epics
        );

        // Log the EPICs being processed in this batch
        debug!("EPICs in this batch: {}", epics_chunk.join(", "));

        match client.get_multiple_market_details(epics_chunk).await {
            Ok(response) => {
                // Match each result with its corresponding EPIC
                for (i, details) in response.iter().enumerate() {
                    let epic = &epics_chunk[i];
                    debug!("✅ Successfully fetched details for {}", epic);
                    market_details_vec.push((epic.clone(), details.clone()));
                }

                processed_count += response.len();
                info!(
                    "✅ Successfully processed batch of {} EPICs ({}/{})",
                    response.len(),
                    processed_count,
                    total_epics
                );
            }
            Err(e) => {
                error!("❌ Failed to fetch details for batch: {:?}", e);

                // Fall back to processing EPICs individually in case of batch failure
                info!("Falling back to processing EPICs individually...");

                for epic in epics_chunk {
                    info!("Fetching market details for {} individually", epic);

                    match client.get_market_details(epic).await {
                        Ok(details) => {
                            debug!("✅ Successfully fetched details for {}", epic);
                            market_details_vec.push((epic.clone(), details));
                            processed_count += 1;
                        }
                        Err(e) => {
                            error!("❌ Failed to fetch details for {}: {:?}", epic, e);
                        }
                    }
                }
            }
        }
    }

    // Create MultipleMarketDetailsResponse from collected details
    let market_details_only: Vec<MarketDetails> = market_details_vec
        .iter()
        .map(|(_, details)| details.clone())
        .collect();

    let response = MultipleMarketDetailsResponse {
        market_details: market_details_only,
    };

    // Display the results using the Display trait
    info!("\n{}", response);

    // Save the final results to JSON
    let json_data = market_details_vec.iter()
        .map(|(epic, details)| {
            serde_json::json!({
                "epic": epic,
                "instrument_name": details.instrument.name,
                "expiry": details.instrument.expiry,
                "last_dealing_date": details.instrument.expiry_details.as_ref().map(|ed| ed.last_dealing_date.clone()),
                "bid": details.snapshot.bid,
                "offer": details.snapshot.offer,
                "high": details.snapshot.high,
                "low": details.snapshot.low,
                "update_time": details.snapshot.update_time
            })
        })
        .collect::<Vec<_>>();

    let json = serde_json::to_string_pretty(&json_data)?;

    // Save the results to a file
    let filename = "Data/market_details.json".to_string();
    std::fs::write(&filename, &json)?;
    info!("Results saved to '{}'", filename);
    info!(
        "Successfully processed {} out of {} EPICs",
        market_details_vec.len(),
        epics.len()
    );

    Ok(())
}
