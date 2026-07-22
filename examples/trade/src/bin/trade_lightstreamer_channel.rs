//! Trade data streaming example using a channel to hand updates to another
//! task.
//!
//! Useful when the task that receives the data (CONFIRMS, OPU, WOU) is not the
//! task that should process it: the listener callback does nothing but forward,
//! and all the real work happens in a separate task that owns the receiver.

use ig_client::application::interfaces::listener::Listener;
use ig_client::error::AppError;
use ig_client::prelude::*;
use lightstreamer_rs::{
    Client as LsClient, ClientConfig, Credentials, FieldSchema, ItemGroup, ServerAddress, Snapshot,
    Subscription, SubscriptionMode,
};
use std::sync::Arc;
use tokio::sync::{Notify, mpsc};
use tracing::{debug, error, info, warn};

/// How many updates may queue before the forwarding callback starts dropping.
const CHANNEL_BUFFER_SIZE: usize = 100;

/// The trade fields IG publishes on a `TRADE:` item.
const TRADE_FIELDS: [&str; 3] = ["CONFIRMS", "OPU", "WOU"];

/// Consumes the decoded updates. In a real application this is where they would
/// be stored, forwarded or acted on.
async fn process_updates(mut receiver: mpsc::Receiver<TradeData>) {
    info!("Starting trade update processor task");

    while let Some(trade_data) = receiver.recv().await {
        match serde_json::to_string_pretty(&trade_data) {
            Ok(json) => info!("Received TradeData:\n{}", json),
            Err(e) => error!("Failed to serialize TradeData: {}", e),
        }
    }

    info!("Trade update processor task finished");
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    setup_logger();

    let http_client = Client::try_new()?;
    let ws_info = http_client.ws_info().await?;
    debug!(
        server = %ws_info.server,
        account_id = %ws_info.account_id,
        "WebSocket info obtained"
    );

    let (sender, receiver) = mpsc::channel::<TradeData>(CHANNEL_BUFFER_SIZE);
    let processor = tokio::spawn(process_updates(receiver));

    let config = ClientConfig::builder(ServerAddress::try_new(ws_info.server.as_str())?)
        .with_credentials(Credentials::new(
            ws_info.account_id.as_str(),
            ws_info.get_ws_password(),
        ))
        .build()?;

    let (ls_client, _session_events) = LsClient::connect(config).await?;
    info!("Connected to Lightstreamer");

    let item = format!("TRADE:{}", ws_info.account_id);
    info!("Subscribing to trade updates: {}", item);
    let updates = ls_client
        .subscribe(
            Subscription::new(
                SubscriptionMode::Distinct,
                ItemGroup::from_items([item])?,
                FieldSchema::from_fields(TRADE_FIELDS)?,
            )
            .with_snapshot(Snapshot::On),
        )
        .await?;

    // The callback only forwards. `try_send` keeps the streaming task from
    // blocking: a full buffer means the processor is behind, which is worth a
    // warning rather than backpressure into the socket.
    let listener = Listener::new(move |trade_data: &TradeData| {
        match sender.try_send(trade_data.clone()) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("Channel buffer full, dropping update")
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                error!("Channel closed, cannot send update");
            }
        }
        Ok(())
    });

    let shutdown = Arc::new(Notify::new());
    let pump = listener.spawn(updates, Arc::clone(&shutdown), "trade");

    let interrupted = tokio::signal::ctrl_c().await;
    shutdown.notify_one();
    // Awaiting the pump drops the last sender, which ends the processor.
    if let Err(e) = pump.await {
        warn!(error = %e, "listener pump did not exit cleanly");
    }
    if let Err(e) = processor.await {
        warn!(error = %e, "update processor did not exit cleanly");
    }
    ls_client.disconnect().await?;
    info!("Exiting orderly from the Lightstreamer client");

    interrupted
        .map_err(|e| AppError::Generic(format!("could not wait for the interrupt signal: {e}")))
}
