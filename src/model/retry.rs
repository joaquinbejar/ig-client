/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/10/25
******************************************************************************/
use crate::constants::{
    DEFAULT_MAX_RETRIES, DEFAULT_RETRY_DELAY_SECS, DEPRECATED_INFINITE_RETRY_CAP,
    MAX_RETRY_DELAY_SECS,
};
use crate::utils::config::get_env_or_none;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Configuration for HTTP request retry behavior.
///
/// Retry is always finite: when a field is unset the finite defaults
/// [`DEFAULT_MAX_RETRIES`] and [`DEFAULT_RETRY_DELAY_SECS`] apply. Unbounded
/// retry was removed in PR #26 and must not be reintroduced.
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    /// Maximum number of retries on transient failures.
    ///
    /// `None` means "use [`DEFAULT_MAX_RETRIES`]" — it is never interpreted as
    /// infinite retries.
    pub max_retry_count: Option<u32>,
    /// Base delay in seconds between retries.
    ///
    /// `None` means "use [`DEFAULT_RETRY_DELAY_SECS`]". This value is the base
    /// for exponential backoff, not a fixed per-attempt delay.
    pub retry_delay_secs: Option<u64>,
}

impl RetryConfig {
    /// Creates a new retry configuration with the finite defaults.
    ///
    /// Equivalent to [`RetryConfig::default`]: reads `MAX_RETRY_COUNT` /
    /// `RETRY_DELAY_SECS` from the environment when set, otherwise uses
    /// [`DEFAULT_MAX_RETRIES`] and [`DEFAULT_RETRY_DELAY_SECS`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Deprecated: unbounded retry is banned (PR #26).
    ///
    /// Previously this returned a config with infinite retries. It now clamps to
    /// [`DEPRECATED_INFINITE_RETRY_CAP`] — a large but finite cap — so callers
    /// keep compiling while never looping forever. Use [`RetryConfig::default`]
    /// or [`RetryConfig::with_max_retries`] instead.
    ///
    /// Uses `RETRY_DELAY_SECS` when set, otherwise [`DEFAULT_RETRY_DELAY_SECS`].
    #[must_use]
    #[deprecated(note = "unbounded retry is banned; use a finite RetryConfig")]
    pub fn infinite() -> Self {
        Self {
            max_retry_count: Some(DEPRECATED_INFINITE_RETRY_CAP),
            retry_delay_secs: get_env_or_none("RETRY_DELAY_SECS"),
        }
    }

    /// Creates a new retry configuration with a maximum number of retries.
    ///
    /// Uses `RETRY_DELAY_SECS` when set, otherwise [`DEFAULT_RETRY_DELAY_SECS`].
    #[must_use]
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self {
            max_retry_count: Some(max_retries),
            retry_delay_secs: get_env_or_none("RETRY_DELAY_SECS"),
        }
    }

    /// Deprecated: this constructor left the retry count unbounded.
    ///
    /// It now clamps the retry count to [`DEPRECATED_INFINITE_RETRY_CAP`] — a
    /// large but finite cap — while still applying the custom base delay. Use
    /// [`RetryConfig::with_max_retries_and_delay`] for an explicit finite cap.
    #[must_use]
    #[deprecated(note = "unbounded retry is banned; use a finite RetryConfig")]
    pub fn with_delay(delay_secs: u64) -> Self {
        Self {
            max_retry_count: Some(DEPRECATED_INFINITE_RETRY_CAP),
            retry_delay_secs: Some(delay_secs),
        }
    }

    /// Creates a new retry configuration with both max retries and custom delay.
    #[must_use]
    pub fn with_max_retries_and_delay(max_retries: u32, delay_secs: u64) -> Self {
        Self {
            max_retry_count: Some(max_retries),
            retry_delay_secs: Some(delay_secs),
        }
    }

    /// Gets the maximum retry count.
    ///
    /// Always finite: returns [`DEFAULT_MAX_RETRIES`] when unconfigured. A value
    /// of `0` means "try once, never retry"; it is never treated as infinite.
    #[must_use]
    pub fn max_retries(&self) -> u32 {
        self.max_retry_count.unwrap_or(DEFAULT_MAX_RETRIES)
    }

    /// Gets the base retry delay in seconds.
    ///
    /// Returns [`DEFAULT_RETRY_DELAY_SECS`] when unconfigured.
    #[must_use]
    pub fn delay_secs(&self) -> u64 {
        self.retry_delay_secs.unwrap_or(DEFAULT_RETRY_DELAY_SECS)
    }

    /// Computes the backoff delay before the given 0-based retry `attempt`.
    ///
    /// Exponential: `base * 2^attempt` seconds, where `base` is
    /// [`RetryConfig::delay_secs`], saturating at [`MAX_RETRY_DELAY_SECS`]. The
    /// cap is the policy, so rounding down to it is intentional. A jitter of
    /// `[0, 25%]` of the delay is added to avoid thundering-herd retries; the
    /// jitter entropy comes from the current wall-clock subsecond nanoseconds
    /// (this is jitter, not cryptographic randomness).
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        backoff_delay(Duration::from_secs(self.delay_secs()), attempt)
    }
}

