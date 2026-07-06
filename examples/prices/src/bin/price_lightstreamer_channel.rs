//! Price streaming example using channel-based pattern for receiving updates.
//!
//! This example demonstrates how to use channels to receive price updates
//! from Lightstreamer, which is useful for multithreaded applications where
//! different threads need to process the data.

use ig_client::prelude::Client;
use ig_client::presentation::price::PriceData;
use ig_client::utils::logger::setup_logger;
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
        // Clone the update and send it through the channel
        let update_clone = update.clone();
        let sender = self.sender.clone();

        // Use try_send to avoid blocking - if the channel is full, we log a warning
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
        info!("Subscription confirmed by the server");
    }
}

/// Process updates received through the channel
async fn process_updates(mut receiver: mpsc::Receiver<ItemUpdate>) {
    info!("Starting update processor task");

    while let Some(update) = receiver.recv().await {
        // Convert ItemUpdate to PriceData
        let price_data = PriceData::from(&update);

        // Process the price data (in a real application, you might:
        // - Store it in a database
        // - Send it to another service
        // - Update a UI
        // - Perform calculations
        match serde_json::to_string_pretty(&price_data) {
            Ok(json) => info!("Received PriceData:\n{}", json),
            Err(e) => error!("Failed to serialize PriceData: {}", e),
        }
    }

    info!("Update processor task finished");
}

#[tokio::main]
async fn main() -> Result<(), ig_client::error::AppError> {
    setup_logger();

    // Initialize the IG client and get WebSocket credentials
    let http_client = Client::try_new()?;
    let ws_info = http_client.ws_info().await?;
    let password = ws_info.get_ws_password();
    debug!(
        server = %ws_info.server,
        account_id = %ws_info.account_id,
        "WebSocket info obtained"
    );

    // Create a channel for receiving updates
    let (sender, receiver) = mpsc::channel::<ItemUpdate>(CHANNEL_BUFFER_SIZE);

    // Spawn a task to process updates from the channel
    let processor_handle = tokio::spawn(process_updates(receiver));

    // Create a subscription for a market
    let epic = format!("PRICE:{}:OP.D.OTCSPXWK.6720C.IP", ws_info.account_id);
    info!("Subscribing to: {}", epic);

    let mut subscription = Subscription::new(
        SubscriptionMode::Merge,
        Some(vec![epic]),
        Some(vec![
            "HIGH".to_string(),
            "LOW".to_string(),
            "BIDSIZE1".to_string(),
            "ASKSIZE1".to_string(),
            "DLG_FLAG".to_string(),
            "C1BIDSIZE1".to_string(),
            "C1ASKSIZE1".to_string(),
            "TIMESTAMP".to_string(),
            "NET_CHG".to_string(),
        ]),
    )?;

    // Create the channel-based listener
    let listener = ChannelListener::new(sender);
    subscription.set_data_adapter(Some("Pricing".to_string()))?;
    subscription.set_requested_snapshot(Some(Snapshot::Yes))?;
    subscription.add_listener(Box::new(listener));

    // Create the Lightstreamer client
    let client = Arc::new(Mutex::new(LightstreamerClient::new(
        Some(ws_info.server.as_str()),
        None, // Use default adapter set for IG Markets
        Some(&ws_info.account_id),
        Some(&password),
    )?));

    // Add the subscription to the client
    {
        let mut client = client.lock().await;
        LightstreamerClient::subscribe(client.subscription_sender.clone(), subscription).await?;
        client
            .connection_options
            .set_forced_transport(Some(Transport::WsStreaming));
        info!("Subscription added");
    }

    // Setup signal handling for graceful shutdown
    let shutdown_signal = Arc::new(Notify::new());
    setup_signal_hook(Arc::clone(&shutdown_signal)).await;

    // Connection loop with retry logic
    let mut retry_interval_millis: u64 = 0;
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

    // Wait for the processor task to finish (it will end when the channel is dropped)
    drop(client); // Drop the client to close the channel
    let _ = processor_handle.await;

    std::process::exit(0);
}
