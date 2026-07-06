/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/10/25
******************************************************************************/

//! HTTP client and request execution for the IG Markets API.
//!
//! This module owns all outbound HTTP I/O: the shared `reqwest` client, rate
//! limiting, finite retry with backoff, and the automatic token
//! refresh-and-replay contract. It lives in the `application` layer because it
//! depends on `Auth`, `Session`, `Config` and `RateLimiter` — the pure `model`
//! layer must not perform I/O.

use crate::application::auth::{Auth, Session, WebsocketInfo};
use crate::application::config::Config;
use crate::application::rate_limiter::{RateLimitClass, RateLimiter};
use crate::constants::USER_AGENT;
use crate::error::AppError;
use crate::model::retry::RetryConfig;
use reqwest::Client as HttpInternalClient;
use reqwest::{Client, Method, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Simplified client for IG Markets API with automatic authentication
///
/// This client handles all authentication complexity internally, including:
/// - Initial login
/// - OAuth token refresh
/// - Re-authentication when tokens expire
/// - Account switching
/// - Rate limiting for all API requests
pub struct HttpClient {
    auth: Arc<Auth>,
    http_client: HttpInternalClient,
    config: Arc<Config>,
    // `RateLimiter` is `Clone` and already wraps each governor bucket in an
    // `Arc`, so it is stored directly: the limiter is configured once and never
    // write-swapped, so an outer `RwLock` would only add an allocation and an
    // await point (a read guard held across the pacing sleep) for no benefit.
    rate_limiter: RateLimiter,
}

impl HttpClient {
    /// Creates a new client and performs initial authentication
    ///
    /// # Arguments
    /// * `config` - Configuration containing credentials and API settings
    ///
    /// # Returns
    /// * `Ok(Client)` - Authenticated client ready to use
    /// * `Err(AppError)` - If authentication fails
    pub async fn new(config: Config) -> Result<Self, AppError> {
        let config = Arc::new(config);

        // Create HTTP client and rate limiter first
        let http_client = HttpInternalClient::builder()
            .user_agent(USER_AGENT)
            .build()?;
        let rate_limiter = RateLimiter::new(&config.rate_limiter);

        // Create Auth instance
        let auth = Arc::new(Auth::new(config.clone()));

        // Perform initial login
        auth.login().await?;

        Ok(Self {
            auth,
            http_client,
            config,
            rate_limiter,
        })
    }

    /// Creates a new client without performing initial authentication
    ///
    /// # Errors
    /// Returns `AppError::Network` if the HTTP client cannot be constructed.
    pub fn new_lazy(config: Config) -> Result<Self, AppError> {
        let config = Arc::new(config);

        // Create HTTP client and rate limiter first
        let http_client = HttpInternalClient::builder()
            .user_agent(USER_AGENT)
            .build()?;
        let rate_limiter = RateLimiter::new(&config.rate_limiter);

        // Create Auth instance via the fallible constructor so the whole
        // `new_lazy` path (and `Client::try_new` built on it) never panics.
        let auth = Arc::new(Auth::try_new(config.clone())?);

        Ok(Self {
            auth,
            http_client,
            config,
            rate_limiter,
        })
    }

    /// Gets WebSocket connection information for Lightstreamer, reusing the
    /// cached session.
    ///
    /// Delegates to [`Auth::ws_info`], which returns the cached session when it
    /// is valid and only logs in when needed.
    ///
    /// # Returns
    /// * `Ok(WebsocketInfo)` - Server endpoint, authentication tokens, and
    ///   account ID for the current session.
    /// * `Err(AppError)` - If session retrieval (login / refresh) fails.
    ///
    /// # Errors
    /// Returns [`AppError`] when the session cannot be retrieved.
    pub async fn ws_info(&self) -> Result<WebsocketInfo, AppError> {
        self.auth.ws_info().await
    }

    /// Gets WebSocket connection information for Lightstreamer
    ///
    /// # Returns
    /// * `WebsocketInfo` containing server endpoint, authentication tokens, and account ID
    #[deprecated(
        note = "use ws_info() which reuses the cached session and returns a typed error instead of a default-on-error WebsocketInfo"
    )]
    pub async fn get_ws_info(&self) -> WebsocketInfo {
        self.ws_info().await.unwrap_or_default()
    }

    /// Makes a GET request
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        version: Option<u8>,
    ) -> Result<T, AppError> {
        self.request(Method::GET, path, None::<()>, version).await
    }

    /// Makes a POST request
    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: B,
        version: Option<u8>,
    ) -> Result<T, AppError> {
        self.request(Method::POST, path, Some(body), version).await
    }

    /// Makes a PUT request
    pub async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: B,
        version: Option<u8>,
    ) -> Result<T, AppError> {
        self.request(Method::PUT, path, Some(body), version).await
    }

    /// Makes a DELETE request
    pub async fn delete<T: DeserializeOwned>(
        &self,
        path: &str,
        version: Option<u8>,
    ) -> Result<T, AppError> {
        self.request(Method::DELETE, path, None::<()>, version)
            .await
    }

    /// Makes a POST request with _method: DELETE header
    ///
    /// This is required by IG API for closing positions, as they don't support
    /// DELETE requests with a body. Instead, they use POST with a special header.
    ///
    /// # Arguments
    /// * `path` - API endpoint path
    /// * `body` - Request body to send
    /// * `version` - API version to use
    ///
    /// # Returns
    /// Deserialized response of type T
    pub async fn post_with_delete_method<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: B,
        version: Option<u8>,
    ) -> Result<T, AppError> {
        // IG requires POST + `_method: DELETE` for position closes; it rejects a
        // DELETE with a body. Everything else — URL construction, auth headers,
        // and the 401 refresh-and-replay contract — is identical to a normal
        // request, so it routes through the same wrapper with one extra header.
        self.request_with_refresh(
            Method::POST,
            path,
            Some(body),
            version,
            &[("_method", "DELETE")],
        )
        .await
    }

    /// Makes a request with custom API version
    pub async fn request<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        version: Option<u8>,
    ) -> Result<T, AppError> {
        self.request_with_refresh(method, path, body, version, &[])
            .await
    }

    /// Sends a request through the shared builder and applies the token
    /// refresh-and-replay contract exactly once.
    ///
    /// This is the single place the 401 / OAuth-token-expiry handling lives:
    /// both [`request`](Self::request) and
    /// [`post_with_delete_method`](Self::post_with_delete_method) route through
    /// here. On [`AppError::OAuthTokenExpired`] it forces a fresh login and
    /// replays the request one time. The match arm is not a loop: the replay
    /// happens exactly once, after which any further failure is returned.
    async fn request_with_refresh<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        version: Option<u8>,
        extra_headers: &[(&str, &str)],
    ) -> Result<T, AppError> {
        match self
            .request_internal(method.clone(), path, &body, version, extra_headers)
            .await
        {
            Ok(response) => self.parse_response(response).await,
            Err(AppError::OAuthTokenExpired) => {
                warn!("OAuth token expired, forcing refresh and retrying once");
                // Force a fresh login so the single replay below never resends
                // the same server-invalidated token. This match arm is not a
                // loop: the replay happens exactly once.
                self.auth.force_refresh().await?;
                let response = self
                    .request_internal(method, path, &body, version, extra_headers)
                    .await?;
                self.parse_response(response).await
            }
            Err(e) => Err(e),
        }
    }

    /// Builds and sends a single HTTP request against the IG API.
    ///
    /// Constructs the URL, assembles the common headers (API key, content type,
    /// version) plus the session auth headers (OAuth `Bearer` or v2
    /// `CST` / `X-SECURITY-TOKEN`), appends any `extra_headers` (e.g. IG's
    /// `_method: DELETE` for position closes), and dispatches through
    /// [`make_http_request`] with the finite default retry policy. It performs
    /// no token refresh — that is the caller's job via
    /// [`request_with_refresh`](Self::request_with_refresh).
    async fn request_internal<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: &Option<B>,
        version: Option<u8>,
        extra_headers: &[(&str, &str)],
    ) -> Result<Response, AppError> {
        let session = self.auth.get_session().await?;

        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            let path = path.trim_start_matches('/');
            format!("{}/{}", self.config.rest_api.base_url, path)
        };

        let version_owned = version.unwrap_or(1).to_string();
        let auth_header_value;

        // Borrow directly from `self.config` and the owned `session`, both of
        // which outlive this function, so no api_key / cst / token clone is
        // needed to build the header tuples.
        let mut headers = vec![
            ("X-IG-API-KEY", self.config.credentials.api_key.as_str()),
            ("Content-Type", "application/json; charset=UTF-8"),
            ("Accept", "application/json; charset=UTF-8"),
            ("Version", version_owned.as_str()),
        ];
        headers.extend_from_slice(extra_headers);

        if let Some(oauth) = &session.oauth_token {
            auth_header_value = format!("Bearer {}", oauth.access_token);
            headers.push(("Authorization", auth_header_value.as_str()));
            headers.push(("IG-ACCOUNT-ID", session.account_id.as_str()));
        } else if let (Some(cst_val), Some(token_val)) = (&session.cst, &session.x_security_token) {
            headers.push(("CST", cst_val.as_str()));
            headers.push(("X-SECURITY-TOKEN", token_val.as_str()));
        }

        make_http_request(
            &self.http_client,
            &self.rate_limiter,
            method,
            &url,
            headers,
            body,
            RetryConfig::default(),
        )
        .await
    }

    /// Parses response
    async fn parse_response<T: DeserializeOwned>(&self, response: Response) -> Result<T, AppError> {
        Ok(response.json().await?)
    }

    /// Switches to a different trading account
    pub async fn switch_account(
        &self,
        account_id: &str,
        default_account: Option<bool>,
    ) -> Result<(), AppError> {
        self.auth
            .switch_account(account_id, default_account)
            .await?;
        Ok(())
    }

    /// Gets the current session
    pub async fn get_session(&self) -> Result<Session, AppError> {
        self.auth.get_session().await
    }

    /// Logs out
    pub async fn logout(&self) -> Result<(), AppError> {
        self.auth.logout().await
    }

    /// Gets Auth reference
    pub fn auth(&self) -> &Auth {
        &self.auth
    }
}

