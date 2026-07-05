/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 19/10/25
******************************************************************************/

//! Authentication module for IG Markets API
//!
//! This module provides a simplified authentication interface that handles:
//! - API v2 (CST/X-SECURITY-TOKEN) authentication
//! - API v3 (OAuth) authentication with automatic token refresh
//! - Account switching
//! - Automatic re-authentication when tokens expire

use crate::application::config::Config;
use crate::application::rate_limiter::RateLimiter;
use crate::error::AppError;
pub(crate) use crate::model::auth::{OAuthToken, SecurityHeaders, SessionResponse};
use crate::model::http::make_http_request;
use crate::model::retry::RetryConfig;
use chrono::Utc;
use reqwest::{Client, Method};
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

const USER_AGENT: &str = "ig-client/0.6.0";

/// WebSocket connection information for Lightstreamer
///
/// Contains the necessary credentials and endpoint information
/// to establish a WebSocket connection to IG's Lightstreamer service.
#[derive(Clone, Default)]
pub struct WebsocketInfo {
    /// Lightstreamer endpoint URL
    pub server: String,
    /// CST token for authentication (API v2)
    pub cst: Option<String>,
    /// X-SECURITY-TOKEN for authentication (API v2)
    pub x_security_token: Option<String>,
    /// Account ID for the WebSocket connection
    pub account_id: String,
}

/// Renders an optional secret as `Some(<redacted>)` / `None`, never exposing
/// the underlying token value in `Debug` / `Display` output or logs.
struct RedactedOption<'a>(&'a Option<String>);

impl fmt::Debug for RedactedOption<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(_) => f.write_str("Some(<redacted>)"),
            None => f.write_str("None"),
        }
    }
}

// Manual redacting `Debug` — `cst` / `x_security_token` are credentials and
// must never reach logs or panics. All non-secret fields stay visible.
impl fmt::Debug for WebsocketInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebsocketInfo")
            .field("server", &self.server)
            .field("cst", &RedactedOption(&self.cst))
            .field("x_security_token", &RedactedOption(&self.x_security_token))
            .field("account_id", &self.account_id)
            .finish()
    }
}

// Manual redacting `Display` — the derived `DisplaySimple` serializes via serde
// and would leak the tokens, so it is replaced with a masking implementation.
impl fmt::Display for WebsocketInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WebsocketInfo {{ server: {}, cst: {:?}, x_security_token: {:?}, account_id: {} }}",
            self.server,
            RedactedOption(&self.cst),
            RedactedOption(&self.x_security_token),
            self.account_id,
        )
    }
}

impl WebsocketInfo {
    /// Generates the WebSocket password for Lightstreamer authentication
    ///
    /// # Returns
    /// * Password in format "CST-{cst}|XST-{token}" if both tokens are available
    /// * Empty string if tokens are not available
    #[must_use]
    pub fn get_ws_password(&self) -> String {
        match (&self.cst, &self.x_security_token) {
            (Some(cst), Some(x_security_token)) => {
                format!("CST-{}|XST-{}", cst, x_security_token)
            }
            _ => String::new(),
        }
    }
}

/// Session information for authenticated requests
#[derive(Clone)]
pub struct Session {
    /// Account ID
    pub account_id: String,
    /// Client ID (for OAuth)
    pub client_id: String,
    /// Lightstreamer endpoint
    pub lightstreamer_endpoint: String,
    /// CST token (API v2)
    pub cst: Option<String>,
    /// X-SECURITY-TOKEN (API v2)
    pub x_security_token: Option<String>,
    /// OAuth token (API v3)
    pub oauth_token: Option<OAuthToken>,
    /// API version used
    pub api_version: u8,
    /// Unix timestamp when session expires (seconds since epoch)
    /// - OAuth (v3): expires in 30 seconds
    /// - API v2: expires in 6 hours (21600 seconds)
    pub expires_at: u64,
}

