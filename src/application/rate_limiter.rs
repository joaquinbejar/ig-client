/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 19/10/25
******************************************************************************/

//! Rate limiter module for controlling API request rates
//!
//! This module provides rate limiting functionality using the `governor` crate
//! to ensure compliance with IG Markets API rate limits.
//!
//! # Enforced budget
//!
//! `RateLimiter` holds one `governor` token bucket per
//! `RateLimitClass`, all derived from the single
//! `RateLimiterConfig` passed to
//! `RateLimiter::new`:
//!
//! - `NonTrading` honors the configured budget
//!   exactly: `max_requests` requests per `period_seconds`, with `burst_size`
//!   burst capacity. The per-cell replenishment interval is `period_seconds /
//!   max_requests`.
//! - `Trading` uses a stricter, fixed budget derived
//!   from IG's published per-app trading limit (~1 request/second), independent
//!   of the configured budget so a permissive config cannot loosen it.
//! - `Historical` uses its own conservative bucket
//!   because historical price fetches also draw down a weekly data-point
//!   allowance.
//!
//! The buckets are independent, so bulk non-trading traffic can never queue
//! order placement behind it.

use crate::application::config::RateLimiterConfig;
use crate::constants::{
    DEFAULT_RATE_LIMIT_BURST_SIZE, FALLBACK_RATE_LIMIT_MAX_REQUESTS,
    HISTORICAL_RATE_LIMIT_PER_SECOND, TRADING_HISTORICAL_BURST_SIZE, TRADING_RATE_LIMIT_PER_SECOND,
};
use governor::{
    Quota, RateLimiter as GovernorRateLimiter,
    clock::QuantaClock,
    state::{InMemoryState, NotKeyed},
};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

/// Concrete `governor` direct rate limiter used for every class.
type DirectLimiter = GovernorRateLimiter<NotKeyed, InMemoryState, QuantaClock>;

/// Number of nanoseconds in one second, used when deriving per-second budgets.
const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// Rate-limit class for an IG endpoint.
///
/// IG applies separate budgets to trading and non-trading traffic, and
/// historical price fetches additionally draw down a weekly data-point
/// allowance. Each class maps to an independent token bucket inside
/// `RateLimiter` so one kind of traffic never starves another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RateLimitClass {
    /// Order and position mutations (`positions/otc`, `workingorders/otc`).
    ///
    /// Subject to IG's strict per-app trading limit; the most tightly paced
    /// class.
    Trading,
    /// Historical price fetches (endpoints under `prices/`).
    ///
    /// Subject to a weekly data-point allowance in addition to a per-second
    /// rate, so it gets its own conservative bucket.
    Historical,
    /// Everything else: market data, account queries, sentiment, watchlists,
    /// working-order and position *reads*, and so on.
    ///
    /// Paced by the configured `RateLimiterConfig` budget.
    NonTrading,
}

/// Rate limiter for controlling API request rates
///
/// Uses the `governor` crate to implement a token bucket algorithm for rate
/// limiting API requests, with one independent bucket per `RateLimitClass`.
/// See the [module documentation](self) for the enforced budget of each class.
#[derive(Clone)]
pub struct RateLimiter {
    /// Bucket for `RateLimitClass::NonTrading` — the configured budget.
    non_trading: Arc<DirectLimiter>,
    /// Bucket for `RateLimitClass::Trading` — derived strict trading budget.
    trading: Arc<DirectLimiter>,
    /// Bucket for `RateLimitClass::Historical` — derived conservative budget.
    historical: Arc<DirectLimiter>,
}

