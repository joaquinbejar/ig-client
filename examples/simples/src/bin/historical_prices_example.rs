use ig_client::application::client::Client;
use ig_client::application::interfaces::market::MarketService;
use ig_client::error::AppError;
use ig_client::utils::setup_logger;
use tracing::{error, info, warn};

/// List of EPICs to fetch historical prices for.
/// Adjust these to instruments available in your account.
const EPICS: &[&str] = &[
    "CS.D.EURUSD.TODAY.IP",
    "CS.D.GBPUSD.TODAY.IP",
    "IX.D.DAX.IFMM.IP",
];

/// Resolution for historical price data.
/// Options: SECOND, MINUTE, MINUTE_2, MINUTE_3, MINUTE_5, MINUTE_10,
///          MINUTE_15, MINUTE_30, HOUR, HOUR_2, HOUR_3, HOUR_4, DAY, WEEK, MONTH
const RESOLUTION: &str = "HOUR";

/// Number of data points to request per EPIC (API v2).
const NUM_POINTS: u32 = 10;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    setup_logger();

    info!("=== Historical Prices Example ===");
    info!(
        "Fetching {} data points ({}) for {} EPICs",
        NUM_POINTS,
        RESOLUTION,
        EPICS.len()
    );

    let client = Client::default();
    info!("Client created and authenticated");

    for epic in EPICS {
        info!("--- Requesting prices for: {} ---", epic);

        match client
            .get_historical_prices_by_count_v2(epic, RESOLUTION, NUM_POINTS)
            .await
        {
            Ok(response) => {
                info!(
                    "Received {} data points for {}",
                    response.prices.len(),
                    epic
                );

                // Log allowance information — this is the key metric to watch
                if let Some(allowance) = &response.allowance {
                    info!(
                        "Allowance: {}/{} remaining, resets in {} seconds",
                        allowance.remaining_allowance,
                        allowance.total_allowance,
                        allowance.allowance_expiry
                    );

                    if allowance.remaining_allowance <= 0 {
                        warn!(
                            "Allowance exhausted! Resets in {} seconds ({:.1} hours)",
                            allowance.allowance_expiry,
                            allowance.allowance_expiry as f64 / 3600.0
                        );
                        break;
                    }

                    if allowance.remaining_allowance < 100 {
                        warn!(
                            "Allowance running low: {} remaining",
                            allowance.remaining_allowance
                        );
                    }
                }

                // Print last price as a quick sanity check
                if let Some(last) = response.prices.last() {
                    info!(
                        "  Last candle: {} | close bid={:?} ask={:?}",
                        last.snapshot_time, last.close_price.bid, last.close_price.ask
                    );
                }
            }
            Err(AppError::HistoricalDataAllowanceExceeded { allowance_expiry }) => {
                error!(
                    "Historical data allowance exceeded for {}! Weekly quota exhausted. Resets in {} seconds ({:.1} hours). Aborting.",
                    epic,
                    allowance_expiry,
                    allowance_expiry as f64 / 3600.0
                );
                return Err(AppError::HistoricalDataAllowanceExceeded { allowance_expiry });
            }
            Err(e) => {
                error!("Failed to fetch prices for {}: {}", epic, e);
            }
        }
    }

    info!("=== Done ===");
    Ok(())
}
