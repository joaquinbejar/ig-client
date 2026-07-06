//! Offline tests for `make_http_request` — the single choke point that maps HTTP
//! status codes and IG error bodies to typed `AppError` variants and applies the
//! finite retry policy. Driven against a `wiremock::MockServer`; retry delays are
//! zero so the suite stays fast, and mock `.expect(...)` counts assert the exact
//! number of attempts (retries + 1).

use ig_client::application::config::RateLimiterConfig;
use ig_client::application::rate_limiter::RateLimiter;
use ig_client::error::AppError;
use ig_client::model::http::make_http_request;
use ig_client::model::retry::RetryConfig;
use reqwest::{Client, Method, StatusCode};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A permissive rate limiter so pacing never slows the tests. The mock endpoint
/// path is non-trading, so it is governed by this configured budget.
fn permissive_rate_limiter() -> RateLimiter {
    RateLimiter::new(&RateLimiterConfig {
        max_requests: 1000,
        period_seconds: 1,
        burst_size: 100,
    })
}

/// A finite retry config with zero base delay: `max_retries = 2` means up to
/// three attempts total, and a zero delay keeps retries instantaneous.
fn fast_retry() -> RetryConfig {
    RetryConfig {
        max_retry_count: Some(2),
        retry_delay_secs: Some(0),
    }
}

/// `max_retries` for [`fast_retry`]; the total attempt count on a persistently
/// retryable status is `RETRIES + 1`.
const RETRIES: u64 = 2;

/// Issues a GET to `{server}/test-endpoint` through `make_http_request`.
async fn get_test_endpoint(
    server: &MockServer,
    retry: RetryConfig,
) -> Result<reqwest::Response, AppError> {
    let client = Client::new();
    let url = format!("{}/test-endpoint", server.uri());
    make_http_request(
        &client,
        &permissive_rate_limiter(),
        Method::GET,
        &url,
        vec![("X-IG-API-KEY", "FAKE-API-KEY")],
        &None::<()>,
        retry,
    )
    .await
}

/// Mounts a single response for `GET /test-endpoint`, expecting exactly
/// `expected_hits` matching requests (verified on server drop).
async fn mount_get(server: &MockServer, response: ResponseTemplate, expected_hits: u64) {
    Mock::given(method("GET"))
        .and(path("/test-endpoint"))
        .respond_with(response)
        .expect(expected_hits)
        .mount(server)
        .await;
}