/// Computes the per-cell replenishment interval that honors the configured
/// budget: `period_seconds / max_requests`.
///
/// This is the core fix for issue #48. The previous implementation used the
/// whole `period_seconds` as the interval and ignored `max_requests` entirely,
/// so a `{ max_requests: 60, period_seconds: 60 }` config allowed ~1
/// request/minute instead of 60.
///
/// Guards the structurally-invalid zero cases without dividing by zero or
/// building a zero-length period:
/// - `max_requests == 0` falls back to [`FALLBACK_RATE_LIMIT_MAX_REQUESTS`]
///   (one request per period).
/// - `period_seconds == 0` falls back to a one-second period.
///
/// Rounding is toward zero (integer nanosecond division); the result is clamped
/// to at least one nanosecond so the interval is always non-zero.
#[must_use]
#[inline]
fn replenish_period(config: &RateLimiterConfig) -> Duration {
    let max_requests = config.max_requests.max(FALLBACK_RATE_LIMIT_MAX_REQUESTS);
    let period = if config.period_seconds == 0 {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(config.period_seconds)
    };
    // `as_nanos` is u128; clamp to u64 for the very large (multi-century) periods
    // that cannot occur in practice but must not panic.
    let period_nanos = u64::try_from(period.as_nanos()).unwrap_or(u64::MAX);
    // `max_requests >= 1` here, so this division can never divide by zero.
    let per_cell_nanos = (period_nanos / u64::from(max_requests)).max(1);
    Duration::from_nanos(per_cell_nanos)
}

/// Per-cell replenishment interval for a fixed "requests per second" budget.
///
/// Used to derive the trading and historical buckets. Guards `requests_per_second
/// == 0` by treating it as one, and clamps the result to at least one nanosecond.
#[must_use]
#[inline]
fn per_second_period(requests_per_second: u32) -> Duration {
    let rps = u64::from(requests_per_second.max(1));
    Duration::from_nanos((NANOS_PER_SECOND / rps).max(1))
}

/// Builds a direct `governor` limiter for a per-cell interval and burst size.
///
/// Guards both invalid inputs without panicking:
/// - a zero `burst_size` falls back to [`DEFAULT_RATE_LIMIT_BURST_SIZE`];
/// - a zero-length `period_per_cell` (structurally unreachable given the callers)
///   falls back to a one-request-per-second quota via [`Quota::per_second`].
#[must_use]
fn build_limiter(period_per_cell: Duration, burst_size: u32) -> Arc<DirectLimiter> {
    let burst = NonZeroU32::new(burst_size)
        .or_else(|| NonZeroU32::new(DEFAULT_RATE_LIMIT_BURST_SIZE))
        .unwrap_or(NonZeroU32::MIN);

    let quota = match Quota::with_period(period_per_cell) {
        Some(quota) => quota.allow_burst(burst),
        // Unreachable in practice: `replenish_period` / `per_second_period`
        // always yield a non-zero interval. Fall back to a safe quota instead of
        // panicking so the limiter stays functional.
        None => Quota::per_second(NonZeroU32::MIN).allow_burst(burst),
    };

    Arc::new(GovernorRateLimiter::direct(quota))
}

impl RateLimiter {
    /// Creates a new rate limiter from configuration
    ///
    /// The non-trading bucket honors the configured budget exactly: it admits
    /// `config.max_requests` requests per `config.period_seconds`, with
    /// `config.burst_size` burst capacity (replenishing one cell every
    /// `period_seconds / max_requests`). The trading and historical buckets use
    /// stricter, fixed budgets derived from IG's published limits and are
    /// independent of the configured budget. See the [module documentation](self).
    ///
    /// # Arguments
    ///
    /// * `config` - Rate limiter configuration containing max requests, period, and burst size
    ///
    /// # Returns
    ///
    /// A new `RateLimiter` instance
    ///
    /// # Example
    ///
    /// ```ignore
    /// use ig_client::application::config::RateLimiterConfig;
    /// use ig_client::application::rate_limiter::RateLimiter;
    ///
    /// let config = RateLimiterConfig {
    ///     max_requests: 60,
    ///     period_seconds: 60,
    ///     burst_size: 10,
    /// };
    ///
    /// // Non-trading admits 60 requests/minute (one cell per second, +burst).
    /// let limiter = RateLimiter::new(&config);
    /// ```
    #[must_use]
    pub fn new(config: &RateLimiterConfig) -> Self {
        let non_trading = build_limiter(replenish_period(config), config.burst_size);
        let trading = build_limiter(
            per_second_period(TRADING_RATE_LIMIT_PER_SECOND),
            TRADING_HISTORICAL_BURST_SIZE,
        );
        let historical = build_limiter(
            per_second_period(HISTORICAL_RATE_LIMIT_PER_SECOND),
            TRADING_HISTORICAL_BURST_SIZE,
        );

        Self {
            non_trading,
            trading,
            historical,
        }
    }

