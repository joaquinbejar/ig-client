use crate::constants::{DAYS_TO_BACK_LOOK, DEFAULT_PAGE_SIZE, DEFAULT_SLEEP_TIME};
use crate::utils::config::get_env_or_default;
use dotenv::dotenv;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::error;
use tracing::log::debug;

/// Configuration for database connections
///
/// This is a pure configuration DTO with no I/O. It lives in the application
/// config module (alongside the other `*Config` types) so the application layer
/// can embed it in [`Config`] without depending on the storage layer. The
/// storage layer re-exports it (see `storage::config`) and owns the actual pool
/// construction (`storage::utils::create_connection_pool`).
#[derive(Serialize, Deserialize, Clone)]
pub struct DatabaseConfig {
    /// Database connection URL
    pub url: String,
    /// Maximum number of connections in the connection pool
    pub max_connections: u32,
}

// The connection `url` commonly embeds a password, so `Debug`/`Display` must
// never print it — they redact the URL and show only `max_connections`.
impl std::fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field("url", &"<redacted>")
            .field("max_connections", &self.max_connections)
            .finish()
    }
}

impl std::fmt::Display for DatabaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DatabaseConfig {{ url: <redacted>, max_connections: {} }}",
            self.max_connections
        )
    }
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
/// Authentication credentials for the IG Markets API
pub struct Credentials {
    /// Username for the IG Markets account
    pub username: String,
    /// Password for the IG Markets account
    pub password: String,
    /// Account ID for the IG Markets account
    pub account_id: String,
    /// API key for the IG Markets API
    pub api_key: String,
    /// Client token for the IG Markets API
    pub client_token: Option<String>,
    /// Account token for the IG Markets API
    pub account_token: Option<String>,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
/// Main configuration for the IG Markets API client
pub struct Config {
    /// Authentication credentials
    pub credentials: Credentials,
    /// REST API configuration
    pub rest_api: RestApiConfig,
    /// WebSocket API configuration
    pub websocket: WebSocketConfig,
    /// Database configuration for data persistence
    pub database: DatabaseConfig,
    /// Rate limiter configuration for API requests
    pub rate_limiter: RateLimiterConfig,
    /// Number of hours between transaction fetching operations
    pub sleep_hours: u64,
    /// Number of items to retrieve per page in API requests
    pub page_size: u32,
    /// Number of days to look back when fetching historical data
    pub days_to_look_back: i64,
    /// API version to use for authentication (2 or 3). If None, auto-detect based on available tokens
    pub api_version: Option<u8>,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
/// Configuration for the REST API
pub struct RestApiConfig {
    /// Base URL for the IG Markets REST API
    pub base_url: String,
    /// Timeout in seconds for REST API requests
    pub timeout: u64,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
/// Configuration for the WebSocket API
pub struct WebSocketConfig {
    /// URL for the IG Markets WebSocket API
    pub url: String,
    /// Reconnect interval in seconds for WebSocket connections
    pub reconnect_interval: u64,
}

#[derive(DebugPretty, DisplaySimple, Serialize, Deserialize, Clone)]
/// Configuration for rate limiting API requests
pub struct RateLimiterConfig {
    /// Maximum number of requests allowed per period
    pub max_requests: u32,
    /// Time period in seconds for the rate limit
    pub period_seconds: u64,
    /// Burst size - maximum number of requests that can be made at once
    pub burst_size: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// Creates a new configuration instance with a specific rate limit type
    ///
    /// # Arguments
    ///
    /// * `rate_limit_type` - The type of rate limit to enforce
    /// * `safety_margin` - A value between 0.0 and 1.0 representing the percentage of the actual limit to use
    ///
    /// # Returns
    ///
    /// A new `Config` instance
    pub fn new() -> Self {
        // Explicitly load the .env file
        match dotenv() {
            Ok(_) => debug!("Successfully loaded .env file"),
            Err(e) => debug!("Failed to load .env file: {e}"),
        }

        // Check if environment variables are configured
        let username = get_env_or_default("IG_USERNAME", String::from("default_username"));
        let password = get_env_or_default("IG_PASSWORD", String::from("default_password"));
        let api_key = get_env_or_default("IG_API_KEY", String::from("default_api_key"));
        let sleep_hours = get_env_or_default("TX_LOOP_INTERVAL_HOURS", DEFAULT_SLEEP_TIME);
        let page_size = get_env_or_default("TX_PAGE_SIZE", DEFAULT_PAGE_SIZE);
        let days_to_look_back = get_env_or_default("TX_DAYS_LOOKBACK", DAYS_TO_BACK_LOOK);

        // Check if we are using default values
        if username == "default_username" {
            error!("IG_USERNAME not found in environment variables or .env file");
        }
        if password == "default_password" {
            error!("IG_PASSWORD not found in environment variables or .env file");
        }
        if api_key == "default_api_key" {
            error!("IG_API_KEY not found in environment variables or .env file");
        }

        Config {
            credentials: Credentials {
                username,
                password,
                account_id: get_env_or_default(
                    "IG_ACCOUNT_ID",
                    String::from(crate::constants::DEFAULT_ACCOUNT_ID),
                ),
                api_key,
                client_token: None,
                account_token: None,
            },
            rest_api: RestApiConfig {
                base_url: get_env_or_default(
                    "IG_REST_BASE_URL",
                    String::from("https://demo-api.ig.com/gateway/deal"),
                ),
                timeout: get_env_or_default("IG_REST_TIMEOUT", 30),
            },
            websocket: WebSocketConfig {
                url: get_env_or_default(
                    "IG_WS_URL",
                    String::from("wss://demo-apd.marketdatasystems.com"),
                ),
                reconnect_interval: get_env_or_default("IG_WS_RECONNECT_INTERVAL", 5),
            },
            database: DatabaseConfig {
                url: get_env_or_default(
                    "DATABASE_URL",
                    String::from("postgres://postgres:postgres@localhost/ig"),
                ),
                max_connections: get_env_or_default("DATABASE_MAX_CONNECTIONS", 5),
            },
            rate_limiter: RateLimiterConfig {
                max_requests: get_env_or_default("IG_RATE_LIMIT_MAX_REQUESTS", 4), // 3
                period_seconds: get_env_or_default("IG_RATE_LIMIT_PERIOD_SECONDS", 12), // 10
                burst_size: get_env_or_default("IG_RATE_LIMIT_BURST_SIZE", 3),
            },
            sleep_hours,
            page_size,
            days_to_look_back,
            api_version: env::var("IG_API_VERSION")
                .ok()
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|&v| v == 2 || v == 3)
                .or(Some(3)), // Default to API v3 (OAuth) if not specified
        }
    }
}