impl Default for HttpClient {
    /// Creates a lazily-authenticated client with the default configuration.
    ///
    /// # Panics
    /// Panics if the underlying HTTP client cannot be constructed via
    /// [`HttpClient::new_lazy`] — typically a missing TLS backend, but also
    /// invalid proxy or certificate configuration. This is an unrecoverable
    /// startup invariant. Callers that need to handle that case gracefully
    /// should call [`HttpClient::new_lazy`] directly and propagate the returned
    /// [`AppError`] with `?`.
    fn default() -> Self {
        let config = Config::default();
        // Construction normally succeeds; it fails only if the reqwest client
        // cannot be built (no usable TLS backend, or invalid proxy/certificate
        // configuration), which is an unrecoverable startup invariant.
        Self::new_lazy(config).expect("failed to create default HTTP client")
    }
}

/// Makes an HTTP request with automatic rate limiting and retry on rate limit errors
///
/// This function provides a centralized way to make HTTP requests to the IG Markets API
/// with built-in rate limiting and automatic retry logic.
///
/// # Arguments
///
/// * `client` - The HTTP client to use for the request
/// * `rate_limiter` - Shared rate limiter (borrowed) to pace the request
/// * `method` - HTTP method (GET, POST, PUT, DELETE, etc.)
/// * `url` - Full URL to request
/// * `headers` - Vector of (header_name, header_value) tuples
/// * `body` - Optional request body (will be serialized to JSON)
/// * `retry_config` - Retry configuration (max retries and delay)
///
/// # Returns
///
/// * `Ok(Response)` - Successful HTTP response
/// * `Err(AppError)` - Error if request fails (excluding rate limit errors which are retried)
///
/// Retry is always finite: transient failures (429, 5xx, and IG allowance
/// rate limits) are retried with exponential backoff up to
/// `retry_config.max_retries()`; everything else fails fast. The 401
/// token-refresh path is handled by the caller, not here.
///
/// # Example
///
/// ```ignore
/// use ig_client::application::http::make_http_request;
/// use ig_client::model::retry::RetryConfig;
/// use reqwest::{Client, Method};
///
/// let client = Client::new();
/// let rate_limiter = RateLimiter::new(&config);
/// let headers = vec![
///     ("X-IG-API-KEY", "your-api-key"),
///     ("Content-Type", "application/json"),
/// ];
///
/// // Finite defaults (DEFAULT_MAX_RETRIES retries, exponential backoff)
/// let response = make_http_request(
///     &client,
///     &rate_limiter,
///     Method::GET,
///     "https://demo-api.ig.com/gateway/deal/markets/EPIC",
///     headers.clone(),
///     &None::<()>,
///     RetryConfig::default(),
/// ).await?;
///
/// // Maximum 3 retries with a 5 second base delay
/// let response = make_http_request(
///     &client,
///     &rate_limiter,
///     Method::GET,
///     "https://demo-api.ig.com/gateway/deal/markets/EPIC",
///     headers,
///     &None::<()>,
///     RetryConfig::with_max_retries_and_delay(3, 5),
/// ).await?;
/// ```
pub async fn make_http_request<B: Serialize>(
    client: &Client,
    rate_limiter: &RateLimiter,
    method: Method,
    url: &str,
    headers: Vec<(&str, &str)>,
    body: &Option<B>,
    retry_config: RetryConfig,
) -> Result<Response, AppError> {
    let max_retries = retry_config.max_retries();

    // Pace this request against the bucket for its endpoint class (trading /
    // historical / non-trading) so trading calls never queue behind bulk
    // non-trading traffic. The class is derived purely from the method + URL.
    let class = classify_endpoint(&method, url);

    // Bounded loop: `attempt` ranges over [0, max_retries]. Attempt 0 is the
    // first try; each further attempt is a retry. This can never loop forever.
    for attempt in 0..=max_retries {
        // Pace this request against its class bucket before sending. The limiter
        // is shared by reference; each governor bucket is internally `Arc`-backed
        // and parks the future until a slot is free, so there is no lock guard
        // held across this await.
        rate_limiter.wait_for(class).await;

        debug!(%method, %url, class = ?class, "http request");

        // Build request
        let mut request = client.request(method.clone(), url);

        // Add headers
        for (name, value) in &headers {
            request = request.header(*name, *value);
        }

        // Add body if present
        if let Some(b) = body {
            request = request.json(b);
        }

        // Send request
        let response = request.send().await?;
        let status = response.status();
        debug!(status = ?status, "http response");

        if status.is_success() {
            return Ok(response);
        }

        // Classify the failure into a retryable error or an immediate return.
        // Body-dependent statuses (401, 403) are handled inline; everything
        // else goes through the pure `classify_status` helper.
        let retryable_err: AppError = match status {
            StatusCode::FORBIDDEN => {
                let body_text = response.text().await.unwrap_or_default();

                // Historical data allowance is a weekly quota (default 10,000 data points).
                // Retrying is pointless — fail fast and let the caller decide.
                if body_text.contains("exceeded-account-historical-data-allowance") {
                    error!("historical data allowance exceeded (weekly quota exhausted)");
                    return Err(AppError::HistoricalDataAllowanceExceeded {
                        allowance_expiry: 0,
                    });
                }

                if body_text.contains("exceeded-api-key-allowance")
                    || body_text.contains("exceeded-account-allowance")
                    || body_text.contains("exceeded-account-trading-allowance")
                {
                    warn!(status = ?status, "allowance rate limit hit");
                    AppError::RateLimitExceeded
                } else {
                    error!(status = ?status, "forbidden");
                    return Err(AppError::Unexpected(status));
                }
            }
            StatusCode::UNAUTHORIZED => {
                let body_text = response.text().await.unwrap_or_default();
                if body_text.contains("oauth-token-invalid") {
                    // Surface to the caller so it can refresh the token and replay.
                    return Err(AppError::OAuthTokenExpired);
                }
                error!(status = ?status, "unauthorized");
                return Err(AppError::Unauthorized);
            }
            other => match classify_status(other) {
                StatusClass::Retryable => {
                    // Drain the body (without logging it) so reqwest can return
                    // the connection to the pool; an undrained body forces the
                    // connection closed and amplifies load during retry storms.
                    let _ = response.bytes().await;
                    if other == StatusCode::TOO_MANY_REQUESTS {
                        warn!(status = ?other, "rate limit (429) hit");
                        AppError::RateLimitExceeded
                    } else {
                        warn!(status = ?other, "server error");
                        AppError::Unexpected(other)
                    }
                }
                StatusClass::Permanent => {
                    error!(status = ?other, "request failed");
                    return Err(AppError::Unexpected(other));
                }
            },
        };

        // We have a transient failure. Retry with exponential backoff unless the
        // budget is exhausted (`attempt` here is < max_retries only when retrying).
        if attempt < max_retries {
            let delay = retry_config.delay_for_attempt(attempt);
            let delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
            warn!(
                attempt = attempt.saturating_add(1),
                max_retries, delay_ms, "retrying after transient failure"
            );
            tokio::time::sleep(delay).await;
            continue;
        }

        error!(max_retries, "retries exhausted after transient failures");
        return Err(retryable_err);
    }

    // Unreachable: `0..=max_retries` always yields at least one iteration and the
    // final iteration returns. Kept to satisfy the type checker without a panic.
    Err(AppError::RateLimitExceeded)
}

