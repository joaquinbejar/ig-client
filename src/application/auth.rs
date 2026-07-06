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
use crate::application::http::make_http_request;
use crate::application::rate_limiter::RateLimiter;
use crate::constants::USER_AGENT;
use crate::error::{AppError, AuthError};
pub(crate) use crate::model::auth::{SecurityHeaders, SessionResponse};
use crate::model::retry::RetryConfig;
use reqwest::{Client, Method};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
// `OAuthToken` and `chrono::Utc` are only referenced from the unit tests below
// (the `Session` data type that used them in production moved to
// `crate::model::auth`), so they are imported under `cfg(test)` to keep the
// non-test build free of unused-import warnings.
#[cfg(test)]
use crate::model::auth::OAuthToken;
#[cfg(test)]
use chrono::Utc;

// `Session` and `WebsocketInfo` are pure data types and live in the model
// layer (`crate::model::auth`). They are re-exported here so the historical
// public paths `crate::application::auth::Session` /
// `crate::application::auth::WebsocketInfo` keep resolving, and so the auth
// I/O manager below can reference them unqualified.
pub use crate::model::auth::{Session, WebsocketInfo};

/// Merges freshly-issued v2 security tokens into a session after an account
/// switch.
///
/// IG returns a new `X-SECURITY-TOKEN` (and sometimes a new `CST`) in the
/// response to `PUT /session`. When a token is present it replaces the stale
/// one; when absent the existing token is preserved — the switch response does
/// not always re-issue both, and nulling a token would break the next request.
/// All other session fields are carried through unchanged.
#[must_use]
fn apply_switch_headers(mut session: Session, cst: Option<&str>, xst: Option<&str>) -> Session {
    if let Some(cst) = cst {
        session.cst = Some(cst.to_string());
    }
    if let Some(xst) = xst {
        session.x_security_token = Some(xst.to_string());
    }
    session
}

/// Decides whether a v2 login should switch to the configured account.
///
/// Returns `true` only when a *real* account was configured (not empty and not
/// the [`DEFAULT_ACCOUNT_ID`](crate::constants::DEFAULT_ACCOUNT_ID) sentinel) and
/// it differs from the account the login landed on. OAuth (v3) pins the account
/// elsewhere and is never switched here. Guarding the sentinel is essential: it
/// is non-empty, so without this check an unconfigured client would try to
/// switch to `"default_account_id"`, which IG rejects, failing an otherwise-valid
/// login.
#[must_use]
fn should_switch_account(api_version: u8, configured: &str, current: &str) -> bool {
    api_version != 3
        && !configured.is_empty()
        && configured != crate::constants::DEFAULT_ACCOUNT_ID
        && configured != current
}

/// Selects the proactive-refresh safety margin (in seconds) for a session based
/// on its authentication model.
///
/// v3 (OAuth) access tokens are short-lived (~60s), so the v2 margin would keep
/// them permanently "about to expire" and force a login on every call; a small
/// [`PROACTIVE_REFRESH_MARGIN_V3_SECS`](crate::constants::PROACTIVE_REFRESH_MARGIN_V3_SECS)
/// margin is used instead. v2 (CST / X-SECURITY-TOKEN) sessions last ~6h, so the
/// larger
/// [`PROACTIVE_REFRESH_MARGIN_V2_SECS`](crate::constants::PROACTIVE_REFRESH_MARGIN_V2_SECS)
/// margin gives ample lead time.
///
/// The *same* margin is used by both [`Auth::get_session`] (to decide a refresh
/// is due) and [`Auth::refresh_token`] (to actually perform it), so the
/// proactive-refresh window is consistent and always fires — the previous 300s /
/// 1s mismatch meant the advertised margin never triggered a refresh.
#[must_use]
fn proactive_refresh_margin_secs(session: &Session) -> u64 {
    if session.is_oauth() {
        crate::constants::PROACTIVE_REFRESH_MARGIN_V3_SECS
    } else {
        crate::constants::PROACTIVE_REFRESH_MARGIN_V2_SECS
    }
}

