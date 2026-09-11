use crate::constants::{
    DAYS_TO_BACK_LOOK, DEFAULT_API_VERSION, DEFAULT_CONFIG_RATE_LIMIT_BURST_SIZE,
    DEFAULT_CONFIG_RATE_LIMIT_MAX_REQUESTS, DEFAULT_CONFIG_RATE_LIMIT_PERIOD_SECONDS,
    DEFAULT_DATABASE_MAX_CONNECTIONS, DEFAULT_DATABASE_URL, DEFAULT_PAGE_SIZE,
    DEFAULT_REST_BASE_URL, DEFAULT_REST_TIMEOUT_SECS, DEFAULT_SLEEP_TIME,
    DEFAULT_WS_RECONNECT_INTERVAL_SECS, DEFAULT_WS_URL,
};
use crate::utils::config::get_env_or_default;
use dotenvy::dotenv;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{debug, error};

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

#[derive(Serialize, Deserialize, Clone)]
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

// `password`, `api_key` and the tokens are secrets, so `Debug`/`Display` must
// never print them — they show `<redacted>` and leave only `username` /
// `account_id` (non-sensitive identifiers) visible.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("account_id", &self.account_id)
            .field("api_key", &"<redacted>")
            .field(
                "client_token",
                &self.client_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "account_token",
                &self.account_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl std::fmt::Display for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Credentials {{ username: {}, account_id: {}, password: <redacted>, \
             api_key: <redacted>, client_token: {}, account_token: {} }}",
            self.username,
            self.account_id,
            if self.client_token.is_some() {
                "<redacted>"
            } else {
                "None"
            },
            if self.account_token.is_some() {
                "<redacted>"
            } else {
                "None"
            },
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Main configuration for the IG Markets API client.
///
/// `Debug` is derived and delegates to each field's `Debug`, so the redacting
/// `Credentials` and `DatabaseConfig` impls keep secrets out of the output;
/// `Display` is manual for the same reason.
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
    /// API version to use for authentication: `Some(2)` for CST /
    /// X-SECURITY-TOKEN, `Some(3)` for OAuth. Both constructors set
    /// `Some(3)` ([`crate::constants::DEFAULT_API_VERSION`]); an explicit
    /// `None` makes login fall back to v2.
    pub api_version: Option<u8>,
}

// Manual `Display` (replacing the derived, serde-based `DisplaySimple`, which
// would serialize the whole tree including credential secrets). It delegates to
// the nested configs' own `Display` impls — `Credentials` and `DatabaseConfig`
// redact their secrets — and shows only non-sensitive scalars directly.
impl std::fmt::Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Config {{ credentials: {}, rest_api: {}, websocket: {}, database: {}, \
             rate_limiter: {}, sleep_hours: {}, page_size: {}, days_to_look_back: {}, \
             api_version: {:?} }}",
            self.credentials,
            self.rest_api,
            self.websocket,
            self.database,
            self.rate_limiter,
            self.sleep_hours,
            self.page_size,
            self.days_to_look_back,
            self.api_version,
        )
    }
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

// The `Default` impls below are the single source of truth for the
// non-credential defaults: they hold the literals and `Config::new()` uses them
// as its `get_env_or_default` fallbacks. They read no environment variable and
// load no `.env` file, so they are usable from the injection path
// ([`Config::from_credentials`] / `Client::with_config`).

impl Default for RestApiConfig {
    /// IG **demo** REST gateway with a 30 second request timeout.
    fn default() -> Self {
        Self {
            base_url: String::from(DEFAULT_REST_BASE_URL),
            timeout: DEFAULT_REST_TIMEOUT_SECS,
        }
    }
}

impl Default for WebSocketConfig {
    /// IG **demo** Lightstreamer endpoint with a 5 second reconnect interval.
    fn default() -> Self {
        Self {
            url: String::from(DEFAULT_WS_URL),
            reconnect_interval: DEFAULT_WS_RECONNECT_INTERVAL_SECS,
        }
    }
}

