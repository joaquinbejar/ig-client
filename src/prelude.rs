/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/10/25
******************************************************************************/

//! Prelude module for convenient imports
//!
//! This module re-exports commonly used types, traits, and functions
//! to make it easier to use the library.
//!
//! # Example
//! ```rust,ignore
//! use ig_client::prelude::*;
//!
//! let client = Client::try_new()?;
//! let markets = client.search_markets("EUR").await?;
//! ```
//!
//! With the default `streaming` feature enabled, the prelude is self-sufficient
//! for the streaming API too: `StreamerClient`, `DynamicMarketStreamer`, the
//! `Streaming*Field` selectors and the
//! [`PriceData`](crate::presentation::price::PriceData) DTO all resolve from a
//! single glob import, with no extra `use` lines. The `Streaming*Field`
//! selectors and the presentation DTOs are plain data types and stay available
//! even with `streaming` off; only the client types go away.
//!
//! ```rust
//! use ig_client::prelude::*;
//!
//! // Both the streaming client type and the price DTO resolve through the
//! // prelude alone — no additional imports are required.
//! fn accepts_price(_price: &PriceData) {}
//!
//! # #[cfg(feature = "streaming")]
//! fn returns_streamer(client: StreamerClient) -> StreamerClient {
//!     client
//! }
//! ```

// Core client
pub use crate::application::client::Client;

// HTTP client
pub use crate::application::http::HttpClient;

// Authentication
pub use crate::application::auth::{Auth, Session};

// Configuration
pub use crate::application::config::{
    Config, Credentials, DatabaseConfig, RateLimiterConfig, RestApiConfig, WebSocketConfig,
};

// Rate limiter
pub use crate::application::rate_limiter::{RateLimitClass, RateLimiter};

// Service interfaces
pub use crate::application::interfaces::account::AccountService;
pub use crate::application::interfaces::costs::CostsService;
#[cfg(feature = "streaming")]
pub use crate::application::interfaces::listener::ListenerResult;
pub use crate::application::interfaces::market::MarketService;
pub use crate::application::interfaces::operations::OperationsService;
pub use crate::application::interfaces::order::OrderService;
pub use crate::application::interfaces::sentiment::SentimentService;
pub use crate::application::interfaces::watchlist::WatchlistService;

// Streaming client and subscription manager (feature `streaming`)
#[cfg(feature = "streaming")]
pub use crate::application::client::StreamerClient;
#[cfg(feature = "streaming")]
pub use crate::application::dynamic_streamer::DynamicMarketStreamer;

// Error handling
pub use crate::error::AppError;

// Common presentation models
pub use crate::presentation::account::*;
pub use crate::presentation::chart::*;
pub use crate::presentation::instrument::*;
pub use crate::presentation::market::*;
pub use crate::presentation::order::*;
pub use crate::presentation::price::PriceData;
pub use crate::presentation::trade::*;
pub use crate::presentation::transaction::*;

// Request models
pub use crate::model::requests::*;

// Response models
pub use crate::model::responses::*;

// Streaming field selectors
pub use crate::model::streaming::{
    StreamingAccountDataField, StreamingChartField, StreamingMarketField, StreamingPriceField,
};

// Utility helpers
pub use crate::utils::finance::{calculate_percentage_return, calculate_pnl};
pub use crate::utils::id::get_id;
pub use crate::utils::logger::setup_logger;
pub use crate::utils::parsing::{
    ParsedMarketData, ParsedOptionInfo, normalize_text, parse_instrument_name,
};

// Re-export commonly used external types
pub use async_trait::async_trait;
pub use serde::{Deserialize, Serialize};

pub use crate::application::market_hierarchy::{
    build_market_hierarchy, extract_markets_from_hierarchy,
};
pub use crate::presentation::order::{Direction, Status};
// Persistence layer (feature `persistence`)
#[cfg(feature = "persistence")]
pub use crate::storage::market_database::MarketDatabaseService;

#[cfg(feature = "persistence")]
pub use crate::storage::utils::{create_connection_pool, create_database_config_from_env};

/// Result type alias for IG client operations
///
/// This is a convenience type alias that uses `AppError` as the error type
pub type IgResult<T> = Result<T, AppError>;
