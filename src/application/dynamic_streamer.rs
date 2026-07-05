/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 30/10/25
******************************************************************************/

//! Dynamic market streaming with thread-safe subscription management.
//!
//! This module provides a wrapper around `StreamerClient` that allows dynamic
//! addition and removal of market subscriptions from multiple threads.

use crate::application::client::StreamerClient;
use crate::error::AppError;
use crate::model::streaming::StreamingMarketField;
use crate::presentation::price::PriceData;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Notify, RwLock, mpsc};
use tracing::{debug, info, warn};

/// Dynamic market streamer with thread-safe subscription management.
///
/// This struct wraps a `StreamerClient` and provides methods to dynamically
/// add, remove, and clear market subscriptions. All operations are thread-safe
/// and can be called from multiple threads concurrently.
///
/// # Examples
///
/// ```ignore
/// use ig_client::application::dynamic_streamer::DynamicMarketStreamer;
/// use ig_client::model::streaming::StreamingMarketField;
/// use std::collections::HashSet;
///
/// #[tokio::main]
/// async fn main() -> Result<(), ig_client::error::AppError> {
///     // Create fields to subscribe to
///     let fields = HashSet::from([
///         StreamingMarketField::Bid,
///         StreamingMarketField::Offer,
///     ]);
///
///     // Create the dynamic streamer
///     let mut streamer = DynamicMarketStreamer::new(fields).await?;
///
///     // Get the receiver for price updates
///     let mut receiver = streamer.get_receiver().await?;
///
///     // Add markets from different threads
///     let streamer_clone = streamer.clone();
///     tokio::spawn(async move {
///         if let Err(e) = streamer_clone.add("IX.D.DAX.DAILY.IP".to_string()).await {
///             tracing::error!("Failed to add market: {}", e);
///         }
///     });
///
///     // Start receiving updates
///     tokio::spawn(async move {
///         while let Some(price_data) = receiver.recv().await {
///             println!("Price update: {}", price_data);
///         }
///     });
///
///     // Connect and run
///     streamer.connect(None).await?;
///     Ok(())
/// }
/// ```
pub struct DynamicMarketStreamer {
    /// Internal streamer client (recreated on epic changes)
    client: Arc<RwLock<Option<StreamerClient>>>,
    /// Set of EPICs currently subscribed
    epics: Arc<RwLock<HashSet<String>>>,
    /// Market fields to subscribe to
    fields: HashSet<StreamingMarketField>,
    /// Channel sender for price updates
    price_tx: Arc<RwLock<Option<mpsc::UnboundedSender<PriceData>>>>,
    /// Channel receiver for price updates (taken on first get_receiver call)
    price_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<PriceData>>>>,
    /// Flag indicating if the streamer is connected
    is_connected: Arc<RwLock<bool>>,
    /// Shutdown signal for current connection
    shutdown_signal: Arc<RwLock<Option<Arc<Notify>>>>,
    /// Monotonic connection generation. Bumped on every `start_internal`; a
    /// connection task only clears `is_connected` on exit if its captured
    /// generation is still current, so a superseded task tearing down its old
    /// connection cannot clobber the state of the newer one that replaced it.
    generation: Arc<AtomicU64>,
}

