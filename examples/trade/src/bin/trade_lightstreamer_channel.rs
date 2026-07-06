//! Trade data streaming example using channel-based pattern for receiving updates.
//!
//! This example demonstrates how to use channels to receive trade updates
//! (CONFIRMS, OPU, WOU) from Lightstreamer, which is useful for multithreaded
//! applications where different threads need to process the data.

use ig_client::prelude::*;
use lightstreamer_rs::client::{LightstreamerClient, Transport};
use lightstreamer_rs::subscription::{ItemUpdate, Snapshot, Subscription, SubscriptionMode};
use lightstreamer_rs::utils::setup_signal_hook;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify, mpsc};
use tracing::{debug, error, info, warn};

const MAX_CONNECTION_ATTEMPTS: u64 = 3;
const CHANNEL_BUFFER_SIZE: usize = 100;

/// Channel-based listener that sends ItemUpdates through a channel
/// instead of processing them directly in the callback.
struct ChannelListener {
    sender: mpsc::Sender<ItemUpdate>,
}

impl ChannelListener {
    fn new(sender: mpsc::Sender<ItemUpdate>) -> Self {
        Self { sender }
    }
}

impl lightstreamer_rs::subscription::SubscriptionListener for ChannelListener {
    fn on_item_update(&self, update: &ItemUpdate) {
        let update_clone = update.clone();
        let sender = self.sender.clone();

        match sender.try_send(update_clone) {
            Ok(_) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("Channel buffer full, dropping update");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                error!("Channel closed, cannot send update");
            }
        }
    }

    fn on_subscription(&mut self) {
        info!("Trade subscription confirmed by the server");
    }
}

/// Process trade updates received through the channel
async fn process_updates(mut receiver: mpsc::Receiver<ItemUpdate>) {
    info!("Starting trade update processor task");

    while let Some(update) = receiver.recv().await {
        let trade_data = TradeData::from(&update);

        match serde_json::to_string_pretty(&trade_data) {
            Ok(json) => info!("Received TradeData:\n{}", json),
            Err(e) => error!("Failed to serialize TradeData: {}", e),
        }
    }

    info!("Trade update processor task finished");
}

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    let client = Client::try_new()?;
    let ws_info = client.ws_info().await?;
    let password = ws_info.get_ws_password();

    debug!(
        server = %ws_info.server,
        account_id = %ws_info.account_id,
        "WebSocket info obtained"
    );
    info!("Using Lightstreamer server: {}", ws_info.server);
    info!("Using account ID: {}", ws_info.account_id);

    // Create a channel for receiving updates
    let (sender, receiver) = mpsc::channel::<ItemUpdate>(CHANNEL_BUFFER_SIZE);

    // Spawn a task to process updates from the channel
    let processor_handle = tokio::spawn(process_updates(receiver));

    // Create a subscription for trade data
    let epic = format!("TRADE:{}", ws_info.account_id);
    info!("Subscribing to trade updates: {}", epic);

    let mut subscription = Subscription::new(
        SubscriptionMode::Distinct,
        Some(vec![epic]),
        Some(vec![
            "CONFIRMS".to_string(),
            "OPU".to_string(),
            "WOU".to_string(),
        ]),
    )?;

    let listener = ChannelListener::new(sender);
    subscription.set_data_adapter(None)?;
    subscription.set_requested_snapshot(Some(Snapshot::Yes))?;
    subscription.add_listener(Box::new(listener));

    // Create the Lightstreamer client
    let ls_client = Arc::new(Mutex::new(LightstreamerClient::new(
        Some(ws_info.server.as_str()),
        None,
        Some(&ws_info.account_id),
        Some(&password),
    )?));

    // Add the subscription to the client
    {
        let mut ls = ls_client.lock().await;
        LightstreamerClient::subscribe(ls.subscription_sender.clone(), subscription).await?;
        ls.connection_options
            .set_forced_transport(Some(Transport::WsStreaming));
        info!("Trade subscription added");
    }

    // Setup signal handling for graceful shutdown
    let shutdown_signal = Arc::new(Notify::new());
    setup_signal_hook(Arc::clone(&shutdown_signal)).await;

    // Connection loop with retry logic
    let mut retry_interval_millis: u64 = 0;
    let mut retry_counter: u64 = 0;

    while retry_counter < MAX_CONNECTION_ATTEMPTS {
        let mut ls = ls_client.lock().await;
        match ls.connect_direct(Arc::clone(&shutdown_signal)).await {
            Ok(_) => {
                ls.disconnect().await;
                break;
            }
            Err(e) => {
                error!("Failed to connect: {:?}", e);
                tokio::time::sleep(std::time::Duration::from_millis(retry_interval_millis)).await;
                retry_interval_millis = (retry_interval_millis + (200 * retry_counter)).min(5000);
                retry_counter += 1;
                warn!(
                    "Retrying connection in {:.2} seconds...",
                    retry_interval_millis as f64 / 1000.0
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

    drop(ls_client);
    let _ = processor_handle.await;

    std::process::exit(0);
}
