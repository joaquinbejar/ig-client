/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/10/25
******************************************************************************/

use crate::application::auth::{Auth, Session, WebsocketInfo};
use crate::application::config::Config;
use crate::application::rate_limiter::RateLimiter;
use crate::error::AppError;
use crate::model::retry::RetryConfig;
use reqwest::Client as HttpInternalClient;
use reqwest::{Client, Method, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

const USER_AGENT: &str = "ig-client/0.6.0";

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
    rate_limiter: Arc<RwLock<RateLimiter>>,
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
        let rate_limiter = Arc::new(RwLock::new(RateLimiter::new(&config.rate_limiter)));

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
        let rate_limiter = Arc::new(RwLock::new(RateLimiter::new(&config.rate_limiter)));

        // Create Auth instance
        let auth = Arc::new(Auth::new(config.clone()));

        Ok(Self {
            auth,
            http_client,
            config,
            rate_limiter,
        })
    }

    /// Gets WebSocket connection information for Lightstreamer
    ///
    /// # Returns
    /// * `WebsocketInfo` containing server endpoint, authentication tokens, and account ID
    pub async fn get_ws_info(&self) -> WebsocketInfo {
        self.auth.get_ws_info().await
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
        match self
            .request_internal_with_delete_method(path, &body, version)
            .await
        {
            Ok(response) => self.parse_response(response).await,
            Err(AppError::OAuthTokenExpired) => {
                warn!("OAuth token expired, refreshing and retrying");
                self.auth.refresh_token().await?;
                let response = self
                    .request_internal_with_delete_method(path, &body, version)
                    .await?;
                self.parse_response(response).await
            }
            Err(e) => Err(e),
        }
    }

    /// Makes a request with custom API version
    pub async fn request<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        version: Option<u8>,
    ) -> Result<T, AppError> {
        match self
            .request_internal(method.clone(), path, &body, version)
            .await
        {
            Ok(response) => self.parse_response(response).await,
            Err(AppError::OAuthTokenExpired) => {
                warn!("OAuth token expired, refreshing and retrying");
                self.auth.refresh_token().await?;
                let response = self.request_internal(method, path, &body, version).await?;
                self.parse_response(response).await
            }
            Err(e) => Err(e),
        }
    }

    /// Internal method to make HTTP requests
    async fn request_internal<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: &Option<B>,
        version: Option<u8>,
    ) -> Result<Response, AppError> {
        let session = self.auth.get_session().await?;

        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            let path = path.trim_start_matches('/');
            format!("{}/{}", self.config.rest_api.base_url, path)
        };

        let api_key = self.config.credentials.api_key.clone();
        let version_owned = version.unwrap_or(1).to_string();
        let auth_header_value;
        let account_id;
        let cst;
        let x_security_token;

        let mut headers = vec![
            ("X-IG-API-KEY", api_key.as_str()),
            ("Content-Type", "application/json; charset=UTF-8"),
            ("Accept", "application/json; charset=UTF-8"),
            ("Version", version_owned.as_str()),
        ];

        if let Some(oauth) = &session.oauth_token {
            auth_header_value = format!("Bearer {}", oauth.access_token);
            account_id = session.account_id.clone();
            headers.push(("Authorization", auth_header_value.as_str()));
            headers.push(("IG-ACCOUNT-ID", account_id.as_str()));
        } else if let (Some(cst_val), Some(token_val)) = (&session.cst, &session.x_security_token) {
            cst = cst_val.clone();
            x_security_token = token_val.clone();
            headers.push(("CST", cst.as_str()));
            headers.push(("X-SECURITY-TOKEN", x_security_token.as_str()));
        }

        make_http_request(
            &self.http_client,
            self.rate_limiter.clone(),
            method,
            &url,
            headers,
            body,
            RetryConfig::default(),
        )
        .await
    }

    /// Internal method to make POST requests with _method: DELETE header
    ///
    /// This is required by IG API for closing positions
    async fn request_internal_with_delete_method<B: Serialize>(
        &self,
        path: &str,
        body: &B,
        version: Option<u8>,
    ) -> Result<Response, AppError> {
        let session = self.auth.get_session().await?;

        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            let path = path.trim_start_matches('/');
            format!("{}/{}", self.config.rest_api.base_url, path)
        };

        let api_key = self.config.credentials.api_key.clone();
        let version_owned = version.unwrap_or(1).to_string();
        let auth_header_value;
        let account_id;
        let cst;
        let x_security_token;

        let mut headers = vec![
            ("X-IG-API-KEY", api_key.as_str()),
            ("Content-Type", "application/json; charset=UTF-8"),
            ("Accept", "application/json; charset=UTF-8"),
            ("Version", version_owned.as_str()),
            ("_method", "DELETE"), // Special header for IG API
        ];

        if let Some(oauth) = &session.oauth_token {
            auth_header_value = format!("Bearer {}", oauth.access_token);
            account_id = session.account_id.clone();
            headers.push(("Authorization", auth_header_value.as_str()));
            headers.push(("IG-ACCOUNT-ID", account_id.as_str()));
        } else if let (Some(cst_val), Some(token_val)) = (&session.cst, &session.x_security_token) {
            cst = cst_val.clone();
            x_security_token = token_val.clone();
            headers.push(("CST", cst.as_str()));
            headers.push(("X-SECURITY-TOKEN", x_security_token.as_str()));
        }

        make_http_request(
            &self.http_client,
            self.rate_limiter.clone(),
            Method::POST, // Always POST for this method
            &url,
            headers,
            &Some(body),
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
    fn default() -> Self {
        let config = Config::default();
        // SAFETY: Default TLS configuration should always succeed.
        // This only fails if the system has no TLS backend, which is unrecoverable.
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
/// * `rate_limiter` - Shared rate limiter to control request rate
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
/// use ig_client::model::http::make_http_request;
/// use ig_client::model::retry::RetryConfig;
/// use reqwest::{Client, Method};
/// use std::sync::Arc;
/// use tokio::sync::RwLock;
///
/// let client = Client::new();
/// let rate_limiter = Arc::new(RwLock::new(RateLimiter::new(&config)));
/// let headers = vec![
///     ("X-IG-API-KEY", "your-api-key"),
///     ("Content-Type", "application/json"),
/// ];
///
/// // Finite defaults (DEFAULT_MAX_RETRIES retries, exponential backoff)
/// let response = make_http_request(
///     &client,
///     rate_limiter.clone(),
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
///     rate_limiter,
///     Method::GET,
///     "https://demo-api.ig.com/gateway/deal/markets/EPIC",
///     headers,
///     &None::<()>,
///     RetryConfig::with_max_retries_and_delay(3, 5),
/// ).await?;
/// ```
pub async fn make_http_request<B: Serialize>(
    client: &Client,
    rate_limiter: Arc<RwLock<RateLimiter>>,
    method: Method,
    url: &str,
    headers: Vec<(&str, &str)>,
    body: &Option<B>,
    retry_config: RetryConfig,
) -> Result<Response, AppError> {
    let max_retries = retry_config.max_retries();

    // Bounded loop: `attempt` ranges over [0, max_retries]. Attempt 0 is the
    // first try; each further attempt is a retry. This can never loop forever.
    for attempt in 0..=max_retries {
        // Wait for rate limiter before making request
        {
            let limiter = rate_limiter.read().await;
            limiter.wait().await;
        }

        debug!(%method, %url, "http request");

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

#[cfg(test)]
mod tests {
    use super::{StatusClass, classify_status};
    use reqwest::StatusCode;

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
