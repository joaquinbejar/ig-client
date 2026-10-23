/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 23/10/26
******************************************************************************/
//! Live quotes for a rotating set of instruments, within IG's streaming cap.
//!
//! IG caps how many instruments one account can stream at once (about
//! thirty), so a service that prices whichever option it is about to trade
//! cannot subscribe to everything it might need. [`QuoteStreamer`] subscribes
//! on demand: the first request for an EPIC opens a `MERGE` subscription to
//! its `PRICE:{account}:{epic}` item on the `Pricing` data adapter, whose
//! snapshot is the current quote, and later requests read the latest update
//! without asking IG anything.
//!
//! The pricing item rather than `MARKET:{epic}`: on 2026-10-23 IG's demo
//! servers refused every `MARKET:` group with "invalid group" (code 21),
//! options and indices alike, while the pricing item of the same option
//! answered with its snapshot at once. When the cap is
//! reached, the instrument used least recently is unsubscribed to make room.
//!
//! Unlike [`DynamicMarketStreamer`](crate::application::dynamic_streamer::DynamicMarketStreamer),
//! which re-subscribes its whole list on every change, adding or dropping one
//! instrument here is one control request on the open session: the other
//! subscriptions keep flowing.
//!
//! A streamed quote does not count against the REST API allowance, which is
//! the point: a quote read through `GET /markets/{epic}` costs one request
//! from a budget the whole application shares.

use crate::application::client::Client;
use crate::application::streaming_convert::StreamingUpdate;
use crate::error::AppError;
use crate::model::streaming::{StreamingPriceField, get_streaming_price_fields};
use crate::presentation::price::PriceData;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use lightstreamer_rs::{
    Client as LsClient, ClientConfig, Credentials, FieldSchema, ItemGroup, ServerAddress,
    SessionEvent, Snapshot, Subscription, SubscriptionEvent, SubscriptionMode,
};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// IG's documented cap on the instruments one account streams at once.
pub const DEFAULT_QUOTE_CAPACITY: usize = 30;

/// The latest quote of one instrument, as the stream delivered it.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamQuote {
    /// The instrument.
    pub epic: String,
    /// Best bid, when IG quotes one.
    pub bid: Option<f64>,
    /// Best offer, when IG quotes one.
    pub offer: Option<f64>,
    /// IG's dealing flag (`DEAL`, `CLOSED`, `CALL`, `EDIT`…).
    pub dealing_flag: Option<String>,
    /// IG's own time of the last update, in epoch milliseconds.
    pub timestamp: Option<i64>,
    /// When this process received the update.
    pub received_at: DateTime<Utc>,
}

impl StreamQuote {
    /// The quote after `update`: fields the update carries replace the old
    /// ones, fields it leaves out keep their value, as `MERGE` mode means.
    #[must_use]
    pub fn merged(previous: Option<&Self>, epic: &str, update: &PriceData) -> Self {
        let fields = &update.fields;
        let flag = fields.dealing_flag.as_ref().and_then(|flag| {
            serde_json::to_value(flag)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
        });
        Self {
            epic: epic.to_string(),
            bid: fields
                .bid_price1
                .or(fields.bid)
                .or_else(|| previous.and_then(|p| p.bid)),
            offer: fields
                .ask_price1
                .or(fields.offer)
                .or_else(|| previous.and_then(|p| p.offer)),
            dealing_flag: flag.or_else(|| previous.and_then(|p| p.dealing_flag.clone())),
            timestamp: fields
                .timestamp
                .or_else(|| previous.and_then(|p| p.timestamp)),
            received_at: Utc::now(),
        }
    }

    /// How old the quote is.
    #[must_use]
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.received_at
    }
}

