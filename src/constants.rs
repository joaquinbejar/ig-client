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
