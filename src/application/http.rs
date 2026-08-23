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
use crate::application::config::{Config, RateLimiterConfig};
use crate::application::rate_limiter::{RateLimitClass, RateLimiter};
use crate::constants::USER_AGENT;
use crate::error::AppError;
use crate::model::retry::RetryConfig;
use reqwest::Client as HttpInternalClient;
use reqwest::{Client, Method, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
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
    /// One entry per API key in the pool. `pool[0]` owns the same [`Auth`] and
    /// [`RateLimiter`] as the fields above, so a single-key configuration keeps
    /// the historical behaviour exactly.
    pool: Vec<KeySlot>,
    /// Budget shared by the whole pool.
    ///
    /// IG documents a per-account ceiling on top of the per-key one, and every
    /// key here authenticates the same account. Without this, N keys would pace
    /// N times the account's allowance and the account limit would be found the
    /// hard way.
    account_limiter: RateLimiter,
    /// Round-robin starting point for slot selection.
    ///
    /// Without it every caller scans the pool from index 0, so concurrent
    /// requests pile onto the first key while the rest sit idle. Advancing it
    /// per selection spreads equally-available keys evenly.
    cursor: AtomicUsize,
}

/// One API key of the pool, with the session and pacing budget that belong to it.
///
/// IG meters its non-trading allowance **per API key**, so each key needs both
/// its own session (`CST` / `X-SECURITY-TOKEN` are issued per key) and its own
/// rate limiter. Sharing either across keys would defeat the point: a single
/// bucket would pace the whole pool at one key's rate.
struct KeySlot {
    api_key: String,
    auth: Arc<Auth>,
    rate_limiter: RateLimiter,
    /// Set when IG rejects this key with an allowance error; the slot is skipped
    /// until the instant passes, so the pool stops handing work to a key that
    /// has already told us it is empty.
    cooldown_until: Arc<StdMutex<Option<Instant>>>,
}

/// How long a key is skipped after IG rejects it for exceeding its allowance.
///
/// The observed bucket refills at roughly its sustained rate, so a minute is
/// enough for a saturated key to become useful again without parking it for so
/// long that the pool shrinks under sustained load.
const KEY_COOLDOWN: Duration = Duration::from_secs(60);

/// IG's documented non-trading ceiling for one account, in requests per minute.
///
/// Measurement never reached it — four keys sustained 32/min with no rejection —
/// but it is the published limit, so the pool paces below it rather than
/// discovering it in production.
const ACCOUNT_MAX_REQUESTS_PER_MINUTE: u32 = 30;

/// Renders an API key for logs as its first eight characters.
///
/// Enough to tell the pool's keys apart when reading a trace, never enough to
/// use: an API key is a secret and must not reach the logs in full.
fn redact_key(api_key: &str) -> String {
    let head: String = api_key.chars().take(8).collect();
    format!("{head}…")
}

