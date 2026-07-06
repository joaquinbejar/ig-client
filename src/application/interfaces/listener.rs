/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/10/25
******************************************************************************/

use crate::error::AppError;
use lightstreamer_rs::subscription::{ItemUpdate, SubscriptionListener};
use std::fmt::{Debug, Display};
use std::sync::Arc;
use tracing::log::debug;
use tracing::{error, info};

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

impl<T> SubscriptionListener for Listener<T>
where
    T: for<'a> From<&'a ItemUpdate> + Display + Debug + 'static,
{
    fn on_item_update(&self, update: &ItemUpdate) {
        let data: T = T::from(update);

        match self.callback(&data) {
            Ok(_) => debug!("{data}"),
            Err(e) => error!("Error in trade data callback: {}", e),
        }
    }

    fn on_subscription(&mut self) {
        info!("Trade Subscription confirmed by the server");
    }
}
