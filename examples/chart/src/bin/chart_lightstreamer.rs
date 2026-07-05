use ig_client::application::interfaces::listener::Listener;
use ig_client::error::AppError;
use ig_client::prelude::*;
use lightstreamer_rs::client::{LightstreamerClient, Transport};
use lightstreamer_rs::subscription::{Snapshot, Subscription, SubscriptionMode};
use lightstreamer_rs::utils::setup_signal_hook;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tracing::{debug, error, info, warn};

const MAX_CONNECTION_ATTEMPTS: u64 = 3;

fn callback(update: &ChartData) -> Result<(), AppError> {
    let item = serde_json::to_string_pretty(&update)?;
    info!("ChartData: {}", item);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    // Create a subscription for a market
    let epic = "CHART:OP.D.OTCDAX1.021100P.IP:TICK".to_string();
    let mut subscription = Subscription::new(
        SubscriptionMode::Distinct,
        Some(vec![epic]),
        Some(vec![
            "BID".to_string(),
            "OFR".to_string(),
            "LTP".to_string(),
            "LTV".to_string(),
            "TTV".to_string(),
            "UTM".to_string(),
            "DAY_OPEN_MID".to_string(),
            "DAY_NET_CHG_MID".to_string(),
            "DAY_PERC_CHG_MID".to_string(),
            "DAY_HIGH".to_string(),
            "DAY_LOW".to_string(),
        ]),
    )?;

    let listener = Listener::new(callback);
    subscription.set_data_adapter(None)?;
    subscription.set_requested_snapshot(Some(Snapshot::Yes))?;
    subscription.add_listener(Box::new(listener));

    let client = Client::default();
    let ws_info = client.ws_info().await?;
    let password = ws_info.get_ws_password();

    debug!(
        server = %ws_info.server,
        account_id = %ws_info.account_id,
        "WebSocket info obtained"
    );
    // Create a new Lightstreamer client instance and wrap it in an Arc<Mutex<>> so it can be shared across threads.
    let client = Arc::new(Mutex::new(LightstreamerClient::new(
        Some(ws_info.server.as_str()),
        None,
        Some(&ws_info.account_id),
        Some(&password),
    )?));

    //
    // Add the subscription to the client.
    //
    {
        let mut client = client.lock().await;
        LightstreamerClient::subscribe(client.subscription_sender.clone(), subscription).await?;
        client
            .connection_options
            .set_forced_transport(Some(Transport::WsStreaming));
    }

    // Create a new Notify instance to send a shutdown signal to the signal handler thread.
    let shutdown_signal = Arc::new(Notify::new());
    // Spawn a new thread to handle SIGINT and SIGTERM process signals.
    setup_signal_hook(Arc::clone(&shutdown_signal)).await;

    //
    // Infinite loop that will indefinitely retry failed connections unless
    // a SIGTERM or SIGINT signal is received.
    //
    let mut retry_interval_milis: u64 = 0;
    let mut retry_counter: u64 = 0;
    while retry_counter < MAX_CONNECTION_ATTEMPTS {
        let mut client = client.lock().await;
        match client.connect_direct(Arc::clone(&shutdown_signal)).await {
            Ok(_) => {
                client.disconnect().await;
                break;
            }
            Err(e) => {
                error!("Failed to connect: {:?}", e);
                tokio::time::sleep(std::time::Duration::from_millis(retry_interval_milis)).await;
                retry_interval_milis = (retry_interval_milis + (200 * retry_counter)).min(5000);
                retry_counter += 1;
                warn!(
                    "Retrying connection in {} seconds...",
                    format!("{:.2}", retry_interval_milis as f64 / 1000.0)
                );
            }
        }
    }

    if retry_counter == MAX_CONNECTION_ATTEMPTS {
        error!(
            "Failed to connect after {} retries. Exiting...",
            retry_counter
        );
    } else {
        info!("Exiting orderly from Lightstreamer client...");
    }

    // Exit using std::process::exit() to avoid waiting for existing tokio tasks to complete.
    std::process::exit(0);
}
