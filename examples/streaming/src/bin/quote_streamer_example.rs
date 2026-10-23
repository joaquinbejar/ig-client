/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 23/10/26
******************************************************************************/

//! Example demonstrating `QuoteStreamer`: live quotes subscribed on demand.
//!
//! Each EPIC on the command line is quoted once; the first request
//! subscribes and waits for IG's snapshot, the second reads the streamed
//! quote at once. At most `DEFAULT_QUOTE_CAPACITY` instruments stay
//! subscribed; asking for more drops the least recently used.
//!
//! ```text
//! cargo run --bin quote_streamer_example -- OP.D.OTCDAX1.24000C.IP IX.D.DAX.DAILY.IP
//! ```

use ig_client::prelude::*;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    setup_logger();

    let epics: Vec<String> = std::env::args().skip(1).collect();
    let epics = if epics.is_empty() {
        vec!["IX.D.DAX.DAILY.IP".to_string()]
    } else {
        epics
    };

    let capacity = NonZeroUsize::new(DEFAULT_QUOTE_CAPACITY)
        .ok_or_else(|| AppError::InvalidInput("capacity must be positive".to_string()))?;
    let streamer = QuoteStreamer::new(Client::try_new()?, capacity);

    for round in 1..=2 {
        for epic in &epics {
            let started = Instant::now();
            match streamer.quote(epic, Duration::from_secs(2)).await? {
                Some(quote) => info!(
                    round,
                    epic,
                    bid = ?quote.bid,
                    offer = ?quote.offer,
                    flag = ?quote.dealing_flag,
                    ms = started.elapsed().as_millis(),
                    "quote"
                ),
                None => info!(round, epic, "no streamed quote in time"),
            }
        }
    }
    info!(subscribed = ?streamer.epics().await, "subscriptions");

    streamer.close().await
}