/// Classification of an HTTP status code for retry decisions.
///
/// Body-dependent statuses (401, 403) are handled separately in
/// [`make_http_request`]; this covers the status-only decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusClass {
    /// Transient failure: retry with backoff.
    Retryable,
    /// Permanent failure: return immediately.
    Permanent,
}

/// Classifies a non-success HTTP status as transient (retryable) or permanent.
///
/// Transient: `429 Too Many Requests` and any `5xx` server error. Everything
/// else (client errors other than 429) is permanent and fails fast.
#[must_use]
#[inline]
pub(crate) fn classify_status(status: StatusCode) -> StatusClass {
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        StatusClass::Retryable
    } else {
        StatusClass::Permanent
    }
}

/// Classifies an IG endpoint into its `RateLimitClass` from the HTTP method
/// and request path (or full URL).
///
/// Mapping:
/// - `POST` / `PUT` / `DELETE` on `positions/otc` or `workingorders/otc`
///   (order and position mutations, including position close via
///   `POST` + `_method: DELETE`) → `RateLimitClass::Trading`.
/// - Any path under `prices/` (historical price fetches) →
///   `RateLimitClass::Historical`.
/// - Everything else (market data, account queries, sentiment, watchlists,
///   working-order / position *reads*, …) → `RateLimitClass::NonTrading`.
///
/// `path` may be a bare path or a full URL; matching is by path substring, so
/// both `positions/otc` and `.../positions/otc/{deal_id}` classify as trading.
/// A `GET` on `positions` or `workingorders` is a read and stays non-trading.
#[must_use]
#[inline]
pub(crate) fn classify_endpoint(method: &Method, path: &str) -> RateLimitClass {
    let is_mutation = matches!(*method, Method::POST | Method::PUT | Method::DELETE);
    let is_trading_path = path.contains("positions/otc") || path.contains("workingorders/otc");

    if is_mutation && is_trading_path {
        RateLimitClass::Trading
    } else if path.contains("prices/") {
        RateLimitClass::Historical
    } else {
        RateLimitClass::NonTrading
    }
}