// Manual redacting `Debug` — `cst`, `x_security_token` and the OAuth token are
// credentials. The `OAuthToken` `Debug` impl is itself redacting, so printing
// `oauth_token` here stays safe. All non-secret fields remain visible.
impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("account_id", &self.account_id)
            .field("client_id", &self.client_id)
            .field("lightstreamer_endpoint", &self.lightstreamer_endpoint)
            .field("cst", &RedactedOption(&self.cst))
            .field("x_security_token", &RedactedOption(&self.x_security_token))
            .field("oauth_token", &self.oauth_token)
            .field("api_version", &self.api_version)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl Session {
    /// Checks if this session uses OAuth authentication
    #[must_use]
    #[inline]
    pub fn is_oauth(&self) -> bool {
        self.oauth_token.is_some()
    }

    /// Checks if session is expired or will expire soon
    ///
    /// # Arguments
    /// * `margin_seconds` - Safety margin in seconds (default: 60 = 1 minute)
    ///
    /// # Returns
    /// * `true` if session is expired or will expire within margin
    /// * `false` if session is still valid
    #[must_use]
    #[inline]
    pub fn is_expired(&self, margin_seconds: Option<u64>) -> bool {
        let margin = margin_seconds.unwrap_or(60);
        let now = Utc::now().timestamp() as u64;
        now >= (self.expires_at - margin)
    }

    /// Gets the number of seconds until session expires
    ///
    /// # Returns
    /// * Positive number if session is still valid
    /// * Negative number if session is already expired
    #[must_use]
    #[inline]
    pub fn seconds_until_expiry(&self) -> u64 {
        self.expires_at - Utc::now().timestamp() as u64
    }

    /// Checks if OAuth token needs refresh (alias for is_expired for backwards compatibility)
    ///
    /// # Arguments
    /// * `margin_seconds` - Safety margin in seconds (default: 60 = 1 minute)
    #[must_use]
    #[inline]
    pub fn needs_token_refresh(&self, margin_seconds: Option<u64>) -> bool {
        self.is_expired(margin_seconds)
    }

    /// Extracts WebSocket connection information from the session
    ///
    /// # Returns
    /// * `WebsocketInfo` containing endpoint and authentication tokens
    #[must_use]
    pub fn get_websocket_info(&self) -> WebsocketInfo {
        // Ensure the server URL has the https:// prefix
        let server = if self.lightstreamer_endpoint.starts_with("http://")
            || self.lightstreamer_endpoint.starts_with("https://")
        {
            format!("{}/lightstreamer", self.lightstreamer_endpoint)
        } else {
            format!("https://{}/lightstreamer", self.lightstreamer_endpoint)
        };

        WebsocketInfo {
            server,
            cst: self.cst.clone(),
            x_security_token: self.x_security_token.clone(),
            account_id: self.account_id.clone(),
        }
    }
}

impl From<SessionResponse> for Session {
    fn from(v: SessionResponse) -> Self {
        v.get_session()
    }
}

/// Authentication manager for IG Markets API
///
/// Handles all authentication operations including:
/// - Login with API v2 or v3
/// - Automatic OAuth token refresh
/// - Account switching
/// - Session management
/// - Rate limiting for API requests
pub struct Auth {
    config: Arc<Config>,
    client: Client,
    session: Arc<RwLock<Option<Session>>>,
    rate_limiter: Arc<RwLock<RateLimiter>>,
}

impl Auth {
    /// Creates a new Auth instance
    ///
    /// # Arguments
    /// * `config` - Configuration containing credentials and API settings
    pub fn new(config: Arc<Config>) -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .expect("Failed to create HTTP client");

        let rate_limiter = Arc::new(RwLock::new(RateLimiter::new(&config.rate_limiter)));