impl Default for RateLimiterConfig {
    /// The crate-wide non-trading budget: 4 requests per 12 seconds, burst 3.
    fn default() -> Self {
        Self {
            max_requests: DEFAULT_CONFIG_RATE_LIMIT_MAX_REQUESTS,
            period_seconds: DEFAULT_CONFIG_RATE_LIMIT_PERIOD_SECONDS,
            burst_size: DEFAULT_CONFIG_RATE_LIMIT_BURST_SIZE,
        }
    }
}

impl Default for DatabaseConfig {
    /// Credential-less placeholder URL — persistence will not connect until the
    /// caller supplies a real one.
    fn default() -> Self {
        Self {
            url: String::from(DEFAULT_DATABASE_URL),
            max_connections: DEFAULT_DATABASE_MAX_CONNECTIONS,
        }
    }
}

impl Credentials {
    /// Builds credentials from the four required fields, leaving both session
    /// tokens unset.
    ///
    /// The tokens (`client_token` / `account_token`) are populated by the
    /// session layer on login, so callers never provide them.
    ///
    /// # Arguments
    ///
    /// * `username` - IG account username
    /// * `password` - IG account password
    /// * `account_id` - IG account identifier
    /// * `api_key` - IG API key
    #[must_use]
    pub fn new(username: String, password: String, account_id: String, api_key: String) -> Self {
        Self {
            username,
            password,
            account_id,
            api_key,
            client_token: None,
            account_token: None,
        }
    }

    /// Splits [`api_key`](Self::api_key) into the individual keys of a pool.
    ///
    /// IG enforces its non-trading allowance **per API key**, so a caller that
    /// owns several keys on the same account can raise its aggregate throughput
    /// by spreading requests across them. To express that, `api_key` accepts a
    /// comma-separated list; a single key (no comma) yields a one-element pool,
    /// which is the historical behaviour.
    ///
    /// Empty entries and surrounding whitespace are discarded, so
    /// `"a, b, ,c,"` yields `["a", "b", "c"]`. When the field holds nothing
    /// usable the result is empty and the caller decides how to fail.
    #[must_use]
    pub fn api_keys(&self) -> Vec<String> {
        self.api_key
            .split(',')
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(String::from)
            .collect()
    }
}

impl Default for Config {
    /// Delegates to [`Config::new`] and therefore loads a `.env` file and reads
    /// the `IG_*` namespace — unlike the section-level `Default` impls above,
    /// which are env-free. `Config { .., ..Config::default() }` still touches
    /// the environment; use [`Config::from_credentials`] as the base value when
    /// that is not acceptable.
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// Creates a configuration from caller-supplied credentials, reading **no**
    /// environment variable and loading **no** `.env` file.
    ///
    /// This is the injection path for embedding applications that own their
    /// configuration source (their own namespaced env vars, a config file, a
    /// secrets manager). Pair it with
    /// [`Client::with_config`](crate::application::client::Client::with_config).
    /// [`Config::new`] remains the `.env` / `IG_*` convenience path.
    ///
    /// Every non-credential field takes its documented default (IG **demo**
    /// endpoints — see the `Default` impls of [`RestApiConfig`],
    /// [`WebSocketConfig`], [`RateLimiterConfig`] and [`DatabaseConfig`]).
    /// Override individual sections with a struct-update expression, which
    /// stays env-free because the base value is this constructor:
    ///
    /// ```rust
    /// use ig_client::prelude::*;
    ///
    /// let credentials = Credentials::new(
    ///     "user".to_string(),
    ///     "password".to_string(),
    ///     "ABC123".to_string(),
    ///     "api-key".to_string(),
    /// );
    /// let config = Config {
    ///     rest_api: RestApiConfig {
    ///         base_url: "https://demo-api.ig.com/gateway/deal".to_string(),
    ///         timeout: 30,
    ///     },
    ///     ..Config::from_credentials(credentials)
    /// };
    /// assert_eq!(config.rest_api.base_url, "https://demo-api.ig.com/gateway/deal");
    /// ```
    ///
    /// # Arguments
    ///
    /// * `credentials` - IG credentials supplied by the caller
    ///
    /// # Returns
    ///
    /// A `Config` built entirely from `credentials` plus the documented defaults
    #[must_use]
    pub fn from_credentials(credentials: Credentials) -> Self {
        Config {
            credentials,
            rest_api: RestApiConfig::default(),
            websocket: WebSocketConfig::default(),
            database: DatabaseConfig::default(),
            rate_limiter: RateLimiterConfig::default(),
            sleep_hours: DEFAULT_SLEEP_TIME,
            page_size: DEFAULT_PAGE_SIZE,
            days_to_look_back: DAYS_TO_BACK_LOOK,
            api_version: Some(DEFAULT_API_VERSION),
        }
    }