/// Computes an exponential backoff delay with jitter for a given `base` delay
/// and 0-based `attempt`.
///
/// The delay is `base * 2^attempt`, saturating at [`MAX_RETRY_DELAY_SECS`]
/// (the cap is the policy). Checked arithmetic is used on the exponent so a
/// large `attempt` cannot overflow; it simply saturates at the cap. A jitter of
/// `[0, 25%]` of the delay is added, derived from wall-clock subsecond
/// nanoseconds — this is jitter, not cryptographic randomness.
#[must_use]
pub(crate) fn backoff_delay(base: Duration, attempt: u32) -> Duration {
    let cap = Duration::from_secs(MAX_RETRY_DELAY_SECS);

    // base * 2^attempt. `checked_shl` guards the exponent (a shift >= 64 yields
    // the max factor); the multiply then saturates and we clamp to `cap_ms`.
    // Saturating to the cap here is intentional — the cap is the policy.
    let base_ms = u64::try_from(base.as_millis()).unwrap_or(u64::MAX);
    let cap_ms = u64::try_from(cap.as_millis()).unwrap_or(u64::MAX);

    let factor = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
    let scaled_ms = base_ms.saturating_mul(factor).min(cap_ms);

    // Jitter in [0, 25%] of the (capped) delay. Entropy from wall-clock
    // subsecond nanoseconds — documented as jitter, not crypto.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::from(d.subsec_nanos()));
    let max_jitter = scaled_ms / 4;
    let jitter_ms = if max_jitter == 0 {
        0
    } else {
        nanos % (max_jitter + 1)
    };

    Duration::from_millis(scaled_ms.saturating_add(jitter_ms))
}

impl Default for RetryConfig {
    fn default() -> Self {
        let max_retry_count: Option<u32> = get_env_or_none("MAX_RETRY_COUNT");
        let retry_delay_secs: Option<u64> = get_env_or_none("RETRY_DELAY_SECS");

        Self {
            max_retry_count,
            retry_delay_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RetryConfig;
    use crate::constants::{
        DEFAULT_MAX_RETRIES, DEFAULT_RETRY_DELAY_SECS, DEPRECATED_INFINITE_RETRY_CAP,
        MAX_RETRY_DELAY_SECS,
    };
    use std::time::Duration;

    #[test]
    fn test_retry_config_default_is_finite() {
        // Constructed directly (not via env) so the assertion is deterministic.
        let config = RetryConfig {
            max_retry_count: None,
            retry_delay_secs: None,
        };
        assert_eq!(config.max_retries(), DEFAULT_MAX_RETRIES);
        assert_eq!(config.delay_secs(), DEFAULT_RETRY_DELAY_SECS);
    }

    #[test]
    fn test_retry_config_explicit_zero_is_not_infinite() {
        let config = RetryConfig {
            max_retry_count: Some(0),
            retry_delay_secs: None,
        };
        // Zero means "try once, never retry" — distinct from the unset default.
        assert_eq!(config.max_retries(), 0);
    }

    #[test]
    fn test_delay_for_attempt_grows_and_caps() {
        let config = RetryConfig {
            max_retry_count: None,
            retry_delay_secs: Some(1), // 1s base keeps the math easy to bound.
        };

        // Each attempt's delay must sit within [base*2^n, base*2^n * 1.25].
        for attempt in 0u32..4 {
            let base_ms: u64 = 1000 * (1u64 << attempt);
            let expected = base_ms.min(MAX_RETRY_DELAY_SECS * 1000);
            let got =
                u64::try_from(config.delay_for_attempt(attempt).as_millis()).unwrap_or(u64::MAX);
            assert!(
                got >= expected,
                "attempt {attempt}: {got} < lower bound {expected}"
            );
            let upper = expected + expected / 4;
            assert!(
                got <= upper,
                "attempt {attempt}: {got} > upper bound {upper}"
            );
        }

        // Monotonic growth (ignoring jitter) until the cap: compare the
        // exponential component only via the lower bounds already asserted.
        let d0 = config.delay_for_attempt(0);
        let d1 = config.delay_for_attempt(1);
        let d2 = config.delay_for_attempt(2);
        // Lower bound of d1 (2000ms) exceeds upper bound of d0 (1250ms), etc.
        assert!(d1 >= Duration::from_millis(2000) && d0 <= Duration::from_millis(1250));
        assert!(d2 >= Duration::from_millis(4000) && d1 <= Duration::from_millis(2500));
    }

    #[test]
    fn test_delay_for_attempt_saturates_at_cap() {
        let config = RetryConfig {
            max_retry_count: None,
            retry_delay_secs: Some(10),
        };
        // A very large attempt must saturate at the cap (+ up to 25% jitter),
        // never overflow.
        let got = config.delay_for_attempt(60);
        let cap = Duration::from_secs(MAX_RETRY_DELAY_SECS);
        assert!(got >= cap);
        assert!(got <= cap + cap / 4);
    }

    #[test]
    #[allow(deprecated)]
    fn test_deprecated_infinite_is_finite() {
        assert_eq!(
            RetryConfig::infinite().max_retries(),
            DEPRECATED_INFINITE_RETRY_CAP
        );
        assert_eq!(
            RetryConfig::with_delay(5).max_retries(),
            DEPRECATED_INFINITE_RETRY_CAP
        );
        assert_eq!(RetryConfig::with_delay(5).delay_secs(), 5);
    }
}
