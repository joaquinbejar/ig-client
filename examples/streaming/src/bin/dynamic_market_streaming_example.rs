/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 30/10/25
******************************************************************************/

//! Example demonstrating how to use the DynamicMarketStreamer for real-time market data
//! with dynamic subscription management from multiple threads.
//!
//! This example shows how to:
//! - Create a dynamic market streamer with thread-safe operations
//! - Add and remove market subscriptions dynamically
//! - Handle price updates from multiple markets
//! - Manage subscriptions from different threads concurrently

use ig_client::application::dynamic_streamer::DynamicMarketStreamer;
use ig_client::error::AppError;
use ig_client::model::streaming::StreamingMarketField;
use ig_client::prelude::setup_logger;
use std::collections::HashSet;
use tokio::time::{Duration, sleep};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Initialize logging
    setup_logger();

    info!("Starting dynamic market streaming example...");

    // Define which market fields we want to receive
    let fields = HashSet::from([
        StreamingMarketField::Bid, // Bid price
                                   // StreamingMarketField::Offer,       // Offer/Ask price
                                   // StreamingMarketField::High,        // High price
                                   // StreamingMarketField::Low,         // Low price
                                   // StreamingMarketField::MidOpen,     // Mid open price
                                   // StreamingMarketField::Change,      // Price change
                                   // StreamingMarketField::ChangePct,   // Percentage change
                                   // StreamingMarketField::UpdateTime,  // Last update time
                                   // StreamingMarketField::MarketDelay, // Market delay
                                   // StreamingMarketField::MarketState, // Market state (TRADEABLE, etc.)
    ]);

    // Create the dynamic streamer
    let mut streamer = DynamicMarketStreamer::new(fields);
    info!("Dynamic market streamer created");

    // Get the receiver for price updates (can only be called once)
    let mut receiver = streamer.get_receiver().await?;

    // Spawn a task to handle incoming market data updates
    tokio::spawn(async move {
        while let Some(price_data) = receiver.recv().await {
            info!("Market update - {}", price_data);
        }
    });

    // Add initial markets - SPX options
    let initial_epics = vec![
        "OP.D.OTCSPX3.6875P.IP".to_string(),
        "OP.D.OTCSPX3.6875C.IP".to_string(),
    ];

    for epic in &initial_epics {
        streamer.add(epic.clone()).await?;
        info!("Added initial market: {}", epic);
    }

    // Clone the streamer for use in other threads
    let streamer_clone1 = streamer.clone();
    let streamer_clone2 = streamer.clone();
    let streamer_clone3 = streamer.clone();

    // Spawn a task to add markets dynamically after 5 seconds
    tokio::spawn(async move {
        sleep(Duration::from_secs(5)).await;
        info!("Adding new markets from thread 1...");

        let new_epics = vec![
            "OP.D.OTCSPX3.6880P.IP".to_string(),
            "OP.D.OTCSPX3.6880C.IP".to_string(),
        ];

        for epic in new_epics {
            match streamer_clone1.add(epic.clone()).await {
                Ok(_) => info!("Thread 1 successfully added: {}", epic),
                Err(e) => tracing::error!("Failed to add {}: {:?}", epic, e),
            }
        }

        let current_epics = streamer_clone1.get_epics().await;
        info!(
            "Thread 1 - Current subscriptions: {} EPICs",
            current_epics.len()
        );
    });

    // Spawn another task to add more markets after 10 seconds
    tokio::spawn(async move {
        sleep(Duration::from_secs(10)).await;
        info!("Adding more markets from thread 2...");

        let more_epics = vec![
            "OP.D.OTCSPX3.6900P.IP".to_string(),
            "OP.D.OTCSPX3.6900C.IP".to_string(),
        ];

        for epic in more_epics {
            match streamer_clone2.add(epic.clone()).await {
                Ok(_) => info!("Thread 2 successfully added: {}", epic),
                Err(e) => tracing::error!("Failed to add {}: {:?}", epic, e),
            }
        }

        let current_epics = streamer_clone2.get_epics().await;
        info!(
            "Thread 2 - Current subscriptions: {} EPICs",
            current_epics.len()
        );
    });

    // Spawn a task to add even more markets and then remove one after 15 seconds
    tokio::spawn(async move {
        sleep(Duration::from_secs(15)).await;
        info!("Adding final markets from thread 3...");

        let final_epics = vec![
            "OP.D.OTCSPX3.6910P.IP".to_string(),
            "OP.D.OTCSPX3.6910C.IP".to_string(),
        ];

        for epic in final_epics {
            match streamer_clone3.add(epic.clone()).await {
                Ok(_) => info!("Thread 3 successfully added: {}", epic),
                Err(e) => tracing::error!("Failed to add {}: {:?}", epic, e),
            }
        }

        // Wait a bit and then remove one market
        sleep(Duration::from_secs(5)).await;
        info!("Removing a market from thread 3...");

        let epic_to_remove = "OP.D.OTCSPX3.6875P.IP".to_string();
        if let Err(e) = streamer_clone3.remove(epic_to_remove.clone()).await {
            tracing::error!("Failed to remove {}: {:?}", epic_to_remove, e);
        } else {
            info!("Thread 3 removed: {}", epic_to_remove);
        }

        // Show current subscriptions
        let current_epics = streamer_clone3.get_epics().await;
        info!(
            "Current subscriptions ({} total): {:?}",
            current_epics.len(),
            current_epics
        );
    });

    // Connect and maintain the connection
    // This will block until SIGINT/SIGTERM or connection failure
    info!("Connecting to Lightstreamer server...");
    info!("Monitoring markets. Press Ctrl+C to exit.");
    info!("Markets will be added/removed dynamically during execution.");

    streamer.connect().await?;

    // Cleanup (only reached after graceful shutdown)
    info!("Dynamic market streaming example completed");
    Ok(())
}