/// Ensures a v3 (OAuth) login actually produced an OAuth session.
///
/// The v3 `/session` endpoint is expected to return an OAuth body. Because
/// [`SessionResponse`] is untagged, a v2-shaped body sent to the v3 request
/// still deserializes successfully — as a v2 session carrying no OAuth token.
/// That is a server-side / protocol mismatch, not a client bug. On the reactive
/// [`force_refresh`](Auth::force_refresh) -> [`login`](Auth::login) path a 401
/// reaches this code more often, so a mismatched body must surface as a typed
/// error rather than panic.
///
/// The offending response body / token is never logged.
///
/// # Errors
/// Returns [`AppError::Unauthorized`] when the session lacks an OAuth token.
fn ensure_oauth_session(session: Session) -> Result<Session, AppError> {
    if session.is_oauth() {
        Ok(session)
    } else {
        error!("v3 login response did not contain an OAuth token");
        Err(AppError::Unauthorized)
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
    // `RateLimiter` is `Clone` and already wraps each governor bucket in an
    // `Arc`, so it is shared directly without an outer `RwLock`: the limiter is
    // configured once at construction and never write-swapped.
    rate_limiter: RateLimiter,
}

impl Auth {
    /// Creates a new Auth instance
    ///
    /// # Arguments
    /// * `config` - Configuration containing credentials and API settings
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client cannot be constructed at
    /// startup — typically a missing TLS backend, but also invalid proxy or
    /// certificate configuration. This is an unrecoverable environment
    /// invariant at construction time. For graceful handling use
    /// [`Auth::try_new`], which returns a typed [`AppError`] instead of
    /// panicking.
    pub fn new(config: Arc<Config>) -> Self {
        Self::try_new(config).expect("Failed to create HTTP client")
    }

    /// Creates a new Auth instance, returning an error if the HTTP client
    /// cannot be constructed.
    ///
    /// This is the fallible counterpart to [`Auth::new`]: it surfaces a TLS /
    /// client-builder failure as a typed [`AppError`] instead of panicking, so
    /// callers can handle a broken TLS backend gracefully.
    ///
    /// # Arguments
    /// * `config` - Configuration containing credentials and API settings
    ///
    /// # Errors
    /// Returns [`AppError::Network`] if the underlying `reqwest` client cannot
    /// be built (e.g. the system TLS backend fails to initialize).
    pub fn try_new(config: Arc<Config>) -> Result<Self, AppError> {
        let client = Client::builder().user_agent(USER_AGENT).build()?;

        let rate_limiter = RateLimiter::new(&config.rate_limiter);

        Ok(Self {
            config,
            client,
            session: Arc::new(RwLock::new(None)),
            rate_limiter,
        })
    }

    /// Gets WebSocket connection information for Lightstreamer, reusing the
    /// cached session.
    ///
    /// This calls [`Auth::get_session`], which returns the cached session when
    /// it is still valid and only logs in (storing the new session) when none
    /// exists or it has expired. The configured API version is honoured — no
    /// per-call v2 re-login is performed.
    ///
    /// # Returns
    /// * `Ok(WebsocketInfo)` - Endpoint and authentication tokens for the
    ///   current session.
    /// * `Err(AppError)` - If session retrieval (login / refresh) fails.
    ///
    /// # Errors
    /// Returns [`AppError`] when the session cannot be retrieved (login or token
    /// refresh failure).
    pub async fn ws_info(&self) -> Result<WebsocketInfo, AppError> {
        let session = self.get_session().await?;
        Ok(session.get_websocket_info())
    }

    /// Gets the WebSocket password for Lightstreamer authentication
    ///
    /// # Returns
    /// * WebSocket password in format "CST-{cst}|XST-{token}" or empty string if session is not available
    #[deprecated(
        note = "use ws_info() which reuses the cached session and returns a typed error instead of a default-on-error WebsocketInfo"
    )]
    pub async fn get_ws_info(&self) -> WebsocketInfo {
        self.ws_info().await.unwrap_or_default()
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
            // Refresh proactively once the session enters its refresh margin. The
            // margin is derived from the session type so it matches the one
            // `refresh_token` re-checks with — otherwise the refresh detour would
            // hand back the same near-expired session (the old 300s/1s mismatch).
            let margin = proactive_refresh_margin_secs(sess);
            if sess.needs_token_refresh(Some(margin)) {
                drop(session); // Release read lock
                debug!(margin_secs = margin, "session within refresh margin");
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

        // Store session. The write guard is scoped so it is released before any
        // account-selection switch below: `switch_account` reads `self.session`
        // via `get_session`, and holding the guard across that call would
        // deadlock.
        {
            let mut sess = self.session.write().await;
            *sess = Some(session.clone());
        }

        info!("✓ Login successful, account: {}", session.account_id);

        // v2 account selection: the v2 `/session` response reports IG's
        // current/default account, which may differ from the configured one.
        // Switch to the configured account when it is set and differs. This
        // cannot recurse: the session was just stored above, so
        // `switch_account` -> `get_session` returns the cached (valid) session
        // and never triggers another login. OAuth (v3) already pins the account
        // in `login_oauth`, so it is skipped here.
        if api_version != 3 {
            let configured = self.config.credentials.account_id.clone();
            if should_switch_account(api_version, &configured, &session.account_id) {
                info!("selecting configured account after v2 login");
                // `Box::pin` breaks the compile-time async recursion cycle
                // (login -> switch_account -> get_session -> login). The cycle
                // is runtime-bounded: the session was stored above, so
                // `get_session` returns it without logging in again.
                return Box::pin(self.switch_account(&configured, None)).await;
            }
        }

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
            &self.rate_limiter,
            Method::POST,
            &url,
            headers,
            &Some(body),
            RetryConfig::default(),
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
                // A rejected / malformed auth response, not bad caller input:
                // surface a typed auth error naming the missing header.
                error!("missing cst header in login response");
                return Err(AuthError::MissingSessionToken("cst".to_string()).into());
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
                // A rejected / malformed auth response, not bad caller input:
                // surface a typed auth error naming the missing header.
                error!("missing x-security-token header in login response");
                return Err(AuthError::MissingSessionToken("x-security-token".to_string()).into());
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
            &self.rate_limiter,
            Method::POST,
            &url,
            headers,
            &Some(body),
            RetryConfig::default(),
        )
        .await?;

        let response: SessionResponse = response.json().await?;
        let mut session = response.get_session();
        if session.account_id != self.config.credentials.account_id {
            session.account_id = self.config.credentials.account_id.clone();
        };

        // A v2-shaped body on the v3 path (server-side mismatch) must not panic;
        // surface it as a typed error instead.
        ensure_oauth_session(session)
    }

    /// Proactively refreshes the session when it is within its refresh margin.
    ///
    /// This is the *proactive* path (driven by the local clock). It re-checks the
    /// cached session against the same margin
    /// [`get_session`](Self::get_session) used to decide a refresh was due, so
    /// the two stay consistent and the refresh actually fires when the session is
    /// close to expiry. If the session is still comfortably valid it is returned
    /// unchanged; otherwise a full [`login`](Self::login) is performed.
    ///
    /// For the reactive 401 / server-side-invalidation path — where the local
    /// clock still considers the token valid but IG has already rejected it — use
    /// [`force_refresh`](Self::force_refresh), which re-authenticates
    /// unconditionally.
    ///
    /// # Returns
    /// * `Ok(Session)` - A valid session (refreshed if it was within margin).
    /// * `Err(AppError)` - If re-authentication fails.
    ///
    /// # Errors
    /// Returns [`AppError`] when a required login fails (network, credentials, or
    /// rate limiting).
    pub async fn refresh_token(&self) -> Result<Session, AppError> {
        let current_session = {
            let session = self.session.read().await;
            session.clone()
        };

        if let Some(sess) = current_session {
            // Honour the SAME margin `get_session` used to route here, so a
            // session inside the proactive window is actually re-authenticated
            // instead of being handed back near-expired.
            let margin = proactive_refresh_margin_secs(&sess);
            if sess.is_expired(Some(margin)) {
                debug!(
                    margin_secs = margin,
                    "session within refresh margin, logging in"
                );
                self.login().await
            } else {
                Ok(sess)
            }
        } else {
            warn!("No session to refresh, performing login");
            self.login().await
        }
    }

    /// Forces a fresh re-authentication regardless of local expiry state.
    ///
    /// This is the reactive 401 / server-side-invalidation path. When IG rejects
    /// a token that the local clock still considers valid (server-side
    /// invalidation, a concurrent login elsewhere, or clock skew), a proactive
    /// [`refresh_token`](Self::refresh_token) would see a "valid" session and
    /// hand back the *same* stale token, so the replayed request would fail
    /// again. `force_refresh` ignores local expiry and performs a full
    /// [`login`](Self::login), which fetches and stores a brand-new session.
    ///
    /// It cannot loop back through the 401 handler: [`login`](Self::login) issues
    /// its HTTP requests through
    /// [`make_http_request`] directly, not
    /// through the [`HttpClient`](crate::application::http::HttpClient) refresh-and-replay
    /// path, so a 401 encountered *during* login surfaces as a typed error rather
    /// than recursing into `force_refresh`.
    ///
    /// # Returns
    /// * `Ok(Session)` - A freshly authenticated session with new tokens.
    /// * `Err(AppError)` - If re-authentication fails.
    ///
    /// # Errors
    /// Returns [`AppError`] when the login request fails (network, credentials,
    /// or rate limiting).
    pub async fn force_refresh(&self) -> Result<Session, AppError> {
        debug!("forcing re-authentication, ignoring local expiry");
        self.login().await
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

        // Build headers with authentication. Only the v2 (CST /
        // X-SECURITY-TOKEN) path is reachable here: OAuth sessions
        // (`api_version == 3`) are rejected above, so no `Authorization: Bearer`
        // branch is needed.
        let api_key = self.config.credentials.api_key.clone();
        let cst;
        let x_security_token;

        let mut headers = vec![
            ("X-IG-API-KEY", api_key.as_str()),
            ("Content-Type", "application/json"),
            ("Version", "1"),
        ];

        if let Some(cst_val) = &current_session.cst {
            cst = cst_val.clone();
            headers.push(("CST", cst.as_str()));
        }
        if let Some(token_val) = &current_session.x_security_token {
            x_security_token = token_val.clone();
            headers.push(("X-SECURITY-TOKEN", x_security_token.as_str()));
        }

        let response = make_http_request(
            &self.client,
            &self.rate_limiter,
            Method::PUT,
            &url,
            headers,
            &Some(body),
            RetryConfig::default(),
        )
        .await?;

        // IG re-issues the X-SECURITY-TOKEN (and sometimes CST) in the switch
        // response. Read them from the headers and merge them in; a missing
        // header keeps the existing token rather than nulling it.
        let new_cst = response
            .headers()
            .get("CST")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let new_x_security_token = response
            .headers()
            .get("X-SECURITY-TOKEN")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        // IG re-issues X-SECURITY-TOKEN on a successful switch. Its absence on a
        // 2xx response is anomalous (proxy stripping / response-shape drift): the
        // old token is kept below, but flag it since it will 401 the next call.
        if new_x_security_token.is_none() {
            warn!("switch response carried no X-SECURITY-TOKEN; keeping the previous token");
        }

        // After switching, update the session with the fresh tokens and the new
        // account id.
        let mut new_session = apply_switch_headers(
            current_session.clone(),
            new_cst.as_deref(),
            new_x_security_token.as_deref(),
        );
        new_session.account_id = account_id.to_string();

        {
            let mut session = self.session.write().await;
            *session = Some(new_session.clone());
        }

        info!("✓ Switched to account: {}", account_id);
        Ok(new_session)
    }

    /// Logs out and clears the current session.
    ///
    /// Before clearing local state, this issues `DELETE /session` (IG API
    /// Version 1) with the current authentication headers. For a **v2** session
    /// (CST / X-SECURITY-TOKEN) this invalidates the session server-side. For a
    /// **v3 / OAuth** session there is no IG token-revocation endpoint and the
    /// short-lived access token has usually already expired, so `DELETE /session`
    /// is best-effort: logout clears local state and the OAuth tokens lapse on
    /// their own expiry rather than being actively revoked. A `401 Unauthorized`
    /// (or an already-invalid OAuth token) is treated as already-logged-out and
    /// reported as success.
    ///
    /// The local session is always cleared, even when the server-side call
    /// fails, so the client is never wedged in an authenticated-but-unusable
    /// state; the underlying failure is still returned as a typed error.
    ///
    /// # Errors
    /// Returns [`AppError`] when the server-side logout request fails for a
    /// reason other than an already-invalid session (401). The local session is
    /// cleared regardless.
    pub async fn logout(&self) -> Result<(), AppError> {
        info!("Logging out");

        // Snapshot the current session without holding the lock across the HTTP
        // call. With no session there is nothing to revoke server-side.
        let current_session = {
            let session = self.session.read().await;
            session.clone()
        };

        let server_result = match current_session {
            Some(sess) => self.revoke_session(&sess).await,
            None => Ok(()),
        };

        // Always clear local state so the client is never wedged, regardless of
        // the server-side outcome.
        {
            let mut session = self.session.write().await;
            *session = None;
        }

        match server_result {
            Ok(()) => {
                info!("✓ Logged out successfully");
                Ok(())
            }
            Err(e) => {
                // The error type carries no secrets (see `AppError`), so it is
                // safe to log; local state has already been cleared.
                error!("server-side logout failed: {}", e);
                Err(e)
            }
        }
    }

    /// Issues `DELETE /session` (IG API Version 1) to terminate the session
    /// server-side.
    ///
    /// A `401 Unauthorized` (or an already-invalid OAuth token) means the
    /// session is no longer valid server-side and is treated as success.
    ///
    /// # Errors
    /// Returns [`AppError`] if the request fails for any reason other than an
    /// already-invalid session.
    async fn revoke_session(&self, session: &Session) -> Result<(), AppError> {
        let url = format!("{}/session", self.config.rest_api.base_url);
        let api_key = self.config.credentials.api_key.clone();

        let auth_header_value;
        let cst;
        let x_security_token;

        let mut headers = vec![
            ("X-IG-API-KEY", api_key.as_str()),
            ("Content-Type", "application/json"),
            ("Version", "1"),
        ];

        if let Some(oauth) = &session.oauth_token {
            auth_header_value = format!("Bearer {}", oauth.access_token);
            headers.push(("Authorization", auth_header_value.as_str()));
            headers.push(("IG-ACCOUNT-ID", session.account_id.as_str()));
        } else {
            if let Some(cst_val) = &session.cst {
                cst = cst_val.clone();
                headers.push(("CST", cst.as_str()));
            }
            if let Some(token_val) = &session.x_security_token {
                x_security_token = token_val.clone();
                headers.push(("X-SECURITY-TOKEN", x_security_token.as_str()));
            }
        }

        match make_http_request(
            &self.client,
            &self.rate_limiter,
            Method::DELETE,
            &url,
            headers,
            &None::<()>,
            RetryConfig::default(),
        )
        .await
        {
            Ok(_) => Ok(()),
            // A 401 (or an expired OAuth token) means the session is already
            // invalid server-side: the logout goal is met.
            Err(AppError::Unauthorized | AppError::OAuthTokenExpired) => {
                debug!("session already invalid server-side; treating as logged out");
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod session_lifecycle_tests {
    use super::*;

    fn v2_session(cst: Option<&str>, xst: Option<&str>) -> Session {
        Session {
            account_id: "ACC-OLD".to_string(),
            client_id: "CLIENT1".to_string(),
            lightstreamer_endpoint: "demo-apd.marketdatasystems.com".to_string(),
            cst: cst.map(String::from),
            x_security_token: xst.map(String::from),
            oauth_token: None,
            api_version: 2,
            expires_at: 123,
        }
    }

    #[test]
    fn test_apply_switch_headers_replaces_xst_and_keeps_other_fields() {
        let session = v2_session(Some("OLD-CST"), Some("OLD-XST"));
        // A switch response that only re-issues the X-SECURITY-TOKEN.
        let updated = apply_switch_headers(session, None, Some("NEW-XST"));

        assert_eq!(updated.x_security_token.as_deref(), Some("NEW-XST"));
        // CST is preserved when the header is absent.
        assert_eq!(updated.cst.as_deref(), Some("OLD-CST"));
        // Unrelated fields carry through unchanged.
        assert_eq!(updated.account_id, "ACC-OLD");
        assert_eq!(updated.expires_at, 123);
        assert_eq!(updated.api_version, 2);
    }

    #[test]
    fn test_apply_switch_headers_updates_both_tokens() {
        let session = v2_session(Some("OLD-CST"), Some("OLD-XST"));
        let updated = apply_switch_headers(session, Some("NEW-CST"), Some("NEW-XST"));

        assert_eq!(updated.cst.as_deref(), Some("NEW-CST"));
        assert_eq!(updated.x_security_token.as_deref(), Some("NEW-XST"));
    }

    #[test]
    fn test_apply_switch_headers_none_preserves_existing_tokens() {
        let session = v2_session(Some("OLD-CST"), Some("OLD-XST"));
        // A switch response carrying neither header must not null the tokens.
        let updated = apply_switch_headers(session, None, None);

        assert_eq!(updated.cst.as_deref(), Some("OLD-CST"));
        assert_eq!(updated.x_security_token.as_deref(), Some("OLD-XST"));
    }

    #[test]
    fn test_should_switch_account_guards_the_sentinel_and_v3() {
        use crate::constants::DEFAULT_ACCOUNT_ID;

        // Real configured account that differs from the current one: switch.
        assert!(should_switch_account(2, "ACC-B", "ACC-A"));
        // Unconfigured default sentinel: must NOT switch (would fail login).
        assert!(!should_switch_account(2, DEFAULT_ACCOUNT_ID, "ACC-A"));
        // Empty configured account: must not switch.
        assert!(!should_switch_account(2, "", "ACC-A"));
        // Already on the configured account: no switch needed.
        assert!(!should_switch_account(2, "ACC-A", "ACC-A"));
        // OAuth (v3) is never switched through this path.
        assert!(!should_switch_account(3, "ACC-B", "ACC-A"));
    }

    #[tokio::test]
    async fn test_ws_info_uses_cached_session_without_login() {
        let auth = Auth::new(Arc::new(Config::default()));

        // Seed a valid (non-expired) v2 session with known tokens. Because the
        // session is valid, `ws_info` -> `get_session` must return it without a
        // network login (a login would require real credentials and fail).
        let expires_at = (Utc::now().timestamp() + 3600) as u64;
        let seeded = Session {
            account_id: "ACC123".to_string(),
            client_id: "CLIENT1".to_string(),
            lightstreamer_endpoint: "demo-apd.marketdatasystems.com".to_string(),
            cst: Some("CST-TOKEN".to_string()),
            x_security_token: Some("XST-TOKEN".to_string()),
            oauth_token: None,
            api_version: 2,
            expires_at,
        };
        {
            let mut guard = auth.session.write().await;
            *guard = Some(seeded);
        }

        let ws = match auth.ws_info().await {
            Ok(ws) => ws,
            Err(e) => panic!("ws_info should return Ok for a cached session: {e}"),
        };

        // The returned info derives from the cached session's tokens, proving no
        // fresh login occurred.
        assert_eq!(ws.account_id, "ACC123");
        assert_eq!(ws.cst.as_deref(), Some("CST-TOKEN"));
        assert_eq!(ws.x_security_token.as_deref(), Some("XST-TOKEN"));
        assert!(ws.server.contains("demo-apd.marketdatasystems.com"));
    }
}

#[cfg(test)]
mod expiry_and_refresh_tests {
    use super::*;

    fn v2_session(expires_at: u64) -> Session {
        Session {
            account_id: "ACC123".to_string(),
            client_id: "CLIENT1".to_string(),
            lightstreamer_endpoint: "demo-apd.marketdatasystems.com".to_string(),
            cst: Some("CST-TOKEN".to_string()),
            x_security_token: Some("XST-TOKEN".to_string()),
            oauth_token: None,
            api_version: 2,
            expires_at,
        }
    }

    fn v3_session(expires_at: u64) -> Session {
        Session {
            account_id: "ACC123".to_string(),
            client_id: "CLIENT1".to_string(),
            lightstreamer_endpoint: "demo-apd.marketdatasystems.com".to_string(),
            cst: None,
            x_security_token: None,
            oauth_token: Some(OAuthToken {
                access_token: "ACCESS".to_string(),
                refresh_token: "REFRESH".to_string(),
                scope: "read write".to_string(),
                token_type: "Bearer".to_string(),
                expires_in: "60".to_string(),
                created_at: Utc::now(),
            }),
            api_version: 3,
            expires_at,
        }
    }

    #[test]
    fn test_seconds_until_expiry_expired_session_returns_zero() {
        // expires_at 100s in the past -> saturating to 0, never a huge u64.
        let past = u64::try_from(Utc::now().timestamp())
            .unwrap_or(0)
            .saturating_sub(100);
        let session = v2_session(past);
        assert_eq!(session.seconds_until_expiry(), 0);
    }

    #[test]
    fn test_seconds_until_expiry_valid_session_is_positive() {
        let future = u64::try_from(Utc::now().timestamp())
            .unwrap_or(0)
            .saturating_add(3600);
        let session = v2_session(future);
        // Allow a small slack for the wall-clock read inside the method.
        assert!(session.seconds_until_expiry() > 3500);
    }

    #[test]
    fn test_time_until_expiry_expired_session_is_zero() {
        let past = u64::try_from(Utc::now().timestamp())
            .unwrap_or(0)
            .saturating_sub(100);
        let session = v2_session(past);
        assert_eq!(session.time_until_expiry(), std::time::Duration::ZERO);
    }

    #[test]
    fn test_is_expired_large_margin_does_not_underflow() {
        // Tiny expires_at with an enormous margin must not underflow / panic;
        // it simply reports the session as expired.
        let session = v2_session(1);
        assert!(session.is_expired(Some(u64::MAX)));
        assert!(session.is_expired(Some(1000)));
    }

    #[test]
    fn test_is_expired_valid_session_within_margin_still_valid() {
        let future = u64::try_from(Utc::now().timestamp())
            .unwrap_or(0)
            .saturating_add(3600);
        let session = v2_session(future);
        assert!(!session.is_expired(Some(60)));
    }

    #[test]
    fn test_proactive_refresh_margin_v3_is_small_v2_is_large() {
        let v3 = v3_session(0);
        let v2 = v2_session(0);
        assert_eq!(
            proactive_refresh_margin_secs(&v3),
            crate::constants::PROACTIVE_REFRESH_MARGIN_V3_SECS
        );
        assert_eq!(
            proactive_refresh_margin_secs(&v2),
            crate::constants::PROACTIVE_REFRESH_MARGIN_V2_SECS
        );
        // v3's short-lived tokens get a tighter margin than v2's 6h sessions.
        assert!(proactive_refresh_margin_secs(&v3) < proactive_refresh_margin_secs(&v2));
    }

    #[tokio::test]
    async fn test_refresh_token_returns_cached_valid_session_without_login() {
        // refresh_token has an expiry gate: a comfortably-valid session is
        // returned unchanged, so no network login is attempted. This is the
        // behaviour that differs from force_refresh (which always re-logs in).
        let auth = Auth::new(Arc::new(Config::default()));
        let future = u64::try_from(Utc::now().timestamp())
            .unwrap_or(0)
            .saturating_add(3600);
        let seeded = v2_session(future);
        {
            let mut guard = auth.session.write().await;
            *guard = Some(seeded);
        }

        let refreshed = match auth.refresh_token().await {
            Ok(session) => session,
            Err(e) => panic!("refresh_token should return the cached session: {e}"),
        };
        // Same cached tokens returned: no re-authentication happened.
        assert_eq!(refreshed.account_id, "ACC123");
        assert_eq!(refreshed.cst.as_deref(), Some("CST-TOKEN"));
        assert_eq!(refreshed.expires_at, future);
    }

    #[test]
    fn test_force_refresh_is_public_and_present() {
        // Compile-time proof that the additive 401-path API exists. Invoking it
        // would perform a real login, so it is not called here (no network in
        // unit tests).
        let _ = Auth::force_refresh;
    }
}

#[cfg(test)]
mod oauth_login_guard_tests {
    use super::*;

    // Captured real IG demo v2 `/session` body (same shape as the existing
    // deserialization tests). Deserialized through the untagged
    // `SessionResponse` it lands on the V2 variant, so the derived session
    // carries no OAuth token — exactly the mismatch `login_oauth` must reject.
    const V2_BODY: &str = r#"{"accountType":"CFD","accountInfo":{"balance":21065.86,"deposit":3033.31,"profitLoss":-285.27,"available":16659.01},"currencyIsoCode":"EUR","currencySymbol":"E","currentAccountId":"ZZZZZ","lightstreamerEndpoint":"https://demo-apd.marketdatasystems.com","accounts":[{"accountId":"Z405P5","accountName":"Turbo24","preferred":false,"accountType":"PHYSICAL"},{"accountId":"ZHJ5N","accountName":"DEMO_A","preferred":false,"accountType":"CFD"},{"accountId":"ZZZZZ","accountName":"Opciones","preferred":true,"accountType":"CFD"}],"clientId":"101290216","timezoneOffset":1,"hasActiveDemoAccounts":true,"hasActiveLiveAccounts":true,"trailingStopsEnabled":false,"reroutingEnvironment":null,"dealingEnabled":true}"#;

    #[test]
    fn test_ensure_oauth_session_v2_body_on_v3_path_yields_unauthorized()
    -> Result<(), serde_json::Error> {
        // Deserialize a v2-shaped body the way `login_oauth` does after a v3
        // request, then run it through the same guard.
        let response: SessionResponse = serde_json::from_str(V2_BODY)?;
        let session = response.get_session();
        // A v2 body carries no OAuth token.
        assert!(!session.is_oauth());

        // The guard maps this to a typed error instead of panicking (the old
        // `assert!(session.is_oauth())` would have aborted here).
        match ensure_oauth_session(session) {
            Err(AppError::Unauthorized) => Ok(()),
            other => panic!("expected AppError::Unauthorized, got {other:?}"),
        }
    }

    #[test]
    fn test_ensure_oauth_session_oauth_body_passes_through() {
        // A genuine v3 session passes the guard unchanged.
        let session = Session {
            account_id: "ACC123".to_string(),
            client_id: "CLIENT1".to_string(),
            lightstreamer_endpoint: "demo-apd.marketdatasystems.com".to_string(),
            cst: None,
            x_security_token: None,
            oauth_token: Some(OAuthToken {
                access_token: "ACCESS".to_string(),
                refresh_token: "REFRESH".to_string(),
                scope: "read write".to_string(),
                token_type: "Bearer".to_string(),
                expires_in: "60".to_string(),
                created_at: Utc::now(),
            }),
            api_version: 3,
            expires_at: 0,
        };
        assert!(ensure_oauth_session(session).is_ok());
    }
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
