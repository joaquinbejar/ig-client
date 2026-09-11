//! Inspect a decoded activity response through the supported client API.
//! Uses the last 30 days and supports both API v2 and OAuth v3 authentication.

use chrono::{Duration, Utc};
use ig_client::prelude::*;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    info!("=== Activity Debug Example ===");

    let client = Client::try_new()?;
    let to = Utc::now();
    let from = to.checked_sub_signed(Duration::days(30)).ok_or_else(|| {
        AppError::InvalidInput("activity lookback exceeds the supported date range".to_owned())
    })?;
    let from = from.format("%Y-%m-%dT%H:%M:%S").to_string();
    let to = to.format("%Y-%m-%dT%H:%M:%S").to_string();
    info!(%from, %to, "fetching a detailed account activity page");

    // The shared client supplies authentication headers, pacing, and refresh.
    // Diagnostics contain the decoded activity page, never a session or headers.
    let response = client.get_activity_with_details(&from, &to).await?;
    info!(
        activities = response.activities.len(),
        "activity page received"
    );
    debug!(response = %serde_json::to_string_pretty(&response)?, "decoded activity response");

    Ok(())
}