        Self {
            config,
            client,
            session: Arc::new(RwLock::new(None)),
            rate_limiter,
        }
    }

    /// Gets the WebSocket password for Lightstreamer authentication
    ///
    /// # Returns
    /// * WebSocket password in format "CST-{cst}|XST-{token}" or empty string if session is not available
    pub async fn get_ws_info(&self) -> WebsocketInfo {
        match self.login_v2().await {
            Ok(sess) => sess.get_websocket_info(),
            Err(e) => {
                error!("Failed to get WebSocket info, login failed: {}", e);
                WebsocketInfo::default()
            }
        }
    }

    /// Gets the current session, ensuring tokens are valid
    ///
    /// This method automatically refreshes expired OAuth tokens or re-authenticates if needed.
    ///
    /// # Returns
    /// * `Ok(Session)` - Valid session with fresh tokens
    /// * `Err(AppError)` - If authentication fails
    pub async fn get_session(&self) -> Result<Session, AppError> {
        let session = self.session.read().await;

        if let Some(sess) = session.as_ref() {
            // Check if OAuth token needs refresh
            if sess.needs_token_refresh(Some(300)) {
                drop(session); // Release read lock
                debug!("OAuth token needs refresh");
                return self.refresh_token().await;
            }
            return Ok(sess.clone());
        }

        drop(session);

        // No session exists, need to login
        info!("No active session, logging in");
        self.login().await
    }

    /// Performs initial login to IG Markets API
    ///
    /// Automatically detects API version from config and uses appropriate authentication method.
    ///
    /// # Returns
    /// * `Ok(Session)` - Authenticated session
    /// * `Err(AppError)` - If login fails
    pub async fn login(&self) -> Result<Session, AppError> {
        let api_version = self.config.api_version.unwrap_or(2);

        debug!("Logging in with API v{}", api_version);

        let session = if api_version == 3 {
            self.login_oauth().await?
        } else {
            self.login_v2().await?
        };

        // Store session
        let mut sess = self.session.write().await;
        *sess = Some(session.clone());

        info!("✓ Login successful, account: {}", session.account_id);
        Ok(session)
    }

    /// Performs login using API v2 (CST/X-SECURITY-TOKEN) with automatic retry on rate limit
    async fn login_v2(&self) -> Result<Session, AppError> {
        let url = format!("{}/session", self.config.rest_api.base_url);

        let body = serde_json::json!({
            "identifier": self.config.credentials.username,
            "password": self.config.credentials.password,
        });

        debug!("Sending v2 login request to: {}", url);

        let headers = vec![
            ("X-IG-API-KEY", self.config.credentials.api_key.as_str()),
            ("Content-Type", "application/json"),
            ("Version", "2"),
        ];

        let response = make_http_request(
            &self.client,
            self.rate_limiter.clone(),
            Method::POST,
            &url,
            headers,
            &Some(body),
            RetryConfig::infinite(),
        )
        .await?;

        // Extract CST and X-SECURITY-TOKEN from headers
        let cst: String = match response
            .headers()
            .get("CST")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
        {
            Some(token) => token,
            None => {
                error!("CST header not found in response");
                return Err(AppError::InvalidInput("CST missing".to_string()));
            }
        };
        let x_security_token: String = match response
            .headers()
            .get("X-SECURITY-TOKEN")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
        {
            Some(token) => token,
            None => {
                error!("X-SECURITY-TOKEN header not found in response");
                return Err(AppError::InvalidInput(
                    "X-SECURITY-TOKEN missing".to_string(),
                ));
            }
        };

        let x_ig_api_key: String = response
            .headers()
            .get("X-IG-API-KEY")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| self.config.credentials.api_key.clone());

        let security_headers: SecurityHeaders = SecurityHeaders {
            cst,
            x_security_token,
            x_ig_api_key,
        };

        // Get response body as text first for debugging
        let body_text = response.text().await.map_err(|e| {
            error!("Failed to read response body: {}", e);
            AppError::Network(e)
        })?;
        debug!("Login response body length: {} bytes", body_text.len());

        // Parse the JSON
        let mut response: SessionResponse = serde_json::from_str(&body_text).map_err(|e| {
            // Never log the body: the `/session` response carries credentials.
            error!(
                endpoint = %url,
                body_len = body_text.len(),
                "failed to parse login response: {}",
                e
            );
            AppError::Deserialization(format!("Failed to parse login response: {}", e))
        })?;
        let session = response.get_session_v2(&security_headers);

        Ok(session)
    }

    /// Performs login using API v3 (OAuth) with automatic retry on rate limit
    async fn login_oauth(&self) -> Result<Session, AppError> {
        let url = format!("{}/session", self.config.rest_api.base_url);

        let body = serde_json::json!({
            "identifier": self.config.credentials.username,
            "password": self.config.credentials.password,
        });

        debug!("Sending OAuth login request to: {}", url);
        let headers = vec![
            ("X-IG-API-KEY", self.config.credentials.api_key.as_str()),
            ("Content-Type", "application/json"),
            ("Version", "3"),
        ];

        let response = make_http_request(
            &self.client,
            self.rate_limiter.clone(),
            Method::POST,
            &url,
            headers,
            &Some(body),
            RetryConfig::infinite(),
        )
        .await?;

        let response: SessionResponse = response.json().await?;
        let mut session = response.get_session();
        if session.account_id != self.config.credentials.account_id {
            session.account_id = self.config.credentials.account_id.clone();
        };

        assert!(session.is_oauth());

        Ok(session)
    }

    /// Refreshes an expired OAuth token with automatic retry on rate limit
    ///
    /// If refresh fails (e.g., refresh token expired), performs full re-authentication.
    ///
    /// # Returns
    /// * `Ok(Session)` - New session with refreshed tokens
    /// * `Err(AppError)` - If refresh and re-authentication both fail
    pub async fn refresh_token(&self) -> Result<Session, AppError> {
        let current_session = {
            let session = self.session.read().await;
            session.clone()
        };

        if let Some(sess) = current_session {
            if sess.is_expired(Some(1)) {
                debug!("Session expired, performing login");
                self.login().await
            } else {
                Ok(sess)
            }
        } else {
            warn!("No session to refresh, performing login");
            self.login().await
        }
    }

    /// Switches to a different trading account
    ///
    /// # Arguments
    /// * `account_id` - The account ID to switch to
    /// * `default_account` - Whether to set as default account
    ///
    /// # Returns
    /// * `Ok(Session)` - New session for the switched account
    /// * `Err(AppError)` - If account switch fails
    pub async fn switch_account(
        &self,
        account_id: &str,
        default_account: Option<bool>,
    ) -> Result<Session, AppError> {
        let current_session = self.get_session().await?;
        if matches!(current_session.api_version, 3) {
            return Err(AppError::InvalidInput(
                "Cannot switch accounts with OAuth".to_string(),
            ));
        }

        if current_session.account_id == account_id {
            debug!("Already on account {}", account_id);
            return Ok(current_session);
        }

        info!("Switching to account: {}", account_id);

        let url = format!("{}/session", self.config.rest_api.base_url);

        let mut body = serde_json::json!({
            "accountId": account_id,
        });

        if let Some(default) = default_account {
            body["defaultAccount"] = serde_json::json!(default);
        }

        // Build headers with authentication
        let api_key = self.config.credentials.api_key.clone();
        let auth_header_value;
        let cst;
        let x_security_token;

        let mut headers = vec![
            ("X-IG-API-KEY", api_key.as_str()),
            ("Content-Type", "application/json"),
            ("Version", "1"),
        ];

        // Add authentication headers based on session type
        if let Some(oauth) = &current_session.oauth_token {
            auth_header_value = format!("Bearer {}", oauth.access_token);
            headers.push(("Authorization", auth_header_value.as_str()));
        } else {
            if let Some(cst_val) = &current_session.cst {
                cst = cst_val.clone();
                headers.push(("CST", cst.as_str()));
            }
            if let Some(token_val) = &current_session.x_security_token {
                x_security_token = token_val.clone();
                headers.push(("X-SECURITY-TOKEN", x_security_token.as_str()));
            }
        }

        let _response = make_http_request(
            &self.client,
            self.rate_limiter.clone(),
            Method::PUT,
            &url,
            headers,
            &Some(body),
            RetryConfig::infinite(),
        )
        .await?;

        // After switching, update the session
        let mut new_session = current_session.clone();
        new_session.account_id = account_id.to_string();

        let mut session = self.session.write().await;
        *session = Some(new_session.clone());

        info!("✓ Switched to account: {}", account_id);
        Ok(new_session)
    }

    /// Logs out and clears the current session
    pub async fn logout(&self) -> Result<(), AppError> {
        info!("Logging out");

        let mut session = self.session.write().await;
        *session = None;

        info!("✓ Logged out successfully");
        Ok(())
    }
}

