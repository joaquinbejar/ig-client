//! Trade streaming (CONFIRMS / OPU / WOU) driven directly by
//! `lightstreamer-rs`, with `ig_client`'s [`Listener`] doing the decoding.

use ig_client::application::interfaces::listener::Listener;
use ig_client::error::AppError;
use ig_client::prelude::*;
use lightstreamer_rs::{
    Client as LsClient, ClientConfig, Credentials, FieldSchema, ItemGroup, ServerAddress, Snapshot,
    Subscription, SubscriptionMode,
};
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{debug, info, warn};

/// The trade fields IG publishes on a `TRADE:` item.
const TRADE_FIELDS: [&str; 3] = ["CONFIRMS", "OPU", "WOU"];

fn callback(update: &TradeData) -> Result<(), AppError> {
    let item = serde_json::to_string_pretty(&update)?;
    info!("TradeData: {}", item);
    Ok(())
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

    // The Lightstreamer password is the IG session token pair; `Credentials`
    // redacts it, and nothing here prints it.
    let config = ClientConfig::builder(ServerAddress::try_new(ws_info.server.as_str())?)
        .with_credentials(Credentials::new(
            ws_info.account_id.as_str(),
            ws_info.get_ws_password(),
        ))
        .build()?;

    let (ls_client, _session_events) = LsClient::connect(config).await?;
    info!("Connected to Lightstreamer");

    // Each confirmation is its own event, so DISTINCT rather than MERGE.
    let updates = ls_client
        .subscribe(
            Subscription::new(
                SubscriptionMode::Distinct,
                ItemGroup::from_items([format!("TRADE:{}", ws_info.account_id)])?,
                FieldSchema::from_fields(TRADE_FIELDS)?,
            )
            .with_snapshot(Snapshot::On),
        )
        .await?;

    // The pump is the listener's shutdown path: signal, then await it.
    let shutdown = Arc::new(Notify::new());
    let pump = Listener::new(callback).spawn(updates, Arc::clone(&shutdown), "trade");

    let interrupted = tokio::signal::ctrl_c().await;
    shutdown.notify_one();
    if let Err(e) = pump.await {
        warn!(error = %e, "listener pump did not exit cleanly");
    }
    ls_client.disconnect().await?;
    info!("Exiting orderly from the Lightstreamer client");

    interrupted
        .map_err(|e| AppError::Generic(format!("could not wait for the interrupt signal: {e}")))
}