    /// Creates a new configuration instance from the environment.
    ///
    /// Loads a local `.env` file (via `dotenvy`) and reads the `IG_*` /
    /// `DATABASE_*` / `TX_*` environment variables, falling back to the
    /// documented defaults for anything unset. Embedders that must not touch
    /// the `.env` file or the `IG_*` namespace should use
    /// [`Config::from_credentials`] instead.
    ///
    /// # Returns
    ///
    /// A new `Config` instance
    pub fn new() -> Self {
        // Explicitly load the .env file
        match dotenv() {
            Ok(_) => debug!("Successfully loaded .env file"),
            // dotenvy parse errors can contain the original line and its secrets.
            Err(_) => debug!("Failed to load .env file"),
        }

        // Check if environment variables are configured
        let username = get_env_or_default("IG_USERNAME", String::from("default_username"));
        let password = get_env_or_default("IG_PASSWORD", String::from("default_password"));
        let api_key = get_env_or_default("IG_API_KEY", String::from("default_api_key"));
        let sleep_hours = get_env_or_default("TX_LOOP_INTERVAL_HOURS", DEFAULT_SLEEP_TIME);
        let page_size = get_env_or_default("TX_PAGE_SIZE", DEFAULT_PAGE_SIZE);
        let days_to_look_back = get_env_or_default("TX_DAYS_LOOKBACK", DAYS_TO_BACK_LOOK);

        // Defaults come from the `Default` impls so the env path and the
        // env-free path (`from_credentials`) cannot drift apart.
        let rest_defaults = RestApiConfig::default();
        let ws_defaults = WebSocketConfig::default();
        let rate_limit_defaults = RateLimiterConfig::default();
        let database_defaults = DatabaseConfig::default();

        let database_url = get_env_or_default("DATABASE_URL", database_defaults.url);

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
        // Check the variable directly rather than comparing the resolved value
        // to the placeholder: a user may intentionally set a credential-less URL
        // equal to the placeholder, which is not the "unset" case we warn about.
        if env::var("DATABASE_URL").is_err() {
            // Falls back to the credential-less placeholder; persistence will not
            // connect until DATABASE_URL is set. We never use a credentialed default.
            error!(
                "DATABASE_URL not found in environment variables or .env file; \
                 using a credential-less placeholder and persistence will not connect"
            );
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
                base_url: get_env_or_default("IG_REST_BASE_URL", rest_defaults.base_url),
                timeout: get_env_or_default("IG_REST_TIMEOUT", rest_defaults.timeout),
            },
            websocket: WebSocketConfig {
                url: get_env_or_default("IG_WS_URL", ws_defaults.url),
                reconnect_interval: get_env_or_default(
                    "IG_WS_RECONNECT_INTERVAL",
                    ws_defaults.reconnect_interval,
                ),
            },
            database: DatabaseConfig {
                url: database_url,
                max_connections: get_env_or_default(
                    "DATABASE_MAX_CONNECTIONS",
                    database_defaults.max_connections,
                ),
            },
            rate_limiter: RateLimiterConfig {
                max_requests: get_env_or_default(
                    "IG_RATE_LIMIT_MAX_REQUESTS",
                    rate_limit_defaults.max_requests,
                ),
                period_seconds: get_env_or_default(
                    "IG_RATE_LIMIT_PERIOD_SECONDS",
                    rate_limit_defaults.period_seconds,
                ),
                burst_size: get_env_or_default(
                    "IG_RATE_LIMIT_BURST_SIZE",
                    rate_limit_defaults.burst_size,
                ),
            },
            sleep_hours,
            page_size,
            days_to_look_back,
            api_version: env::var("IG_API_VERSION")
                .ok()
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|&v| v == 2 || v == 3)
                .or(Some(DEFAULT_API_VERSION)), // Default to API v3 (OAuth) if not specified
        }
    }
}

