use ig_client::prelude::*;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    info!("=== Activity Debug Example ===");

    // Create HTTP client and config
    let config = Config::default();
    let http_client = HttpClient::new_lazy(config.clone())?;
    let session = http_client.get_session().await?;
    info!("Session started successfully");

    // Get activity with raw response handling
    info!("Fetching account activity...");
    let url = format!(
        "{}/{}",
        config.rest_api.base_url.trim_end_matches('/'),
        "history/activity?from=2025-03-01T00:00:00Z&to=2025-04-01T00:00:00Z&detailed=true"
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("X-IG-API-KEY", &config.credentials.api_key)
        .header("Content-Type", "application/json; charset=UTF-8")
        .header("Accept", "application/json; charset=UTF-8")
        .header("Version", "3")
        .header("CST", session.cst.as_ref().unwrap())
        .header(
            "X-SECURITY-TOKEN",
            session.x_security_token.as_ref().unwrap(),
        )
        .send()
        .await?;

    if response.status().is_success() {
        // Get the raw text response to see the actual structure
        let text = response.text().await?;
        info!("Raw API response: {}", text);
    } else {
        error!("Request failed with status: {}", response.status());
        let error_text = response.text().await?;
        error!("Error response: {}", error_text);
    }

    Ok(())
}