impl KeySlot {
    /// Whether this slot is currently skipped because IG rejected its key.
    fn in_cooldown(&self) -> bool {
        let guard = match self.cooldown_until.lock() {
            Ok(g) => g,
            // A poisoned mutex only ever held an `Option<Instant>`, so treating
            // the slot as available is safe and keeps the pool usable.
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.is_some_and(|until| Instant::now() < until)
    }

    /// When this slot's cooldown ends, if it is in one.
    fn cooldown_deadline(&self) -> Option<Instant> {
        let guard = match self.cooldown_until.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.filter(|&until| Instant::now() < until)
    }

    /// Marks the key as exhausted for [`KEY_COOLDOWN`].
    fn mark_exhausted(&self) {
        let until = Instant::now() + KEY_COOLDOWN;
        match self.cooldown_until.lock() {
            Ok(mut g) => *g = Some(until),
            Err(poisoned) => *poisoned.into_inner() = Some(until),
        }
    }
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
    ///
    /// # Errors
    /// Returns [`AppError::Network`] if the underlying `reqwest` client cannot
    /// be built (e.g. the system TLS backend fails to initialize), or any
    /// [`AppError`] surfaced by the initial [`Auth::login`] call.
    pub async fn new(config: Config) -> Result<Self, AppError> {
        let config = Arc::new(config);

        // Create HTTP client and rate limiter first
        let http_client = HttpInternalClient::builder()
            .user_agent(USER_AGENT)
            .build()?;
        // Build the pool first: every slot owns a single-key `Config`, so the
        // client's own `Auth` is slot 0's rather than one built from the raw
        // config, whose `api_key` may be the whole comma-separated list.
        let (auth, pool) = Self::build_pool(&config)?;
        let account_limiter = Self::build_account_limiter(&config, pool.len());

        // Perform initial login on the first key of the pool
        auth.login().await?;

        Ok(Self {
            auth,
            http_client,
            config,
            pool,
            account_limiter,
            cursor: AtomicUsize::new(0),
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
        // Same as `new`: the client's `Auth` is the pool's first slot, never an
        // `Auth` built from a config whose `api_key` is the whole pool list.
        let (auth, pool) = Self::build_pool(&config)?;
        let account_limiter = Self::build_account_limiter(&config, pool.len());

        Ok(Self {
            auth,
            http_client,
            config,
            pool,
            account_limiter,
            cursor: AtomicUsize::new(0),
        })
    }

    /// Builds one [`KeySlot`] per API key declared in the configuration.
    ///
    /// Every slot — the first included — gets a `Config` carrying **only its own
    /// key**. The first slot used to reuse the caller's `Auth`, which had been
    /// built from the original config: with a pool configured, that `Auth`
    /// carried the whole comma-separated list as its `api_key` and sent it as
    /// one 3 KB `X-IG-API-KEY` header, which IG answers with 403. The list is a
    /// pool specification, never a credential.
    ///
    /// Each key's session and its data requests share one limiter, because IG
    /// meters the login against the same per-key allowance as everything else.
    ///
    /// # Errors
    /// Returns [`AppError::Network`] if a key's [`Auth`] cannot be built.
    /// # Returns
    ///
    /// The pool and the first slot's [`Auth`], which the client adopts as its
    /// own. Returning it here is what guarantees the client never holds an
    /// `Auth` built from the raw multi-key config.
    fn build_pool(config: &Arc<Config>) -> Result<(Arc<Auth>, Vec<KeySlot>), AppError> {
        let keys = config.credentials.api_keys();
        // An empty list means the value held no usable key; keep the raw value
        // so the failure surfaces as IG rejecting it, not as an empty pool.
        let keys = if keys.is_empty() {
            vec![config.credentials.api_key.clone()]
        } else {
            keys
        };

        let mut first_auth: Option<Arc<Auth>> = None;
        let mut pool = Vec::with_capacity(keys.len());
        for key in &keys {
            let mut key_config = (**config).clone();
            key_config.credentials.api_key = key.clone();
            let key_config = Arc::new(key_config);

            let key_limiter = RateLimiter::new(&key_config.rate_limiter);
            let key_auth = Arc::new(Auth::with_rate_limiter(key_config, key_limiter.clone())?);
            if first_auth.is_none() {
                first_auth = Some(key_auth.clone());
            }
            pool.push(KeySlot {
                api_key: key.clone(),
                rate_limiter: key_limiter,
                auth: key_auth,
                cooldown_until: Arc::new(StdMutex::new(None)),
            });
        }

        if pool.len() > 1 {
            debug!(keys = pool.len(), "API key pool enabled");
        }

        // `keys` is non-empty by construction above, so the loop ran at least
        // once and `first_auth` is set; the fallback keeps this total without a
        // panic.
        let auth = match first_auth {
            Some(auth) => auth,
            None => Arc::new(Auth::try_new(config.clone())?),
        };
        Ok((auth, pool))
    }

    /// Builds the pool-wide budget that models IG's per-account ceiling.
    ///
    /// The per-key budget is what the config expresses, so the account's is
    /// derived from it: `keys x max_requests`, capped at
    /// [`ACCOUNT_MAX_REQUESTS_PER_MINUTE`]. With one key it is never the binding
    /// constraint, which keeps single-key behaviour unchanged; with many it
    /// stops the pool from pacing past what the account allows.
    fn build_account_limiter(config: &Arc<Config>, keys: usize) -> RateLimiter {
        let per_key = config.rate_limiter.max_requests;
        let keys = u32::try_from(keys).unwrap_or(u32::MAX);
        let aggregate = per_key.saturating_mul(keys);

        let period = config.rate_limiter.period_seconds.max(1);
        // Scale the documented per-minute ceiling to the configured period so
        // the comparison is like for like.
        let ceiling = u32::try_from(
            u64::from(ACCOUNT_MAX_REQUESTS_PER_MINUTE)
                .saturating_mul(period)
                .div_ceil(60),
        )
        .unwrap_or(u32::MAX)
        .max(1);

        let max_requests = aggregate.min(ceiling);
        RateLimiter::new(&RateLimiterConfig {
            max_requests,
            period_seconds: config.rate_limiter.period_seconds,
            // The burst is the bucket's capacity, so leaving the per-key burst
            // here would let the pool discharge far more than the ceiling in one
            // go and the cap would only bind on the average.
            burst_size: config.rate_limiter.burst_size.min(max_requests),
        })
    }

    /// Reserves a token from a usable key, waiting only if every key is empty.
    ///
    /// This is the proactive half of the pool: a key is left before it is
    /// rejected, not after. It returns the index of a slot whose limiter has
    /// already given up one token, so the caller owes exactly one request and
    /// must not pace it again.
    ///
    /// Trading never spreads. Order traffic is metered against the account, so
    /// rotating buys nothing, and keeping it on one key keeps a position's
    /// requests inside one session — which is what makes a reconciliation
    /// possible. Trading therefore always uses slot 0 and does not touch the
    /// cursor.
    ///
    /// For everything else, selection starts at a shared round-robin cursor so
    /// concurrent callers walk the pool from different points instead of piling
    /// onto the first key. It runs in two passes so that serving one request
    /// never authenticates the whole pool: first the keys that already hold a
    /// session, then — only if none of those had a token — a single key that
    /// still has to log in. When nothing can serve, the pool waits on every
    /// candidate at once and takes whichever refills first.
    ///
    /// # Returns
    ///
    /// The reserved slot's index, or `None` when every candidate was already
    /// tried for this request.
    ///
    /// # Errors
    ///
    /// Returns whatever [`Auth::get_session`] reports when the chosen key
    /// cannot authenticate, unless that is a per-key allowance rejection: that
    /// one parks the key and moves on, because another key can still serve.
    async fn reserve_slot(
        &self,
        class: RateLimitClass,
        tried: &[usize],
    ) -> Result<Option<usize>, AppError> {
        // Trading stays pinned to one slot, so two consecutive orders always
        // travel on the same key and the same session.
        if class == RateLimitClass::Trading {
            if tried.contains(&0) {
                return Ok(None);
            }
            self.account_limiter.reserve(class).await;
            self.pool[0].rate_limiter.reserve(class).await;
            self.pool[0].auth.get_session().await?;
            return Ok(Some(0));
        }

        let len = self.pool.len();
        let start = self.cursor.fetch_add(1, Ordering::Relaxed);

        let candidates: Vec<usize> = (0..len)
            .map(|offset| start.wrapping_add(offset) % len)
            .filter(|i| !tried.contains(i))
            .collect();
        if candidates.is_empty() {
            return Ok(None);
        }

        // The account-wide budget is shared by every key, so it is charged once
        // per request regardless of which key ends up serving it.
        self.account_limiter.reserve(class).await;

        // Pass 1: keys that can send without logging in first.
        for &i in &candidates {
            let slot = &self.pool[i];
            if slot.in_cooldown() || !slot.auth.has_ready_session().await {
                continue;
            }
            if slot.rate_limiter.try_reserve(class) {
                return Ok(Some(i));
            }
        }

        // Pass 2: at most one key is authenticated, and only when no ready key
        // had a token. A login that hits this key's allowance parks it and the
        // next candidate is tried, rather than burning three backoffs here.
        for &i in &candidates {
            let slot = &self.pool[i];
            if slot.in_cooldown() || slot.auth.has_ready_session().await {
                continue;
            }
            match slot.auth.get_session().await {
                Ok(_) => {}
                Err(AppError::ApiKeyAllowanceExceeded) if self.pool.len() > 1 => {
                    slot.mark_exhausted();
                    warn!(
                        key = %redact_key(&slot.api_key),
                        "API key allowance exhausted during login, trying another key"
                    );
                    continue;
                }
                Err(e) => return Err(e),
            }
            if slot.rate_limiter.try_reserve(class) {
                return Ok(Some(i));
            }
            // Authenticated but out of tokens: fall through to the wait below
            // rather than logging in yet another key.
            break;
        }

        // Nothing can serve now. Wait on the keys that are not cooling down; if
        // every one of them is, wait out the shortest cooldown first so the
        // pool honours it instead of hammering a key IG has already refused.
        let live: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&i| !self.pool[i].in_cooldown())
            .collect();

        let waiting = if live.is_empty() {
            if let Some(until) = candidates
                .iter()
                .filter_map(|&i| self.pool[i].cooldown_deadline())
                .min()
            {
                let now = Instant::now();
                if until > now {
                    debug!(
                        wait_ms = (until - now).as_millis(),
                        "every key is cooling down"
                    );
                    tokio::time::sleep(until - now).await;
                }
            }
            candidates
        } else {
            live
        };

        // Prefer keys that already hold a session: if one of those refills
        // first, serving this request costs no extra login.
        let mut ready = Vec::with_capacity(waiting.len());
        for &i in &waiting {
            if self.pool[i].auth.has_ready_session().await {
                ready.push(i);
            }
        }
        let waiting = if ready.is_empty() { waiting } else { ready };

        let waits: Vec<_> = waiting
            .iter()
            .map(|&i| {
                let slot = &self.pool[i];
                Box::pin(async move {
                    slot.rate_limiter.reserve(class).await;
                    i
                })
            })
            .collect();

        let (winner, _, _) = futures::future::select_all(waits).await;

        // The winner's token is already spent, so a login failure here does cost
        // it. Establishing the session earlier is not possible: which key wins
        // is only known once one refills.
        self.pool[winner].auth.get_session().await?;
        Ok(Some(winner))
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
        // The refresh-and-replay lives in `request_internal`, which knows which
        // slot's session IG rejected. Refreshing here would always refresh slot
        // 0, so an expired token on any other key of the pool would be replayed
        // unchanged and fail again.
        let response = self
            .request_internal(method, path, &body, version, extra_headers)
            .await?;
        self.parse_response(response).await
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
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            let path = path.trim_start_matches('/');
            format!("{}/{}", self.config.rest_api.base_url, path)
        };

        let class = classify_endpoint(&method, path);

        // Only non-trading REST rotates. Order placement, amendment and closure
        // stay on one key: they are metered against the account, not the key, so
        // moving them buys nothing and would spread order traffic over sessions
        // that a reconciliation has to tell apart afterwards.
        let may_rotate = class == RateLimitClass::NonTrading && self.pool.len() > 1;

        let mut tried: Vec<usize> = Vec::with_capacity(self.pool.len());
        // One replay after a forced refresh, never a loop of them.
        let mut replayed = false;

        loop {
            let Some(idx) = self.reserve_slot(class, &tried).await? else {
                // Every key has been tried for this request. The last attempt's
                // error was returned below, so reaching here means the pool ran
                // out of candidates without one; report the key-level rejection.
                return Err(AppError::ApiKeyAllowanceExceeded);
            };
            tried.push(idx);
            let slot = &self.pool[idx];

            // With another key left to try, a rejected request is cheaper to
            // move than to wait out this one's backoff, so the per-request retry
            // budget is dropped and rotation handles it.
            let rotate = may_rotate && tried.len() < self.pool.len();
            let retry = if rotate {
                RetryConfig {
                    max_retry_count: Some(0),
                    retry_delay_secs: None,
                }
            } else {
                RetryConfig::default()
            };

            let result = self
                .request_on_slot(slot, &method, &url, body, version, extra_headers, retry)
                .await;

            match result {
                // Per-key allowance: this key is spent. Mark it either way, so a
                // key IG has just refused is not handed the next request even
                // when there is nowhere left to rotate for this one.
                Err(AppError::ApiKeyAllowanceExceeded) => {
                    slot.mark_exhausted();
                    if !rotate {
                        return Err(AppError::ApiKeyAllowanceExceeded);
                    }
                    warn!(
                        key = %redact_key(&slot.api_key),
                        of = self.pool.len(),
                        "API key allowance exhausted, rotating to another key"
                    );
                }
                // The token IG rejected belongs to this slot's session, so the
                // refresh has to happen on that slot - not on the client's own
                // `auth`, which is slot 0 and may be a different key entirely.
                Err(AppError::OAuthTokenExpired) if !replayed => {
                    warn!(
                        key = %redact_key(&slot.api_key),
                        "OAuth token expired, refreshing this key's session and replaying once"
                    );
                    slot.auth.force_refresh().await?;
                    replayed = true;
                    tried.pop();
                }
                // Account and trading allowances belong to the account every key
                // authenticates, and a bare 429 does not say which budget ran
                // out. None of them justify burning another key.
                other => return other,
            }
        }
    }

    /// Issues one request through a specific key slot: its session, its API key
    /// and its own rate-limiter budget.
    ///
    /// The caller has already reserved this slot's token, so the send does not
    /// pace itself again. The session is resolved *before* that token is put on
    /// the wire, so a failed login costs only the login request it made — never
    /// an extra token for a data request that is never sent.
    #[allow(clippy::too_many_arguments)]
    async fn request_on_slot<B: Serialize>(
        &self,
        slot: &KeySlot,
        method: &Method,
        url: &str,
        body: &Option<B>,
        version: Option<u8>,
        extra_headers: &[(&str, &str)],
        retry: RetryConfig,
    ) -> Result<Response, AppError> {
        let session = slot.auth.get_session().await?;

        let version_owned = version.unwrap_or(1).to_string();
        let auth_header_value;

        // Borrow from the slot and the owned `session`, both of which outlive
        // this function, so no api_key / cst / token clone is needed to build
        // the header tuples.
        let mut headers = vec![
            ("X-IG-API-KEY", slot.api_key.as_str()),
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

        make_http_request_reserved(
            &self.http_client,
            &slot.rate_limiter,
            method.clone(),
            url,
            headers,
            body,
            retry,
            true,
        )
        .await
    }

    /// Deserializes a successful HTTP response body into the target DTO,
    /// attaching request context when parsing fails.
    ///
    /// On a deserialization failure the returned [`AppError::Deserialization`]
    /// names the endpoint URL, the HTTP status, and the serde error, so DTO
    /// drift is diagnosable instead of surfacing as a bare serde message.
    ///
    /// For non-`/session` endpoints a truncated body snippet is appended to the
    /// message. The `/session` endpoints are auth-adjacent — their bodies can
    /// carry credentials / CST / X-SECURITY-TOKEN / OAuth tokens — so their body
    /// is deliberately never echoed into the error (status + URL + serde error
    /// only).
    ///
    /// # Errors
    /// Returns [`AppError::Network`] if the body cannot be read, and
    /// [`AppError::Deserialization`] if the body cannot be parsed into `T`.
    async fn parse_response<T: DeserializeOwned>(&self, response: Response) -> Result<T, AppError> {
        let status = response.status();
        let url = response.url().clone();
        // Buffer the body once so a parse failure can be reported with context;
        // `json()` would consume the body and leave nothing to snippet.
        let text = response.text().await?;

        serde_json::from_str(&text).map_err(|e| {
            // `/session` bodies are auth-adjacent and may carry tokens: never
            // echo them. Every other endpoint gets a truncated snippet to help
            // diagnose DTO drift against the real IG payload.
            if is_auth_endpoint(url.path()) {
                AppError::Deserialization(format!("failed to deserialize {url} ({status}): {e}"))
            } else {
                let snippet = truncate_body_snippet(&text);
                AppError::Deserialization(format!(
                    "failed to deserialize {url} ({status}): {e}; body: {snippet}"
                ))
            }
        })
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

    /// Returns the configuration this HTTP client was built with.
    ///
    /// `Config`'s `Debug` / `Display` impls redact credentials and the database
    /// URL, so the returned value can be rendered that way without leaking
    /// secrets. Its `Serialize` impl does **not** redact — never serialize a
    /// `Config` into logs, telemetry or an error payload.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
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
    make_http_request_reserved(
        client,
        rate_limiter,
        method,
        url,
        headers,
        body,
        retry_config,
        false,
    )
    .await
}

/// Same as [`make_http_request`], but told whether the first send already owns
/// a token.
///
/// `reserved = true` means the caller took a cell from `rate_limiter` for this
/// request and the first attempt must not take another. Retries always pace
/// themselves: each one is a fresh request as far as IG is concerned.
#[allow(clippy::too_many_arguments)]
pub async fn make_http_request_reserved<B: Serialize>(
    client: &Client,
    rate_limiter: &RateLimiter,
    method: Method,
    url: &str,
    headers: Vec<(&str, &str)>,
    body: &Option<B>,
    retry_config: RetryConfig,
    reserved: bool,
) -> Result<Response, AppError> {
    let max_retries = retry_config.max_retries();

    // Pace this request against the bucket for its endpoint class (trading /
    // historical / non-trading) so trading calls never queue behind bulk
    // non-trading traffic. The class is derived purely from the method + URL.
    let class = classify_endpoint(&method, url);

    // Bounded loop: `attempt` ranges over [0, max_retries]. Attempt 0 is the
    // first try; each further attempt is a retry. This can never loop forever.
    for attempt in 0..=max_retries {
        // Pace this request against its class bucket before sending, unless the
        // caller already reserved the token for this first send. Reserving and
        // then waiting again would spend two cells on one request and halve the
        // effective rate. Every retry is a *new* request to IG and pays its own
        // token; the limiter is shared by reference and each governor bucket is
        // internally `Arc`-backed, so no lock guard is held across this await.
        if attempt > 0 || !reserved {
            rate_limiter.wait_for(class).await;
        }

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

                // Which allowance ran out decides what the caller may do about
                // it, so each one gets its own error instead of collapsing into
                // a single "rate limited".
                if body_text.contains("exceeded-api-key-allowance") {
                    // Fail fast rather than spending ~76 s of backoff on a key
                    // that has just said it is empty. With a pool the caller
                    // rotates immediately - including on `/session`, where the
                    // backoff used to be paid before any rotation could happen -
                    // and with a single key the caller learns sooner.
                    warn!("api key allowance exceeded");
                    return Err(AppError::ApiKeyAllowanceExceeded);
                } else if body_text.contains("exceeded-account-trading-allowance") {
                    warn!("account trading allowance exceeded");
                    return Err(AppError::TradingAllowanceExceeded);
                } else if body_text.contains("exceeded-account-allowance") {
                    // Every key of a pool authenticates this same account, so
                    // retrying here only spends more of an allowance that is
                    // already gone. Fail fast and let the caller back off.
                    warn!("account allowance exceeded");
                    return Err(AppError::AccountAllowanceExceeded);
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
                        // No body to say which budget ran out, so this stays
                        // generic: reading it as a per-key rejection would burn
                        // keys for an account-wide limit.
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

/// Maximum number of characters of a response body echoed into a
/// deserialization error message.
///
/// Long enough to spot the offending field against the real IG payload, short
/// enough to keep error messages and logs bounded.
const BODY_SNIPPET_MAX_CHARS: usize = 500;

/// Returns whether `path` targets the auth-adjacent `/session` endpoint, whose
/// response body can carry credentials / session tokens and must never be
/// echoed into an error message.
#[must_use]
#[inline]
fn is_auth_endpoint(path: &str) -> bool {
    path.contains("/session")
}

/// Truncates a response body to at most [`BODY_SNIPPET_MAX_CHARS`] characters
/// for inclusion in an error message.
///
/// Truncation is on `char` boundaries so it never splits a UTF-8 code point;
/// a truncation marker is appended when the body was longer than the limit.
#[must_use]
#[inline]
fn truncate_body_snippet(body: &str) -> String {
    let truncated = match body.char_indices().nth(BODY_SNIPPET_MAX_CHARS) {
        // `idx` is the byte offset of the (limit+1)-th char, so `..idx` keeps
        // exactly `BODY_SNIPPET_MAX_CHARS` chars on a valid boundary.
        Some((idx, _)) => format!("{}... (truncated)", &body[..idx]),
        None => body.to_string(),
    };
    // Keep the snippet on one line: the error string is logged, so raw
    // newlines / control characters would fragment the log record and allow
    // log-injection-style confusion. Escape CR/LF/TAB to their literal forms.
    truncated
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
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
    use super::{
        Arc, Auth, Config, Duration, Instant, KeySlot, RateLimiter, StatusClass, StdMutex,
        classify_endpoint, classify_status, redact_key,
    };
    use crate::application::config::Credentials;
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

    #[test]
    fn test_truncate_body_snippet_short_body_is_unchanged() {
        let body = r#"{"errorCode":"validation.null-not-allowed.request.epic"}"#;
        assert_eq!(super::truncate_body_snippet(body), body);
    }

    #[test]
    fn test_truncate_body_snippet_long_body_is_truncated_on_char_boundary() {
        // A multi-byte char repeated past the limit must not be split.
        let body = "é".repeat(super::BODY_SNIPPET_MAX_CHARS + 50);
        let snippet = super::truncate_body_snippet(&body);
        assert!(snippet.ends_with("... (truncated)"));
        // The kept prefix is exactly the char limit (each `é` is 2 bytes).
        let kept = snippet.trim_end_matches("... (truncated)");
        assert_eq!(kept.chars().count(), super::BODY_SNIPPET_MAX_CHARS);
    }

    #[test]
    fn test_is_auth_endpoint_matches_session_paths_only() {
        assert!(super::is_auth_endpoint("/gateway/deal/session"));
        assert!(super::is_auth_endpoint("/session"));
        assert!(!super::is_auth_endpoint(
            "/gateway/deal/markets/CS.D.EURUSD.MINI.IP"
        ));
    }

    /// A DTO with a required field, used to force a deserialization failure
    /// against an unexpected IG payload shape.
    #[derive(Debug, serde::Deserialize)]
    struct RequiredFieldDto {
        #[allow(dead_code)]
        instrument_type: String,
    }

    #[tokio::test]
    async fn test_parse_response_malformed_body_includes_status_and_snippet() {
        use super::HttpClient;
        use crate::error::AppError;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        // A 200 whose body does not match the target DTO (DTO drift).
        Mock::given(method("GET"))
            .and(path("/markets/CS.D.EURUSD.MINI.IP"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"unexpectedField":"surprise","another":"drifted"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let url = format!("{}/markets/CS.D.EURUSD.MINI.IP", server.uri());
        let response = reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .expect("request should reach the mock server");

        let client = HttpClient::new_lazy(crate::application::config::Config::default())
            .expect("lazy HTTP client construction should succeed");
        let result: Result<RequiredFieldDto, AppError> = client.parse_response(response).await;

        let msg = match result {
            Err(AppError::Deserialization(msg)) => msg,
            other => panic!("expected AppError::Deserialization, got {other:?}"),
        };
        // Status, endpoint, and a body snippet all present.
        assert!(
            msg.contains("200"),
            "error should carry the HTTP status: {msg}"
        );
        assert!(
            msg.contains("/markets/"),
            "error should carry the endpoint URL: {msg}"
        );
        assert!(
            msg.contains("body:"),
            "error should carry a body snippet: {msg}"
        );
        assert!(
            msg.contains("unexpectedField"),
            "error should include the malformed body snippet: {msg}"
        );
    }

    #[tokio::test]
    async fn test_parse_response_session_endpoint_omits_body_snippet() {
        use super::HttpClient;
        use crate::error::AppError;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // A /session body that fails to deserialize into the target DTO but
        // carries a token-shaped secret. The error must NOT echo the body.
        const SECRET: &str = "SUPER-SECRET-OAUTH-TOKEN-VALUE";
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/session"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                format!(r#"{{"oauthToken":{{"access_token":"{SECRET}"}}}}"#),
                "application/json",
            ))
            .mount(&server)
            .await;

        let url = format!("{}/session", server.uri());
        let response = reqwest::Client::new()
            .post(&url)
            .send()
            .await
            .expect("request should reach the mock server");

        let client = HttpClient::new_lazy(crate::application::config::Config::default())
            .expect("lazy HTTP client construction should succeed");
        let result: Result<RequiredFieldDto, AppError> = client.parse_response(response).await;

        let msg = match result {
            Err(AppError::Deserialization(msg)) => msg,
            other => panic!("expected AppError::Deserialization, got {other:?}"),
        };
        // Status and endpoint are present for diagnosis...
        assert!(
            msg.contains("200"),
            "error should carry the HTTP status: {msg}"
        );
        assert!(
            msg.contains("/session"),
            "error should carry the endpoint URL: {msg}"
        );
        // ...but the auth-adjacent body (and any token in it) is NOT echoed.
        assert!(
            !msg.contains("body:"),
            "session errors must not include a body snippet: {msg}"
        );
        assert!(
            !msg.contains(SECRET),
            "session errors must never leak token material: {msg}"
        );
    }

    #[test]
    fn test_redact_key_shows_only_a_prefix() {
        let key = "6cb0ae4d738dcf918fa47157858fc0ea11290a5b";
        let shown = redact_key(key);
        assert_eq!(shown, "6cb0ae4d…");
        assert!(
            !shown.contains("738dcf91"),
            "the key body must never be logged"
        );
    }

    #[test]
    fn test_redact_key_handles_short_and_empty_keys() {
        assert_eq!(redact_key("abc"), "abc…");
        assert_eq!(redact_key(""), "…");
    }

    fn slot(api_key: &str) -> KeySlot {
        let config = Arc::new(Config::from_credentials(Credentials::new(
            "user".into(),
            "pass".into(),
            "ACC".into(),
            api_key.into(),
        )));
        KeySlot {
            api_key: api_key.to_string(),
            rate_limiter: RateLimiter::new(&config.rate_limiter),
            auth: Arc::new(Auth::try_new(config).expect("auth builds in tests")),
            cooldown_until: Arc::new(StdMutex::new(None)),
        }
    }

    #[test]
    fn test_key_slot_cooldown_marks_and_expires() {
        let s = slot("key-a");
        assert!(!s.in_cooldown(), "a fresh slot is available");

        s.mark_exhausted();
        assert!(s.in_cooldown(), "a rejected key is skipped");

        // Simulate the cooldown having elapsed.
        *s.cooldown_until.lock().expect("lock") = Some(Instant::now() - Duration::from_secs(1));
        assert!(
            !s.in_cooldown(),
            "the key returns to the pool once it refills"
        );
    }
}