    /// Returns the underlying limiter for a given rate-limit class.
    #[inline]
    fn limiter_for(&self, class: RateLimitClass) -> &DirectLimiter {
        match class {
            RateLimitClass::Trading => &self.trading,
            RateLimitClass::Historical => &self.historical,
            RateLimitClass::NonTrading => &self.non_trading,
        }
    }

    /// Waits until a request in the given class can be made according to its
    /// rate limit.
    ///
    /// Uses `governor`'s async scheduler ([`until_ready`](GovernorRateLimiter::until_ready)):
    /// the future is parked until a slot is available rather than busy-polling.
    /// Each class has an independent bucket, so waiting on one class never blocks
    /// another.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use ig_client::application::rate_limiter::RateLimitClass;
    /// limiter.wait_for(RateLimitClass::Trading).await;
    /// // Place order here
    /// ```
    pub async fn wait_for(&self, class: RateLimitClass) {
        self.limiter_for(class).until_ready().await;
    }

    /// Checks if a request in the given class can be made immediately without
    /// waiting.
    ///
    /// # Returns
    ///
    /// * `true` if a request can be made immediately
    /// * `false` if that class's rate limit has been reached
    #[must_use]
    pub fn check_for(&self, class: RateLimitClass) -> bool {
        self.limiter_for(class).check().is_ok()
    }

    /// Waits until a non-trading request can be made according to the rate limit
    ///
    /// Convenience wrapper over [`wait_for`](Self::wait_for) with
    /// `RateLimitClass::NonTrading`. Blocks (asynchronously) until the rate
    /// limiter allows the request to proceed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// limiter.wait().await;
    /// // Make API request here
    /// ```
    pub async fn wait(&self) {
        self.wait_for(RateLimitClass::NonTrading).await;
    }

    /// Checks if a non-trading request can be made immediately without waiting
    ///
    /// Convenience wrapper over [`check_for`](Self::check_for) with
    /// `RateLimitClass::NonTrading`.
    ///
    /// # Returns
    ///
    /// * `true` if a request can be made immediately
    /// * `false` if the rate limit has been reached
    ///
    /// # Example
    ///
    /// ```ignore
    /// if limiter.check() {
    ///     // Make API request
    /// } else {
    ///     // Wait or handle rate limit
    /// }
    /// ```
    #[must_use]
    pub fn check(&self) -> bool {
        self.check_for(RateLimitClass::NonTrading)
    }
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("non_trading", &"GovernorRateLimiter")
            .field("trading", &"GovernorRateLimiter")
            .field("historical", &"GovernorRateLimiter")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replenish_period_honors_max_requests() {
        // 60 requests per 60 seconds => one cell every 1 second.
        let config = RateLimiterConfig {
            max_requests: 60,
            period_seconds: 60,
            burst_size: 3,
        };
        assert_eq!(replenish_period(&config), Duration::from_secs(1));
        // The OLD bug used the whole 60s period as the interval (~1 req/minute);
        // the interval must be P/N, not P.
        assert_ne!(replenish_period(&config), Duration::from_secs(60));

        // 4 requests per 12 seconds => one cell every 3 seconds.
        let config = RateLimiterConfig {
            max_requests: 4,
            period_seconds: 12,
            burst_size: 3,
        };
        assert_eq!(replenish_period(&config), Duration::from_secs(3));
    }

    #[test]
    fn test_replenish_period_zero_max_requests_falls_back() {
        // max_requests == 0 is structurally invalid: fall back to one request per
        // period (10s per cell) rather than dividing by zero.
        let config = RateLimiterConfig {
            max_requests: 0,
            period_seconds: 10,
            burst_size: 1,
        };
        assert_eq!(replenish_period(&config), Duration::from_secs(10));
    }