#[test]
fn test_v2_response_deserialization() -> Result<(), serde_json::Error> {
    let json = r#"{"accountType":"CFD","accountInfo":{"balance":21065.86,"deposit":3033.31,"profitLoss":-285.27,"available":16659.01},"currencyIsoCode":"EUR","currencySymbol":"E","currentAccountId":"ZZZZZ","lightstreamerEndpoint":"https://demo-apd.marketdatasystems.com","accounts":[{"accountId":"Z405P5","accountName":"Turbo24","preferred":false,"accountType":"PHYSICAL"},{"accountId":"ZHJ5N","accountName":"DEMO_A","preferred":false,"accountType":"CFD"},{"accountId":"ZZZZZ","accountName":"Opciones","preferred":true,"accountType":"CFD"}],"clientId":"101290216","timezoneOffset":1,"hasActiveDemoAccounts":true,"hasActiveLiveAccounts":true,"trailingStopsEnabled":false,"reroutingEnvironment":null,"dealingEnabled":true}"#;

    let result: crate::model::auth::SessionResponse = serde_json::from_str(json)?;
    println!("Success: is_v2={}", result.is_v2());

    let response: crate::model::auth::V2Response = serde_json::from_str(json)?;
    assert_eq!(response.account_type, "CFD");
    Ok(())
}