#[cfg(test)]
mod tests {
    use super::{StatusClass, classify_endpoint, classify_status};
    use crate::application::rate_limiter::RateLimitClass;
    use reqwest::{Method, StatusCode};

    const BASE: &str = "https://demo-api.ig.com/gateway/deal";

    #[test]
    fn test_classify_endpoint_post_positions_otc_is_trading() {
        assert_eq!(
            classify_endpoint(&Method::POST, &format!("{BASE}/positions/otc")),
            RateLimitClass::Trading
        );
    }

    #[test]
    fn test_classify_endpoint_get_prices_is_historical() {
        assert_eq!(
            classify_endpoint(&Method::GET, &format!("{BASE}/prices/CS.D.EURUSD.MINI.IP")),
            RateLimitClass::Historical
        );
    }

    #[test]
    fn test_classify_endpoint_get_markets_is_non_trading() {
        assert_eq!(
            classify_endpoint(&Method::GET, &format!("{BASE}/markets/CS.D.EURUSD.MINI.IP")),
            RateLimitClass::NonTrading
        );
    }

    #[test]
    fn test_classify_endpoint_put_position_update_is_trading() {
        // Position amend: PUT positions/otc/{deal_id}.
        assert_eq!(
            classify_endpoint(&Method::PUT, &format!("{BASE}/positions/otc/DIAAAABBBCCC")),
            RateLimitClass::Trading
        );
    }