    #[test]
    fn test_replenish_period_zero_period_falls_back() {
        // period_seconds == 0 falls back to a one-second period: 5 req/s => 200ms.
        let config = RateLimiterConfig {
            max_requests: 5,
            period_seconds: 0,
            burst_size: 1,
        };
        assert_eq!(replenish_period(&config), Duration::from_millis(200));
    }

    #[test]
    fn test_per_second_period_matches_rate() {
        assert_eq!(per_second_period(1), Duration::from_secs(1));
        assert_eq!(per_second_period(4), Duration::from_millis(250));
        // Zero is treated as one to avoid dividing by zero.
        assert_eq!(per_second_period(0), Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_wait_for_replenishes_at_configured_rate() {
        // 20 requests / 1 second => one cell every 50ms. With burst_size 1 only
        // the initial cell is immediately available; the next must wait ~50ms.
        let config = RateLimiterConfig {
            max_requests: 20,
            period_seconds: 1,
            burst_size: 1,
        };
        let limiter = RateLimiter::new(&config);

        // Consume the single burst cell.
        assert!(limiter.check_for(RateLimitClass::NonTrading));

        let start = std::time::Instant::now();
        limiter.wait_for(RateLimitClass::NonTrading).await;
        let elapsed = start.elapsed();

        // Under the OLD `with_period(period)` bug the interval was the whole 1s
        // period (~1000ms); honoring max_requests makes it ~50ms.
        assert!(
            elapsed >= Duration::from_millis(20),
            "waited {elapsed:?}, expected ~50ms replenishment (rate limiting must apply)"
        );
        assert!(
            elapsed < Duration::from_millis(500),
            "waited {elapsed:?}, the old 1s-per-cell bug is not fixed"
        );
    }

    #[tokio::test]
    async fn test_trading_not_blocked_behind_saturated_non_trading() {
        // Single-cell non-trading bucket that will not replenish for ~60s.
        let config = RateLimiterConfig {
            max_requests: 1,
            period_seconds: 60,
            burst_size: 1,
        };
        let limiter = RateLimiter::new(&config);

        // Saturate the non-trading bucket: first admits, second is denied.
        assert!(limiter.check_for(RateLimitClass::NonTrading));
        assert!(
            !limiter.check_for(RateLimitClass::NonTrading),
            "non-trading bucket should be saturated"
        );

        // Trading and historical have independent buckets and still admit.
        assert!(
            limiter.check_for(RateLimitClass::Trading),
            "trading must not be blocked behind a saturated non-trading bucket"
        );
        assert!(
            limiter.check_for(RateLimitClass::Historical),
            "historical must not be blocked behind a saturated non-trading bucket"
        );
    }

    #[tokio::test]
    async fn test_wait_for_returns_promptly_when_slot_available() {
        // A fresh bucket with burst capacity admits the first request without any
        // wait — proving `wait_for` resolves via the scheduler, not a 10ms poll.
        let config = RateLimiterConfig {
            max_requests: 10,
            period_seconds: 1,
            burst_size: 5,
        };
        let limiter = RateLimiter::new(&config);

        let start = std::time::Instant::now();
        limiter.wait_for(RateLimitClass::NonTrading).await;
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "wait_for should return promptly when a slot is available"
        );
    }

    #[test]
    fn test_check_delegates_to_non_trading() {
        // `check()` (no class) must observe the same bucket as
        // `check_for(NonTrading)` so existing callers keep their semantics.
        let config = RateLimiterConfig {
            max_requests: 1,
            period_seconds: 60,
            burst_size: 1,
        };
        let limiter = RateLimiter::new(&config);

        assert!(limiter.check());
        assert!(
            !limiter.check(),
            "check() must delegate to the non-trading bucket"
        );
        assert!(
            !limiter.check_for(RateLimitClass::NonTrading),
            "check_for(NonTrading) must observe the same saturated bucket as check()"
        );
    }
}