#[test]
fn test_v2_response_deserialization_prod() -> Result<(), serde_json::Error> {
    let json = r#"{"accountType":"CFD","accountInfo":{"balance":18791.56,"deposit":3300.18,"profitLoss":187.42,"available":14952.68},"currencyIsoCode":"EUR","currencySymbol":"E","currentAccountId":"BS0Y3","lightstreamerEndpoint":"https://apd.marketdatasystems.com","accounts":[{"accountId":"BS0Y3","accountName":"Opciones Prod","preferred":true,"accountType":"CFD"},{"accountId":"BSI1I","accountName":"Barreras y Opciones","preferred":false,"accountType":"CFD"},{"accountId":"BSU96","accountName":"Turbos","preferred":false,"accountType":"PHYSICAL"},{"accountId":"BTCKN","accountName":"CFD","preferred":false,"accountType":"CFD"},{"accountId":"BXNIZ","accountName":"Principal","preferred":false,"accountType":"CFD"}],"clientId":"102828353","timezoneOffset":1,"hasActiveDemoAccounts":true,"hasActiveLiveAccounts":true,"trailingStopsEnabled":false,"reroutingEnvironment":null,"dealingEnabled":true}"#;

    let result: crate::model::auth::SessionResponse = serde_json::from_str(json)?;
    println!("Success: is_v2={}", result.is_v2());

    let response: crate::model::auth::V2Response = serde_json::from_str(json)?;
    assert_eq!(response.account_type, "CFD");
    assert_eq!(response.current_account_id, "BS0Y3");
    Ok(())
}

#[cfg(test)]
mod redaction_tests {
    use super::*;

    fn secret_session() -> Session {
        Session {
            account_id: "ACC123".to_string(),
            client_id: "CLIENT1".to_string(),
            lightstreamer_endpoint: "https://ls.example.com".to_string(),
            cst: Some("SECRET-CST-VALUE".to_string()),
            x_security_token: Some("SECRET-XST-VALUE".to_string()),
            oauth_token: Some(OAuthToken {
                access_token: "SECRET-ACCESS-VALUE".to_string(),
                refresh_token: "SECRET-REFRESH-VALUE".to_string(),
                scope: "read write".to_string(),
                token_type: "Bearer".to_string(),
                expires_in: "60".to_string(),
                created_at: chrono::Utc::now(),
            }),
            api_version: 3,
            expires_at: 0,
        }
    }

    #[test]
    fn test_session_debug_redacts_tokens() {
        let session = secret_session();
        let rendered = format!("{session:?}");

        assert!(!rendered.contains("SECRET-CST-VALUE"));
        assert!(!rendered.contains("SECRET-XST-VALUE"));
        assert!(!rendered.contains("SECRET-ACCESS-VALUE"));
        assert!(!rendered.contains("SECRET-REFRESH-VALUE"));
        assert!(rendered.contains("<redacted>"));
        // Non-secret fields stay visible.
        assert!(rendered.contains("ACC123"));
        assert!(rendered.contains("ls.example.com"));
    }

    #[test]
    fn test_websocket_info_debug_redacts_tokens() {
        let ws = WebsocketInfo {
            server: "https://ls.example.com/lightstreamer".to_string(),
            cst: Some("SECRET-CST-VALUE".to_string()),
            x_security_token: Some("SECRET-XST-VALUE".to_string()),
            account_id: "ACC123".to_string(),
        };
        let rendered = format!("{ws:?}");

        assert!(!rendered.contains("SECRET-CST-VALUE"));
        assert!(!rendered.contains("SECRET-XST-VALUE"));
        assert!(rendered.contains("Some(<redacted>)"));
        assert!(rendered.contains("ACC123"));
        assert!(rendered.contains("ls.example.com"));
    }

    #[test]
    fn test_websocket_info_display_redacts_tokens() {
        let ws = WebsocketInfo {
            server: "https://ls.example.com/lightstreamer".to_string(),
            cst: Some("SECRET-CST-VALUE".to_string()),
            x_security_token: Some("SECRET-XST-VALUE".to_string()),
            account_id: "ACC123".to_string(),
        };
        let rendered = format!("{ws}");

        assert!(!rendered.contains("SECRET-CST-VALUE"));
        assert!(!rendered.contains("SECRET-XST-VALUE"));
        assert!(rendered.contains("<redacted>"));
        assert!(rendered.contains("ACC123"));

        // `None` tokens render as `None`, not as a redacted placeholder.
        let ws_none = WebsocketInfo {
            server: "https://ls.example.com/lightstreamer".to_string(),
            cst: None,
            x_security_token: None,
            account_id: "ACC123".to_string(),
        };
        assert!(format!("{ws_none}").contains("None"));
    }
}