    #[test]
    fn test_classify_endpoint_delete_working_order_is_trading() {
        assert_eq!(
            classify_endpoint(
                &Method::DELETE,
                &format!("{BASE}/workingorders/otc/DIAAAABBBCCC")
            ),
            RateLimitClass::Trading
        );
    }

    #[test]
    fn test_classify_endpoint_get_positions_read_is_non_trading() {
        // A GET on positions is a read, not a mutation, so it stays non-trading.
        assert_eq!(
            classify_endpoint(&Method::GET, &format!("{BASE}/positions")),
            RateLimitClass::NonTrading
        );
    }

    #[test]
    fn test_classify_status_429_is_retryable() {
        assert_eq!(
            classify_status(StatusCode::TOO_MANY_REQUESTS),
            StatusClass::Retryable
        );
    }

    #[test]
    fn test_classify_status_500_is_retryable() {
        assert_eq!(
            classify_status(StatusCode::INTERNAL_SERVER_ERROR),
            StatusClass::Retryable
        );
        assert_eq!(
            classify_status(StatusCode::BAD_GATEWAY),
            StatusClass::Retryable
        );
        assert_eq!(
            classify_status(StatusCode::SERVICE_UNAVAILABLE),
            StatusClass::Retryable
        );
    }

    #[test]
    fn test_classify_status_400_is_permanent() {
        assert_eq!(
            classify_status(StatusCode::BAD_REQUEST),
            StatusClass::Permanent
        );
        assert_eq!(
            classify_status(StatusCode::NOT_FOUND),
            StatusClass::Permanent
        );
        assert_eq!(
            classify_status(StatusCode::CONFLICT),
            StatusClass::Permanent
        );
    }
}
