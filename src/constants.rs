/// Default number of days to look back when fetching historical data
pub const DAYS_TO_BACK_LOOK: i64 = 10;
/// Maximum number of consecutive errors before forcing a cooldown
pub const MAX_CONSECUTIVE_ERRORS: u32 = 3;
/// Cooldown time in seconds when hitting max errors (5 minutes)
pub const ERROR_COOLDOWN_SECONDS: u64 = 300;
/// Default sleep time in hours if not specified in environment (24 hours)
pub const DEFAULT_SLEEP_TIME: u64 = 24;
/// Default page size for API requests
pub const DEFAULT_PAGE_SIZE: u32 = 50;
/// Default maximum number of retries for transient HTTP failures when the
/// `MAX_RETRY_COUNT` environment variable is unset.
///
/// This default is intentionally finite: unbounded retry was removed in
/// PR #26 and must never be reintroduced. "No retry count configured" now
/// means "use this finite default", never "retry forever".
pub const DEFAULT_MAX_RETRIES: u32 = 3;
/// Default base delay in seconds between retries when the `RETRY_DELAY_SECS`
/// environment variable is unset. Used as the base for exponential backoff.
pub const DEFAULT_RETRY_DELAY_SECS: u64 = 10;
/// Maximum backoff delay in seconds. Exponential backoff (`base * 2^attempt`)
/// saturates at this cap — the cap is the policy, so rounding down to it is
/// intentional and documented.
pub const MAX_RETRY_DELAY_SECS: u64 = 60;
/// Compatibility retry cap for the deprecated "infinite" retry constructors
/// ([`crate::model::retry::RetryConfig::infinite`] and
/// [`crate::model::retry::RetryConfig::with_delay`]). Unbounded retry is banned
/// (PR #26); these constructors now clamp to this large-but-finite value so
/// existing callers keep compiling while never looping forever.
pub const DEPRECATED_INFINITE_RETRY_CAP: u32 = 1000;
/// Base delay in milliseconds used for proximity-based delays in the rate limiter
/// This value is used to calculate wait times when approaching rate limits
pub const BASE_DELAY_MS: u64 = 1000;
/// Additional safety buffer in milliseconds added to wait times
/// This provides extra margin to ensure rate limits are not exceeded
pub const SAFETY_BUFFER_MS: u64 = 1000;
/// User agent string used in HTTP requests to identify this client to the IG Markets API
pub const USER_AGENT: &str = "Rust-IG-Client/0.1.9";
/// Conservative per-app trading request budget, in requests per second,
/// enforced by the rate limiter for order / position mutations
/// (`positions/otc`, `workingorders/otc`).
///
/// IG applies account-level bans for trading rate violations and the published
/// per-app trading limit is roughly one request per second, so this budget is
/// fixed independently of the configured non-trading budget: a permissive
/// `RateLimiterConfig` can never loosen it.
pub const TRADING_RATE_LIMIT_PER_SECOND: u32 = 1;
/// Conservative per-app historical-price request budget, in requests per second,
/// enforced by the rate limiter for endpoints under `prices/`.
///
/// Historical price fetches also draw down a weekly data-point allowance (default
/// 10,000 points), so the per-second rate is kept low to avoid exhausting the
/// allowance in bursts. Like the trading budget, it is fixed independently of the
/// configured non-trading budget.
pub const HISTORICAL_RATE_LIMIT_PER_SECOND: u32 = 1;
/// Burst capacity for the derived trading and historical rate-limit buckets.
///
/// Kept at one so the derived per-second budgets admit no burst beyond a single
/// in-flight request, matching IG's strict trading / historical limits.
pub const TRADING_HISTORICAL_BURST_SIZE: u32 = 1;
/// Fallback replenishment budget (requests per period) used when
/// [`crate::application::config::RateLimiterConfig::max_requests`] is configured
/// as zero, which is structurally invalid.
///
/// One request per configured period is the safe floor and avoids a
/// divide-by-zero when computing the per-cell replenishment interval.
pub const FALLBACK_RATE_LIMIT_MAX_REQUESTS: u32 = 1;
/// Fallback burst capacity used when
/// [`crate::application::config::RateLimiterConfig::burst_size`] is configured as
/// zero. Preserves the historical default of allowing a small burst.
pub const DEFAULT_RATE_LIMIT_BURST_SIZE: u32 = 10;
/// A constant representing the default sell level for orders.
///
/// This value is set to `0.0` by default and can be used to indicate an initial or
/// baseline sell level for order-related computations or configurations.
pub const DEFAULT_ORDER_SELL_LEVEL: f64 = 0.0;
/// A constant representing the default buy level for orders.
///
/// This value is set as a `f64` and determines the default threshold for the buy level in an order system.
/// Developers can use this constant to ensure uniformity and consistency when working with order buy levels
/// across the application.
pub const DEFAULT_ORDER_BUY_LEVEL: f64 = 10000.0;

/// Sentinel value used for `account_id` when `IG_ACCOUNT_ID` is not configured.
///
/// This is not a real IG account id: it signals "no account was explicitly
/// configured, use whatever account the session lands on". Auth flows must not
/// attempt to switch to this value.
pub const DEFAULT_ACCOUNT_ID: &str = "default_account_id";
