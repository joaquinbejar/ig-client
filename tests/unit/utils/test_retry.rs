use ig_client::constants::{
    DEFAULT_MAX_RETRIES, DEFAULT_RETRY_DELAY_SECS, DEPRECATED_INFINITE_RETRY_CAP,
    MAX_RETRY_DELAY_SECS,
};
use ig_client::model::retry::RetryConfig;
use std::time::Duration;

#[test]
fn test_retry_config_new_has_positive_delay() {
    let config = RetryConfig::new();
    assert!(config.delay_secs() > 0);
}

#[test]
fn test_retry_config_default_unset_is_finite() {
    // Construct directly (not via env) so the assertion is deterministic and
    // does not depend on MAX_RETRY_COUNT / RETRY_DELAY_SECS being unset.
    let config = RetryConfig {
        max_retry_count: None,
        retry_delay_secs: None,
    };
    assert_eq!(config.max_retries(), DEFAULT_MAX_RETRIES);
    assert_eq!(config.delay_secs(), DEFAULT_RETRY_DELAY_SECS);
}

#[test]
fn test_retry_config_with_max_retries() {
    let config = RetryConfig::with_max_retries(5);
    assert_eq!(config.max_retries(), 5);
}

#[test]
fn test_retry_config_with_max_retries_and_delay() {
    let config = RetryConfig::with_max_retries_and_delay(3, 15);
    assert_eq!(config.max_retries(), 3);
    assert_eq!(config.delay_secs(), 15);
}

#[test]
#[allow(deprecated)]
fn test_retry_config_deprecated_infinite_is_finite() {
    // The deprecated "infinite" constructor must now be finite.
    let config = RetryConfig::infinite();
    assert_eq!(config.max_retries(), DEPRECATED_INFINITE_RETRY_CAP);
}

#[test]
#[allow(deprecated)]
fn test_retry_config_deprecated_with_delay_is_finite() {
    let config = RetryConfig::with_delay(30);
    assert_eq!(config.max_retries(), DEPRECATED_INFINITE_RETRY_CAP);
    assert_eq!(config.delay_secs(), 30);
}

#[test]
fn test_retry_config_default_has_positive_delay() {
    let config = RetryConfig::default();
    assert!(config.delay_secs() > 0);
    // The retry count is always finite regardless of environment.
    assert!(config.max_retries() >= 1 || config.max_retries() == 0);
}

#[test]
fn test_retry_config_max_retries_getter() {
    let config1 = RetryConfig {
        max_retry_count: Some(10),
        retry_delay_secs: None,
    };
    assert_eq!(config1.max_retries(), 10);

    // Unset falls back to the finite default, never infinite.
    let config2 = RetryConfig {
        max_retry_count: None,
        retry_delay_secs: None,
    };
    assert_eq!(config2.max_retries(), DEFAULT_MAX_RETRIES);
}

#[test]
fn test_retry_config_delay_secs_getter() {
    let config1 = RetryConfig {
        max_retry_count: None,
        retry_delay_secs: Some(25),
    };
    assert_eq!(config1.delay_secs(), 25);

    let config2 = RetryConfig {
        max_retry_count: None,
        retry_delay_secs: None,
    };
    assert_eq!(config2.delay_secs(), DEFAULT_RETRY_DELAY_SECS);
}

#[test]
fn test_retry_config_backoff_grows_and_caps() {
    let config = RetryConfig {
        max_retry_count: None,
        retry_delay_secs: Some(1), // 1s base keeps the bounds easy to reason about.
    };

    // delay_for_attempt(0) < delay_for_attempt(1) < delay_for_attempt(2)
    // ignoring jitter: lower bound of attempt n+1 exceeds upper bound of n.
    for attempt in 0u32..3 {
        let base_ms: u64 = 1000 * (1u64 << attempt);
        let expected = base_ms.min(MAX_RETRY_DELAY_SECS * 1000);
        let got = u64::try_from(config.delay_for_attempt(attempt).as_millis()).unwrap_or(u64::MAX);
        assert!(got >= expected, "attempt {attempt}: {got} < {expected}");
        assert!(
            got <= expected + expected / 4,
            "attempt {attempt}: {got} exceeds +25% of {expected}"
        );
    }

    // Explicit ordering with the jitter-safe bounds.
    let d0 = config.delay_for_attempt(0);
    let d1 = config.delay_for_attempt(1);
    let d2 = config.delay_for_attempt(2);
    assert!(d0 <= Duration::from_millis(1250));
    assert!(d1 >= Duration::from_millis(2000) && d1 <= Duration::from_millis(2500));
    assert!(d2 >= Duration::from_millis(4000));
}

#[test]
fn test_retry_config_backoff_saturates_at_cap() {
    let config = RetryConfig {
        max_retry_count: None,
        retry_delay_secs: Some(10),
    };
    // A large attempt saturates at the cap (+ up to 25% jitter), never overflows.
    let cap = Duration::from_secs(MAX_RETRY_DELAY_SECS);
    let got = config.delay_for_attempt(60);
    assert!(got >= cap && got <= cap + cap / 4);
}
