/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/10/25
******************************************************************************/

//! Callback-style consumption of a Lightstreamer subscription.
//!
//! `lightstreamer-rs` 1.0 delivers updates as a [`Stream`] rather than through
//! a listener trait, but this crate's own callback surface is unchanged:
//! [`Listener::new`] still takes a `Fn(&T) -> ListenerResult`. What used to be
//! an upstream trait implementation is now [`Listener::spawn`], which pumps the
//! stream and invokes the callback for every update, and
//! [`Listener::on_item_update`], which applies the callback to a single update.
//!
//! [`Stream`]: futures::Stream

use crate::application::streaming_convert::StreamingUpdate;
use crate::error::AppError;
use futures::StreamExt;
use lightstreamer_rs::{SubscriptionEvent, Updates};
use std::fmt::{Debug, Display};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn};

/// Result type for listener operations that don't return a value but may return an error
pub type ListenerResult = Result<(), AppError>;

/// Trade data listener that processes updates through a callback
/// Thread-safe and can be shared between threads
pub struct Listener<T> {
    /// The callback function that will be called with trade data updates
    callback: Arc<dyn Fn(&T) -> ListenerResult + Send + Sync>,
}

impl<T> Listener<T>
where
    T: 'static,
{
    /// Creates a new TradeListener with the specified callback
    ///
    /// # Arguments
    ///
    /// * `callback` - A function that will be called with trade data updates
    ///
    /// # Returns
    ///
    /// A new instance of TradeListener
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&T) -> ListenerResult + Send + Sync + 'static,
    {
        Listener {
            callback: Arc::new(callback),
        }
    }

    /// Updates the callback function
    ///
    /// # Arguments
    ///
    /// * `callback` - The new callback function
    #[allow(dead_code)]
    fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(&T) -> ListenerResult + Send + Sync + 'static,
    {
        self.callback = Arc::new(callback);
    }

    /// Executes the callback with the provided trade data
    ///
    /// # Arguments
    ///
    /// * `trade_data` - The trade data to pass to the callback
    ///
    /// # Returns
    ///
    /// The result of the callback function
    fn callback(&self, data: &T) -> ListenerResult {
        (self.callback)(data)
    }
}

impl<T> Listener<T>
where
    T: for<'a> From<&'a StreamingUpdate> + Display + Debug + 'static,
{
    /// Converts one streaming update and hands it to the callback.
    ///
    /// A failing callback is logged and contained: one bad update must never
    /// end the subscription.
    pub fn on_item_update(&self, update: &StreamingUpdate) {
        let data: T = T::from(update);

        match self.callback(&data) {
            // TRACE, not DEBUG: this fires on every streaming tick and renders
            // the whole payload (account updates carry P&L, equity and margin).
            Ok(()) => trace!(update = %data, "streaming item update"),
            Err(e) => error!(error = %e, "streaming callback failed"),
        }
    }
}

impl<T> Listener<T>
where
    T: for<'a> From<&'a StreamingUpdate> + Display + Debug + Send + 'static,
{
    /// Pumps `updates` until the subscription ends or `shutdown` is signalled,
    /// invoking the callback for every item update.
    ///
    /// This replaces the `SubscriptionListener` implementation the upstream
    /// crate used to require: `lightstreamer-rs` 1.0 hands back a stream from
    /// `subscribe`, so the pump lives here.
    ///
    /// The returned [`JoinHandle`] is the task's shutdown path — await it after
    /// signalling `shutdown` to be sure the callback is no longer running. The
    /// task also ends on its own when the subscription is rejected or
    /// unsubscribed, or when the client is dropped and the stream closes.
    ///
    /// # Arguments
    ///
    /// * `updates` - The subscription stream returned by `Client::subscribe`.
    /// * `shutdown` - Signalled (with `notify_one`) to stop the pump early.
    /// * `label` - Identifies the subscription in log lines.
    #[must_use = "the returned handle is the task's only shutdown path"]
    pub fn spawn(self, updates: Updates, shutdown: Arc<Notify>, label: &str) -> JoinHandle<()> {
        let label = label.to_owned();
        tokio::spawn(async move {
            let mut updates = updates;
            loop {
                let event = tokio::select! {
                    () = shutdown.notified() => {
                        debug!(subscription = %label, "listener pump stopped by shutdown signal");
                        return;
                    }
                    event = updates.next() => event,
                };

                let Some(event) = event else {
                    debug!(subscription = %label, "listener pump stopped: stream closed");
                    return;
                };

                match event {
                    SubscriptionEvent::Update(update) => {
                        self.on_item_update(&StreamingUpdate::from(update.as_ref()));
                    }
                    SubscriptionEvent::Activated {
                        item_count,
                        field_count,
                        ..
                    } => info!(
                        subscription = %label,
                        item_count,
                        field_count,
                        "subscription confirmed by the server"
                    ),
                    // Terminal: the server refused the subscription outright.
                    // Its `Display` carries the server's own code and message
                    // and never a credential.
                    SubscriptionEvent::Rejected(e) => {
                        error!(subscription = %label, error = %e, "subscription rejected");
                        return;
                    }
                    SubscriptionEvent::Unsubscribed => {
                        info!(subscription = %label, "subscription ended");
                        return;
                    }
                    SubscriptionEvent::Overflow {
                        item_index,
                        dropped_count,
                    } => warn!(
                        subscription = %label,
                        item_index,
                        dropped_count,
                        "server dropped updates for this item"
                    ),
                    other => debug!(subscription = %label, event = ?other, "subscription event"),
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Listener, StreamingUpdate};
    use crate::error::AppError;
    use std::collections::HashMap;
    use std::fmt;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct TestData {
        item_name: String,
    }

    impl fmt::Display for TestData {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "TestData({})", self.item_name)
        }
    }

    impl From<&StreamingUpdate> for TestData {
        fn from(update: &StreamingUpdate) -> Self {
            Self {
                item_name: update.item_name.clone().unwrap_or_default(),
            }
        }
    }

    fn update(item_name: &str) -> StreamingUpdate {
        StreamingUpdate {
            item_name: Some(item_name.to_string()),
            item_pos: 1,
            is_snapshot: false,
            fields: HashMap::new(),
            changed_fields: HashMap::new(),
        }
    }

    #[test]
    fn test_on_item_update_invokes_the_callback() {
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&seen);
        let listener = Listener::<TestData>::new(move |data| {
            assert_eq!(data.item_name, "MARKET:IX.D.DAX.DAILY.IP");
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        listener.on_item_update(&update("MARKET:IX.D.DAX.DAILY.IP"));

        assert_eq!(seen.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_callback_error_is_contained() {
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&seen);
        let listener = Listener::<TestData>::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            Err(AppError::InvalidInput("test error".to_string()))
        });

        // A failing callback must not panic and must not stop later updates.
        listener.on_item_update(&update("FIRST"));
        listener.on_item_update(&update("SECOND"));

        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }
}