#[cfg(test)]
mod injection_tests {
    use super::*;

    fn injected_credentials() -> Credentials {
        Credentials::new(
            "embedder-user".to_string(),
            "embedder-password".to_string(),
            "EMBEDDER-ACC".to_string(),
            "embedder-api-key".to_string(),
        )
    }

    #[test]
    fn test_credentials_new_leaves_session_tokens_unset() {
        let credentials = injected_credentials();
        assert_eq!(credentials.username, "embedder-user");
        assert_eq!(credentials.password, "embedder-password");
        assert_eq!(credentials.account_id, "EMBEDDER-ACC");
        assert_eq!(credentials.api_key, "embedder-api-key");
        assert!(credentials.client_token.is_none());
        assert!(credentials.account_token.is_none());
    }

    #[test]
    fn test_config_from_credentials_keeps_injected_credentials_and_env_free_defaults() {
        // No environment is mutated here (`set_var` is `unsafe` on edition 2024
        // and racy under the parallel harness), so this asserts the contract
        // rather than proving env-independence by construction: the injected
        // credentials survive and every other field equals its documented
        // default. The end-to-end env-independence check lives in
        // `tests/unit/application/test_client.rs`, where the injected values
        // cannot coincide with anything the environment holds.
        let config = Config::from_credentials(injected_credentials());

        assert_eq!(config.credentials.username, "embedder-user");
        assert_eq!(config.credentials.api_key, "embedder-api-key");
        assert_eq!(config.rest_api.base_url, DEFAULT_REST_BASE_URL);
        assert_eq!(config.rest_api.timeout, DEFAULT_REST_TIMEOUT_SECS);
        assert_eq!(config.websocket.url, DEFAULT_WS_URL);
        assert_eq!(
            config.websocket.reconnect_interval,
            DEFAULT_WS_RECONNECT_INTERVAL_SECS
        );
        assert_eq!(config.database.url, DEFAULT_DATABASE_URL);
        assert_eq!(
            config.database.max_connections,
            DEFAULT_DATABASE_MAX_CONNECTIONS
        );
        assert_eq!(
            config.rate_limiter.max_requests,
            DEFAULT_CONFIG_RATE_LIMIT_MAX_REQUESTS
        );
        assert_eq!(
            config.rate_limiter.period_seconds,
            DEFAULT_CONFIG_RATE_LIMIT_PERIOD_SECONDS
        );
        assert_eq!(
            config.rate_limiter.burst_size,
            DEFAULT_CONFIG_RATE_LIMIT_BURST_SIZE
        );
        assert_eq!(config.sleep_hours, DEFAULT_SLEEP_TIME);
        assert_eq!(config.page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(config.days_to_look_back, DAYS_TO_BACK_LOOK);
        assert_eq!(config.api_version, Some(DEFAULT_API_VERSION));
    }

    #[test]
    fn test_config_from_credentials_struct_update_overrides_section() {
        // The documented override pattern must not fall back to `Config::new()`
        // (which would run `dotenv()`); the base value is the env-free ctor.
        let config = Config {
            rest_api: RestApiConfig {
                base_url: "https://demo-api.ig.com/gateway/deal".to_string(),
                timeout: 7,
            },
            ..Config::from_credentials(injected_credentials())
        };

        assert_eq!(
            config.rest_api.base_url,
            "https://demo-api.ig.com/gateway/deal"
        );
        assert_eq!(config.rest_api.timeout, 7);
        // Untouched sections keep their env-free defaults.
        assert_eq!(config.websocket.url, DEFAULT_WS_URL);
    }

    #[test]
    fn test_section_defaults_match_documented_constants() {
        // Guards the refactor that made `Config::new()` use these `Default`
        // impls as its env fallbacks: the two paths must not drift apart.
        let rest = RestApiConfig::default();
        let ws = WebSocketConfig::default();
        let rate_limiter = RateLimiterConfig::default();
        let database = DatabaseConfig::default();

        assert_eq!(rest.base_url, DEFAULT_REST_BASE_URL);
        assert_eq!(rest.timeout, DEFAULT_REST_TIMEOUT_SECS);
        assert_eq!(ws.url, DEFAULT_WS_URL);
        assert_eq!(ws.reconnect_interval, DEFAULT_WS_RECONNECT_INTERVAL_SECS);
        assert_eq!(
            rate_limiter.max_requests,
            DEFAULT_CONFIG_RATE_LIMIT_MAX_REQUESTS
        );
        assert_eq!(
            rate_limiter.period_seconds,
            DEFAULT_CONFIG_RATE_LIMIT_PERIOD_SECONDS
        );
        assert_eq!(
            rate_limiter.burst_size,
            DEFAULT_CONFIG_RATE_LIMIT_BURST_SIZE
        );
        assert_eq!(database.url, DEFAULT_DATABASE_URL);
        assert_eq!(database.max_connections, DEFAULT_DATABASE_MAX_CONNECTIONS);
        // The rate-limiter default is NOT the zero-burst fallback constant.
        assert_ne!(
            rate_limiter.burst_size,
            crate::constants::DEFAULT_RATE_LIMIT_BURST_SIZE
        );
    }
}

#[cfg(test)]
mod redaction_tests {
    use super::*;