/// The instrument to drop to make room: the one used least recently.
#[must_use]
pub fn least_recently_used<'a, I>(entries: I) -> Option<String>
where
    I: IntoIterator<Item = (&'a String, Instant)>,
{
    entries
        .into_iter()
        .min_by_key(|(_, last_used)| *last_used)
        .map(|(epic, _)| epic.clone())
}

/// One subscribed instrument.
struct Entry {
    /// The latest quote; `None` until the snapshot arrives.
    latest: watch::Receiver<Option<StreamQuote>>,
    /// The task that owns the subscription. Dropping its update stream, by
    /// stopping it, is what unsubscribes.
    task: JoinHandle<()>,
    /// When a caller last asked for this instrument.
    last_used: Instant,
}

/// The open session, with the task draining its events.
struct Session {
    client: LsClient,
    /// The account the session streams for: pricing items name it.
    account_id: String,
    events: JoinHandle<()>,
    /// Turns `false` once the session is closed for good.
    alive: watch::Receiver<bool>,
}

/// Where the streamer is.
enum State {
    /// No session yet, or the last one ended.
    Idle,
    /// A session is open.
    Open(Session),
    /// [`QuoteStreamer::close`] was called; nothing more is opened.
    Closed,
}

struct Inner {
    client: Client,
    capacity: NonZeroUsize,
    state: Mutex<(State, HashMap<String, Entry>)>,
    shutdown_tx: watch::Sender<bool>,
}

/// Live quotes for up to `capacity` instruments, subscribed on demand.
///
/// Cloning is cheap and every clone shares the session and the
/// subscriptions. Call [`close`](Self::close) to unsubscribe and close the
/// session; every task it spawned stops then.
#[derive(Clone)]
pub struct QuoteStreamer {
    inner: Arc<Inner>,
}

impl QuoteStreamer {
    /// A streamer over `client`'s session, holding at most `capacity`
    /// instruments at once. Nothing is opened until the first quote is
    /// asked for.
    #[must_use]
    pub fn new(client: Client, capacity: NonZeroUsize) -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            inner: Arc::new(Inner {
                client,
                capacity,
                state: Mutex::new((State::Idle, HashMap::new())),
                shutdown_tx,
            }),
        }
    }

    /// The instruments subscribed right now.
    pub async fn epics(&self) -> Vec<String> {
        let state = self.inner.state.lock().await;
        state.1.keys().cloned().collect()
    }

    /// The latest quote of `epic`, subscribing to it if it is not yet.
    ///
    /// A freshly subscribed instrument has no quote until IG sends its
    /// snapshot; the call waits up to `wait` for it. `Ok(None)` means none
    /// arrived in time (or IG refused the subscription, which is logged): the
    /// caller should fall back to the REST quote.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::WebSocketError`] when the session cannot be opened
    /// or the subscription cannot be sent, and [`AppError::InvalidInput`] for
    /// an EPIC Lightstreamer rejects as an item name.
    pub async fn quote(&self, epic: &str, wait: Duration) -> Result<Option<StreamQuote>, AppError> {
        let mut latest = self.subscription(epic).await?;
        if let Some(quote) = latest.borrow().clone() {
            return Ok(Some(quote));
        }
        let arrived = tokio::time::timeout(wait, latest.wait_for(Option::is_some)).await;
        Ok(match arrived {
            Ok(Ok(quote)) => quote.clone(),
            // The subscription ended (refused, or the session closed) or the
            // snapshot is late: either way there is no quote to give.
            Ok(Err(_)) | Err(_) => None,
        })
    }

    /// The receiver of `epic`'s quotes, subscribing when needed.
    async fn subscription(
        &self,
        epic: &str,
    ) -> Result<watch::Receiver<Option<StreamQuote>>, AppError> {
        let mut guard = self.inner.state.lock().await;
        let (state, entries) = &mut *guard;

        // A session that ended takes its subscriptions with it.
        let alive = match state {
            State::Open(session) => *session.alive.borrow(),
            State::Idle => false,
            State::Closed => {
                return Err(AppError::WebSocketError(
                    "the quote streamer is closed".to_string(),
                ));
            }
        };
        if !alive {
            for (_, entry) in entries.drain() {
                entry.task.abort();
            }
            if let State::Open(session) = std::mem::replace(state, State::Idle) {
                session.events.abort();
            }
            *state = State::Open(self.open_session().await?);
        }
        let State::Open(session) = state else {
            return Err(AppError::WebSocketError(
                "streaming session not initialized".to_string(),
            ));
        };

        // A subscription whose task ended (refused by IG) is tried afresh.
        if entries
            .get(epic)
            .is_some_and(|entry| entry.task.is_finished())
        {
            entries.remove(epic);
        }
        if let Some(entry) = entries.get_mut(epic) {
            entry.last_used = Instant::now();
            return Ok(entry.latest.clone());
        }

        while entries.len() >= self.inner.capacity.get() {
            let Some(victim) =
                least_recently_used(entries.iter().map(|(epic, e)| (epic, e.last_used)))
            else {
                break;
            };
            if let Some(entry) = entries.remove(&victim) {
                entry.task.abort();
                debug!(epic = %victim, "quote subscription dropped to make room");
            }
        }

        let fields = HashSet::from([
            StreamingPriceField::BidPrice1,
            StreamingPriceField::AskPrice1,
            StreamingPriceField::DlgFlag,
            StreamingPriceField::Timestamp,
        ]);
        // The data adapter's name is server configuration, overridable the
        // way `StreamerClient::price_subscribe` allows.
        let adapter = std::env::var("IG_PRICING_ADAPTER").unwrap_or_else(|_| "Pricing".to_string());
        let subscription = Subscription::new(
            SubscriptionMode::Merge,
            ItemGroup::from_items(vec![format!("PRICE:{}:{epic}", session.account_id)])?,
            FieldSchema::from_fields(get_streaming_price_fields(&fields))?,
        )
        .with_data_adapter(adapter)
        .with_snapshot(Snapshot::On);
        let mut updates = session.client.subscribe(subscription).await?;

        let (tx, rx) = watch::channel(None::<StreamQuote>);
        let mut shutdown = self.inner.shutdown_tx.subscribe();
        let name = epic.to_string();
        let task = tokio::spawn(async move {
            loop {
                let event = tokio::select! {
                    _ = shutdown.changed() => return,
                    event = updates.next() => event,
                };
                let Some(event) = event else { return };
                match event {
                    SubscriptionEvent::Update(update) => {
                        let data = PriceData::from(&StreamingUpdate::from(update.as_ref()));
                        let next = StreamQuote::merged(tx.borrow().as_ref(), &name, &data);
                        if tx.send(Some(next)).is_err() {
                            return;
                        }
                    }
                    SubscriptionEvent::Rejected(e) => {
                        warn!(epic = %name, error = %e, "IG refused the quote subscription");
                        return;
                    }
                    SubscriptionEvent::Unsubscribed => return,
                    other => debug!(epic = %name, event = ?other, "quote subscription event"),
                }
            }
        });
        entries.insert(
            epic.to_string(),
            Entry {
                latest: rx.clone(),
                task,
                last_used: Instant::now(),
            },
        );
        debug!(epic, subscribed = entries.len(), "quote subscription added");
        Ok(rx)
    }

    /// Opens a session with the client's current streaming credentials.
    ///
    /// Asked for afresh on every open: the Lightstreamer password is the
    /// REST session's token pair, which IG rotates.
    async fn open_session(&self) -> Result<Session, AppError> {
        let ws_info = self.inner.client.ws_info().await?;
        let config = ClientConfig::builder(ServerAddress::try_new(ws_info.server.as_str())?)
            .with_credentials(Credentials::new(
                ws_info.account_id.as_str(),
                ws_info.get_ws_password(),
            ))
            .build()?;
        let (client, mut events) = LsClient::connect(config).await?;
        info!(account_id = %ws_info.account_id, "quote streaming session opened");

        let (alive_tx, alive) = watch::channel(true);
        let mut shutdown = self.inner.shutdown_tx.subscribe();
        let events = tokio::spawn(async move {
            loop {
                let event = tokio::select! {
                    _ = shutdown.changed() => break,
                    event = events.next() => event,
                };
                match event {
                    None => break,
                    Some(SessionEvent::Closed(reason)) => {
                        warn!(?reason, "quote streaming session closed");
                        break;
                    }
                    Some(SessionEvent::Disconnected { reason, retry_in }) => {
                        warn!(?reason, ?retry_in, "quote streaming session disconnected");
                    }
                    Some(SessionEvent::RequestRejected(e)) => {
                        warn!(error = %e, "IG refused a quote streaming control request");
                    }
                    Some(other) => debug!(event = ?other, "quote streaming session event"),
                }
            }
            let _ = alive_tx.send(false);
        });
        Ok(Session {
            client,
            account_id: ws_info.account_id.clone(),
            events,
            alive,
        })
    }

    /// Unsubscribes everything and closes the session. Later quotes fail.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::WebSocketError`] when the session does not close
    /// cleanly; the tasks are stopped either way.
    pub async fn close(&self) -> Result<(), AppError> {
        let _ = self.inner.shutdown_tx.send(true);
        let mut guard = self.inner.state.lock().await;
        let (state, entries) = &mut *guard;
        for (_, entry) in entries.drain() {
            let _ = entry.task.await;
        }
        if let State::Open(session) = std::mem::replace(state, State::Closed) {
            let _ = session.events.await;
            session.client.disconnect().await.map_err(|e| {
                error!(error = %e, "quote streaming session did not close cleanly");
                AppError::from(e)
            })?;
            info!("quote streaming session closed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::price::PriceFields;

    fn update(bid: Option<f64>, offer: Option<f64>, timestamp: Option<i64>) -> PriceData {
        PriceData {
            fields: PriceFields {
                bid_price1: bid,
                ask_price1: offer,
                timestamp,
                ..PriceFields::default()
            },
            ..PriceData::default()
        }
    }

    #[test]
    fn a_merge_update_keeps_the_fields_it_leaves_out() {
        let first = StreamQuote::merged(
            None,
            "DO.D.OTCDDAX.105.IP",
            &update(Some(68.9), Some(72.0), Some(1_792_731_600_000)),
        );
        assert_eq!(
            (first.bid, first.offer, first.timestamp),
            (Some(68.9), Some(72.0), Some(1_792_731_600_000))
        );
        let next = StreamQuote::merged(
            Some(&first),
            "DO.D.OTCDDAX.105.IP",
            &update(Some(69.4), None, None),
        );
        assert_eq!(next.bid, Some(69.4));
        assert_eq!(next.offer, Some(72.0));
        assert_eq!(next.timestamp, Some(1_792_731_600_000));
    }

    #[test]
    fn the_least_recently_used_instrument_makes_room() {
        let now = Instant::now();
        let (a, b, c) = ("A".to_string(), "B".to_string(), "C".to_string());
        let entries = [
            (&a, now),
            (&b, now - Duration::from_secs(30)),
            (&c, now - Duration::from_secs(5)),
        ];
        assert_eq!(least_recently_used(entries), Some("B".to_string()));
        assert_eq!(least_recently_used(Vec::<(&String, Instant)>::new()), None);
    }
}