#[tokio::test]
async fn test_make_http_request_200_returns_ok() {
    let server = MockServer::start().await;
    mount_get(
        &server,
        ResponseTemplate::new(200).set_body_raw(r#"{"ok":true}"#, "application/json"),
        1,
    )
    .await;

    let response = get_test_endpoint(&server, fast_retry())
        .await
        .expect("2xx should return Ok");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_make_http_request_401_oauth_invalid_maps_to_oauth_token_expired() {
    let server = MockServer::start().await;
    // 401 carrying the oauth-token-invalid marker is surfaced (not retried) so
    // the caller can refresh and replay.
    mount_get(
        &server,
        ResponseTemplate::new(401).set_body_raw(
            r#"{"errorCode":"error.security.oauth-token-invalid"}"#,
            "application/json",
        ),
        1,
    )
    .await;

    match get_test_endpoint(&server, fast_retry()).await {
        Err(AppError::OAuthTokenExpired) => {}
        other => panic!("expected OAuthTokenExpired, got {other:?}"),
    }
}

#[tokio::test]
async fn test_make_http_request_401_without_oauth_marker_maps_to_unauthorized() {
    let server = MockServer::start().await;
    mount_get(
        &server,
        ResponseTemplate::new(401).set_body_raw(
            r#"{"errorCode":"error.security.client-token-invalid"}"#,
            "application/json",
        ),
        1,
    )
    .await;

    match get_test_endpoint(&server, fast_retry()).await {
        Err(AppError::Unauthorized) => {}
        other => panic!("expected Unauthorized, got {other:?}"),
    }
}

#[tokio::test]
async fn test_make_http_request_403_historical_allowance_fails_fast() {
    let server = MockServer::start().await;
    // Historical-data allowance is a weekly quota: retrying is pointless, so this
    // must fail fast (exactly one hit, no retries).
    mount_get(
        &server,
        ResponseTemplate::new(403).set_body_raw(
            r#"{"errorCode":"error.public-api.exceeded-account-historical-data-allowance"}"#,
            "application/json",
        ),
        1,
    )
    .await;

    match get_test_endpoint(&server, fast_retry()).await {
        Err(AppError::HistoricalDataAllowanceExceeded { allowance_expiry }) => {
            assert_eq!(allowance_expiry, 0);
        }
        other => panic!("expected HistoricalDataAllowanceExceeded, got {other:?}"),
    }
}

#[tokio::test]
async fn test_make_http_request_403_api_key_allowance_is_rate_limited_and_retried() {
    let server = MockServer::start().await;
    // An API-key allowance breach is a rate limit: it maps to RateLimitExceeded
    // and IS retried, so all RETRIES + 1 attempts are made before giving up.
    mount_get(
        &server,
        ResponseTemplate::new(403).set_body_raw(
            r#"{"errorCode":"error.public-api.exceeded-api-key-allowance"}"#,
            "application/json",
        ),
        RETRIES + 1,
    )
    .await;

    match get_test_endpoint(&server, fast_retry()).await {
        Err(AppError::RateLimitExceeded) => {}
        other => panic!("expected RateLimitExceeded, got {other:?}"),
    }
}

#[tokio::test]
async fn test_make_http_request_403_other_maps_to_unexpected_and_fails_fast() {
    let server = MockServer::start().await;
    mount_get(
        &server,
        ResponseTemplate::new(403).set_body_raw(
            r#"{"errorCode":"error.public-api.some-other-forbidden"}"#,
            "application/json",
        ),
        1,
    )
    .await;

    match get_test_endpoint(&server, fast_retry()).await {
        Err(AppError::Unexpected(status)) => assert_eq!(status, StatusCode::FORBIDDEN),
        other => panic!("expected Unexpected(403), got {other:?}"),
    }
}

#[tokio::test]
async fn test_make_http_request_429_is_rate_limited_and_retried() {
    let server = MockServer::start().await;
    // 429 is transient: retried with backoff, then terminates as RateLimitExceeded
    // once the finite budget is exhausted.
    mount_get(&server, ResponseTemplate::new(429), RETRIES + 1).await;

    match get_test_endpoint(&server, fast_retry()).await {
        Err(AppError::RateLimitExceeded) => {}
        other => panic!("expected RateLimitExceeded after retries, got {other:?}"),
    }
}

#[tokio::test]
async fn test_make_http_request_500_is_retried_then_unexpected() {
    let server = MockServer::start().await;
    // 5xx is transient: retried, then the terminal typed error carries the status.
    mount_get(&server, ResponseTemplate::new(500), RETRIES + 1).await;

    match get_test_endpoint(&server, fast_retry()).await {
        Err(AppError::Unexpected(status)) => {
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        }
        other => panic!("expected Unexpected(500) after retries, got {other:?}"),
    }
}

#[tokio::test]
async fn test_make_http_request_zero_retries_makes_single_attempt() {
    let server = MockServer::start().await;
    // With max_retries = 0 a persistently retryable status is attempted exactly
    // once (try, never retry) before returning the terminal error.
    mount_get(&server, ResponseTemplate::new(503), 1).await;

    let retry = RetryConfig {
        max_retry_count: Some(0),
        retry_delay_secs: Some(0),
    };
    match get_test_endpoint(&server, retry).await {
        Err(AppError::Unexpected(status)) => {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        }
        other => panic!("expected Unexpected(503) on single attempt, got {other:?}"),
    }
}