    fn secret_credentials() -> Credentials {
        Credentials {
            username: "user@example.com".to_string(),
            password: "SUPER-SECRET-PASSWORD".to_string(),
            account_id: "ACC123".to_string(),
            api_key: "SECRET-API-KEY".to_string(),
            client_token: Some("SECRET-CST".to_string()),
            account_token: Some("SECRET-XST".to_string()),
        }
    }

    #[test]
    fn test_credentials_debug_and_display_redact_secrets() {
        let creds = secret_credentials();
        for rendered in [format!("{creds:?}"), format!("{creds}")] {
            for secret in [
                "SUPER-SECRET-PASSWORD",
                "SECRET-API-KEY",
                "SECRET-CST",
                "SECRET-XST",
            ] {
                assert!(
                    !rendered.contains(secret),
                    "credentials rendering leaked {secret}: {rendered}"
                );
            }
            assert!(rendered.contains("<redacted>"));
            // Non-sensitive identifiers stay visible.
            assert!(rendered.contains("user@example.com"));
            assert!(rendered.contains("ACC123"));
        }
    }

    #[test]
    fn test_config_debug_and_display_redact_credential_and_db_secrets() {
        let config = Config {
            credentials: secret_credentials(),
            database: DatabaseConfig {
                url: "postgres://dbuser:DB-SECRET-PW@host/db".to_string(),
                max_connections: 5,
            },
            ..Config::default()
        };
        for rendered in [format!("{config:?}"), format!("{config}")] {
            for secret in ["SUPER-SECRET-PASSWORD", "SECRET-API-KEY", "DB-SECRET-PW"] {
                assert!(
                    !rendered.contains(secret),
                    "config rendering leaked {secret}: {rendered}"
                );
            }
            assert!(rendered.contains("<redacted>"));
        }
    }

    #[test]
    fn test_api_keys_single_key_yields_one_element_pool() {
        let c = Credentials::new("u".into(), "p".into(), "ACC".into(), "abc123".into());
        assert_eq!(c.api_keys(), vec!["abc123".to_string()]);
    }

    #[test]
    fn test_api_keys_comma_separated_list_yields_pool() {
        let c = Credentials::new("u".into(), "p".into(), "ACC".into(), "a,b,c".into());
        assert_eq!(
            c.api_keys(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn test_api_keys_trims_whitespace_and_drops_empty_entries() {
        let c = Credentials::new("u".into(), "p".into(), "ACC".into(), " a , b , ,c, ".into());
        assert_eq!(
            c.api_keys(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn test_api_keys_blank_value_yields_empty_pool() {
        let c = Credentials::new("u".into(), "p".into(), "ACC".into(), "  ,  ".into());
        assert!(c.api_keys().is_empty());
    }
}
