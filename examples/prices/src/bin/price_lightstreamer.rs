//! Price streaming driven directly by `lightstreamer-rs`, with `ig_client`'s
//! [`Listener`] doing the decoding.
//!
//! This is the low-level path: the Lightstreamer session, the subscription and
//! the shutdown are all explicit. Use `StreamerClient` when you want those
//! managed for you.

use ig_client::application::interfaces::listener::Listener;
use ig_client::error::AppError;
use ig_client::prelude::Client;
use ig_client::presentation::price::PriceData;
use ig_client::utils::logger::setup_logger;
use lightstreamer_rs::{
    Client as LsClient, ClientConfig, Credentials, FieldSchema, ItemGroup, ServerAddress, Snapshot,
    Subscription, SubscriptionMode,
};
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{debug, info};

/// The price fields this example asks IG for.
const PRICE_FIELDS: [&str; 5] = ["HIGH", "LOW", "BIDSIZE1", "ASKSIZE1", "DLG_FLAG"];

fn callback(update: &PriceData) -> Result<(), AppError> {
    let item = serde_json::to_string_pretty(&update)?;
    info!("PriceData: {}", item);
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

    let item = format!("PRICE:{}:OP.D.OTCSPXWK.6720C.IP", ws_info.account_id);
    let updates = ls_client
        .subscribe(
            Subscription::new(
                SubscriptionMode::Merge,
                ItemGroup::from_items([item])?,
                FieldSchema::from_fields(PRICE_FIELDS)?,
            )
            // Detailed prices come from IG's `Pricing` data adapter.
            .with_data_adapter("Pricing")
            .with_snapshot(Snapshot::Off),
        )
        .await?;

    // The pump is the listener's shutdown path: signal, then await it.
    let shutdown = Arc::new(Notify::new());
    let pump = Listener::new(callback).spawn(updates, Arc::clone(&shutdown), "price");

    let interrupted = tokio::signal::ctrl_c().await;
    shutdown.notify_one();
    if let Err(e) = pump.await {
        tracing::warn!(error = %e, "listener pump did not exit cleanly");
    }
    ls_client.disconnect().await?;
    info!("Exiting orderly from the Lightstreamer client");

    interrupted
        .map_err(|e| AppError::Generic(format!("could not wait for the interrupt signal: {e}")))
}
