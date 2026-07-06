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
//! let client = Client::default();
//! let markets = client.search_markets("EUR").await?;
//! ```

// Core client
pub use crate::application::client::Client;

// HTTP client
pub use crate::application::http::HttpClient;

// Authentication
pub use crate::application::auth::{Auth, Session};

// Configuration
pub use crate::application::config::{
    Config, Credentials, RateLimiterConfig, RestApiConfig, WebSocketConfig,
};

// Rate limiter
pub use crate::application::rate_limiter::{RateLimitClass, RateLimiter};

// Service interfaces
pub use crate::application::interfaces::account::AccountService;
pub use crate::application::interfaces::listener::ListenerResult;
pub use crate::application::interfaces::market::MarketService;
pub use crate::application::interfaces::order::OrderService;

// Error handling
pub use crate::error::AppError;

// Common presentation models
pub use crate::presentation::account::*;
pub use crate::presentation::chart::*;
pub use crate::presentation::instrument::*;
pub use crate::presentation::market::*;
pub use crate::presentation::order::*;
pub use crate::presentation::trade::*;
pub use crate::presentation::transaction::*;

// Request models
pub use crate::model::requests::*;

// Response models
pub use crate::model::responses::*;

pub use crate::utils::*;

// Re-export commonly used external types
pub use async_trait::async_trait;
pub use serde::{Deserialize, Serialize};

pub use crate::application::market_hierarchy::{
    build_market_hierarchy, extract_markets_from_hierarchy,
};
pub use crate::presentation::order::{Direction, Status};
pub use crate::storage::market_database::MarketDatabaseService;

pub use crate::storage::utils::{create_connection_pool, create_database_config_from_env};

/// Result type alias for IG client operations
///
/// This is a convenience type alias that uses `AppError` as the error type
pub type IgResult<T> = Result<T, AppError>;