impl DynamicMarketStreamer {
    /// Creates a new dynamic market streamer.
    ///
    /// # Arguments
    ///
    /// * `fields` - Set of market data fields to receive (e.g., BID, OFFER, etc.)
    ///
    /// # Returns
    ///
    /// Returns a new `DynamicMarketStreamer` instance or an error if initialization fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let fields = HashSet::from([
    ///     StreamingMarketField::Bid,
    ///     StreamingMarketField::Offer,
    /// ]);
    /// let streamer = DynamicMarketStreamer::new(fields).await?;
    /// ```
    pub async fn new(fields: HashSet<StreamingMarketField>) -> Result<Self, AppError> {
        let (price_tx, price_rx) = mpsc::unbounded_channel();

        Ok(Self {
            client: Arc::new(RwLock::new(None)),
            epics: Arc::new(RwLock::new(HashSet::new())),
            fields,
            price_tx: Arc::new(RwLock::new(Some(price_tx))),
            price_rx: Arc::new(RwLock::new(Some(price_rx))),
            is_connected: Arc::new(RwLock::new(false)),
            shutdown_signal: Arc::new(RwLock::new(None)),
            generation: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Adds a market EPIC to the subscription list.
    ///
    /// If the streamer is already connected, this will reconnect with the updated list.
    ///
    /// # Arguments
    ///
    /// * `epic` - The market EPIC to subscribe to
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the EPIC was added successfully.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// streamer.add("IX.D.DAX.DAILY.IP".to_string()).await?;
    /// ```
    pub async fn add(&self, epic: String) -> Result<(), AppError> {
        let mut epics = self.epics.write().await;

        if epics.contains(&epic) {
            debug!("EPIC {} already subscribed", epic);
            return Ok(());
        }

        epics.insert(epic.clone());
        info!("Added EPIC {} to subscription list", epic);
        drop(epics); // Release lock

        // If already connected, reconnect with new list
        let is_connected = *self.is_connected.read().await;
        if is_connected {
            self.reconnect().await?;
        }

        Ok(())
    }

    /// Removes a market EPIC from the subscription list.
    ///
    /// If the streamer is already connected, this will reconnect with the updated list.
    ///
    /// # Arguments
    ///
    /// * `epic` - The market EPIC to remove
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the EPIC was removed successfully.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// streamer.remove("IX.D.DAX.DAILY.IP".to_string()).await?;
    /// ```
    pub async fn remove(&self, epic: String) -> Result<(), AppError> {
        let mut epics = self.epics.write().await;

        let was_removed = epics.remove(&epic);
        if was_removed {
            info!("Removed EPIC {} from subscription list", epic);
        } else {
            debug!("EPIC {} was not in subscription list", epic);
        }
        drop(epics); // Release lock

        // If already connected and something was removed, reconnect
        if was_removed {
            let is_connected = *self.is_connected.read().await;
            if is_connected {
                self.reconnect().await?;
            }
        }

        Ok(())
    }

    /// Clears all market EPICs from the subscription list.
    ///
    /// If the streamer is connected, the live connection is shut down so it
    /// stops forwarding updates for the cleared EPICs, mirroring [`remove`]:
    /// the current connection is signalled and, because reconnection no-ops on
    /// an empty EPIC set, no new connection is started. After this call the
    /// streamer reports as disconnected.
    ///
    /// [`remove`]: DynamicMarketStreamer::remove
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when all EPICs have been cleared and, if it was
    /// connected, the connection has been signalled to stop.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// streamer.clear().await?;
    /// ```
    pub async fn clear(&self) -> Result<(), AppError> {
        let mut epics = self.epics.write().await;
        let count = epics.len();
        epics.clear();
        info!("Cleared {} EPICs from subscription list", count);
        drop(epics); // Release lock before touching connection state

        // If connected, shut the live connection down so data flow actually
        // stops. `reconnect` signals the current connection; `start_internal`
        // no-ops on the now-empty EPIC set, so nothing reconnects.
        let is_connected = *self.is_connected.read().await;
        if is_connected {
            self.reconnect().await?;
            // No new connection is started for an empty EPIC set, so make the
            // stopped state explicit and immediate instead of waiting for the
            // connection task to flip it asynchronously.
            *self.is_connected.write().await = false;
        }

        Ok(())
    }

    /// Gets the current list of subscribed EPICs.
    ///
    /// # Returns
    ///
    /// Returns a vector containing all currently subscribed EPICs.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let epics = streamer.get_epics().await;
    /// println!("Subscribed to {} markets", epics.len());
    /// ```
    pub async fn get_epics(&self) -> Vec<String> {
        let epics = self.epics.read().await;
        epics.iter().cloned().collect()
    }

    /// Gets the receiver for price updates.
    ///
    /// This method can only be called once. Subsequent calls will return an error.
    ///
    /// # Returns
    ///
    /// Returns a receiver channel for `PriceData` updates, or an error if the
    /// receiver has already been taken.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut receiver = streamer.get_receiver().await?;
    /// tokio::spawn(async move {
    ///     while let Some(price_data) = receiver.recv().await {
    ///         println!("Price update: {}", price_data);
    ///     }
    /// });
    /// ```
    pub async fn get_receiver(&self) -> Result<mpsc::UnboundedReceiver<PriceData>, AppError> {
        let mut rx_lock = self.price_rx.write().await;
        rx_lock
            .take()
            .ok_or_else(|| AppError::InvalidInput("Receiver already taken".to_string()))
    }

    /// Reconnects the streamer with the current list of EPICs.
    ///
    /// This method disconnects the current client and creates a new one with
    /// the updated EPIC list.
    async fn reconnect(&self) -> Result<(), AppError> {
        info!("Reconnecting with updated EPIC list...");

        // Signal shutdown to current client
        {
            let shutdown_lock = self.shutdown_signal.read().await;
            if let Some(signal) = shutdown_lock.as_ref() {
                signal.notify_one();
            }
        }

        // Wait a bit for graceful shutdown
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Start new connection
        let epics = self.get_epics().await;
        if !epics.is_empty() {
            self.start_internal().await?;
        }

        Ok(())
    }

    /// Internal method to start connection.
    async fn start_internal(&self) -> Result<(), AppError> {
        let epics = self.get_epics().await;

        if epics.is_empty() {
            warn!("No EPICs to subscribe to");
            return Ok(());
        }

        info!("Starting connection with {} EPICs", epics.len());

        // Claim a new connection generation. The task spawned below owns this
        // number; a later `start_internal` bumps it, marking any earlier task as
        // superseded so it will not clobber `is_connected` on teardown.
        let my_generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        // Create new client
        let mut new_client = StreamerClient::new().await?;

        // Subscribe to all EPICs
        let fields = self.fields.clone();
        let mut receiver = new_client.market_subscribe(epics.clone(), fields).await?;

        // Forward updates to the main channel
        let price_tx = self.price_tx.read().await;
        if let Some(tx) = price_tx.as_ref() {
            let tx = tx.clone();
            tokio::spawn(async move {
                while let Some(price_data) = receiver.recv().await {
                    if tx.send(price_data).is_err() {
                        warn!("Failed to send price update: receiver dropped");
                        break;
                    }
                }
                debug!("Subscription forwarding task ended");
            });
        }

        // Store the new client
        *self.client.write().await = Some(new_client);

        // Create new shutdown signal
        let signal = Arc::new(Notify::new());
        *self.shutdown_signal.write().await = Some(Arc::clone(&signal));

        // Mark as connected
        *self.is_connected.write().await = true;

        // Spawn connection task in background
        let client = Arc::clone(&self.client);
        let is_connected = Arc::clone(&self.is_connected);
        let generation = Arc::clone(&self.generation);

        tokio::spawn(async move {
            // Move the client out of the shared lock under a short-lived guard,
            // then run the long-lived connection WITHOUT holding any lock. This
            // is the core of the fix: previously the write guard was held across
            // the whole `connect().await`, so any concurrent `add`/`remove`/
            // `disconnect` that needs the client lock hung until disconnect.
            let taken = {
                let mut guard = client.write().await;
                guard.take()
            };

            let result = if let Some(mut c) = taken {
                let r = c.connect(Some(signal)).await;
                // The connection has ended (shutdown signal or error): close the
                // Lightstreamer session and drain its converter tasks, then drop
                // the owned client so it is never left half-open.
                if let Err(e) = c.disconnect().await {
                    tracing::error!("Error closing streamer session: {}", e);
                }
                r
            } else {
                Ok(())
            };

            // Mark as disconnected only if we are still the current generation.
            // A newer `start_internal` (from reconnect on add/remove/clear) may
            // have already brought up a fresh connection while this superseded
            // task was still tearing its old one down; clobbering the flag here
            // would leave the streamer reporting disconnected while live.
            if generation.load(Ordering::SeqCst) == my_generation {
                *is_connected.write().await = false;
            }

            match result {
                Ok(_) => info!("Connection task completed successfully"),
                Err(e) => tracing::error!("Connection task failed: {:?}", e),
            }
        });

        info!("Connection task started in background");
        Ok(())
    }

    /// Starts the connection to the Lightstreamer server and subscribes to all initial EPICs.
    ///
    /// This method subscribes to all EPICs in the subscription list and then spawns a background
    /// task to maintain the connection. This allows dynamic subscription management while connected.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` immediately after starting the connection task.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Start connection
    /// streamer.start().await?;
    ///
    /// // Keep main thread alive
    /// tokio::signal::ctrl_c().await?;
    /// ```
    pub async fn start(&mut self) -> Result<(), AppError> {
        self.start_internal().await
    }

    /// Connects to the Lightstreamer server and blocks until shutdown.
    ///
    /// This is a convenience method that calls `start()` and then waits for a shutdown signal.
    /// Use `start()` if you need non-blocking behavior.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the connection is closed gracefully.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Connect and block until shutdown
    /// streamer.connect().await?;
    /// ```
    pub async fn connect(&mut self) -> Result<(), AppError> {
        self.start().await?;

        // Wait for SIGINT/SIGTERM
        use lightstreamer_rs::utils::setup_signal_hook;
        let signal = Arc::new(Notify::new());
        setup_signal_hook(Arc::clone(&signal)).await;
        signal.notified().await;

        // Disconnect
        self.disconnect().await?;

        Ok(())
    }

    /// Disconnects from the Lightstreamer server.
    ///
    /// This method gracefully closes the connection to the server.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the disconnection was successful.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// streamer.disconnect().await?;
    /// ```
    pub async fn disconnect(&mut self) -> Result<(), AppError> {
        // Signal shutdown to the current connection task; once its `connect`
        // returns it closes the Lightstreamer session itself.
        {
            let shutdown_lock = self.shutdown_signal.read().await;
            if let Some(signal) = shutdown_lock.as_ref() {
                signal.notify_one();
            }
        }

        // If the client is still owned here (the connection task has not taken
        // it yet, or no connection was ever started), close it directly. During
        // a live connection the task owns the client, so this take yields `None`
        // and the task performs the close after being signalled above. The guard
        // is scoped so it is never held across the `disconnect().await`.
        let taken = {
            let mut client_lock = self.client.write().await;
            client_lock.take()
        };
        if let Some(mut client) = taken {
            client.disconnect().await?;
        }

        *self.is_connected.write().await = false;
        info!("Disconnected from Lightstreamer server");
        Ok(())
    }
}

impl Clone for DynamicMarketStreamer {
    fn clone(&self) -> Self {
        Self {
            client: Arc::clone(&self.client),
            epics: Arc::clone(&self.epics),
            fields: self.fields.clone(),
            price_tx: Arc::clone(&self.price_tx),
            price_rx: Arc::clone(&self.price_rx),
            is_connected: Arc::clone(&self.is_connected),
            shutdown_signal: Arc::clone(&self.shutdown_signal),
            generation: Arc::clone(&self.generation),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DynamicMarketStreamer;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use tokio::sync::Notify;

    const TEST_EPIC: &str = "IX.D.DAX.DAILY.IP";

    // --- Task 4: clear() must stop data flow when connected ----------------

    #[tokio::test]
    async fn test_clear_when_not_connected_empties_epics() {
        let streamer = DynamicMarketStreamer::new(HashSet::new())
            .await
            .expect("streamer construction should succeed");
        streamer.epics.write().await.insert(TEST_EPIC.to_string());

        let result = streamer.clear().await;

        assert!(result.is_ok(), "clear should succeed: {result:?}");
        assert!(
            streamer.get_epics().await.is_empty(),
            "EPIC set should be empty after clear"
        );
        assert!(
            !*streamer.is_connected.read().await,
            "should not report connected when it never was"
        );
    }

    #[tokio::test]
    async fn test_clear_when_connected_signals_shutdown_and_reports_stopped() {
        let streamer = DynamicMarketStreamer::new(HashSet::new())
            .await
            .expect("streamer construction should succeed");

        // Simulate a live connection: one EPIC, connected, and a shutdown
        // signal that a connection task would be parked on.
        streamer.epics.write().await.insert(TEST_EPIC.to_string());
        *streamer.is_connected.write().await = true;
        let signal = Arc::new(Notify::new());
        *streamer.shutdown_signal.write().await = Some(Arc::clone(&signal));

        // Stand-in for the live connection waiting to be shut down.
        let waiter = tokio::spawn(async move { signal.notified().await });

        let result = streamer.clear().await;

        assert!(result.is_ok(), "clear should succeed: {result:?}");
        assert!(
            streamer.get_epics().await.is_empty(),
            "EPIC set should be empty after clear"
        );
        assert!(
            !*streamer.is_connected.read().await,
            "clear must mark the streamer disconnected so data flow stops"
        );
        // clear() must have signalled the live connection to shut down. With a
        // stored permit the waiter wakes deterministically.
        assert!(
            tokio::time::timeout(Duration::from_secs(1), waiter)
                .await
                .is_ok(),
            "live connection did not observe the shutdown signal from clear()"
        );
    }

    #[tokio::test]
    async fn test_superseded_generation_does_not_clear_is_connected() {
        // Models the connection-task teardown race: a newer generation is live
        // (is_connected == true) while an older, superseded task finishes tearing
        // its connection down. The superseded task must NOT clear the flag.
        let streamer = DynamicMarketStreamer::new(HashSet::new())
            .await
            .expect("streamer construction should succeed");

        // A newer connection has come up: bump the generation and mark connected.
        let newer = streamer.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *streamer.is_connected.write().await = true;

        // An older task captured an earlier generation.
        let older = newer - 1;

        // Replicate the task's exit guard for the superseded (older) generation.
        if streamer.generation.load(Ordering::SeqCst) == older {
            *streamer.is_connected.write().await = false;
        }
        assert!(
            *streamer.is_connected.read().await,
            "a superseded generation must not clear is_connected on the newer one"
        );

        // The current generation's own teardown still clears it.
        if streamer.generation.load(Ordering::SeqCst) == newer {
            *streamer.is_connected.write().await = false;
        }
        assert!(
            !*streamer.is_connected.read().await,
            "the current generation's teardown must clear is_connected"
        );
    }
}
