//! Acceptance tests for the API key pool.
//!
//! The pool exists because IG meters its non-trading allowance per API key. The
//! properties asserted here are the ones that make that worth having: work is
//! spread *before* a key is rejected, one sent request costs exactly one token,
//! and the errors that do not belong to a key never burn another one.
//!
//! Driven against a `wiremock::MockServer`, which records the `X-IG-API-KEY` of
//! every request and so shows which key served what.

use ig_client::application::config::{
    Config, Credentials, DatabaseConfig, RateLimiterConfig, RestApiConfig, WebSocketConfig,
};
use ig_client::application::http::HttpClient;
use ig_client::application::rate_limiter::{RateLimitClass, RateLimiter};
use ig_client::error::AppError;
use ig_client::model::retry::RetryConfig;
use serde::Deserialize;
use std::time::{Duration, Instant};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Hands each test its own account id, so the per-account budget does not leak
/// between tests running in the same process.
static ACCOUNT_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Minimal DTO for the mock endpoint; the tests care about the traffic, not the
/// payload.
#[derive(Debug, Deserialize)]
struct Dummy {
    #[allow(dead_code)]
    ok: bool,
}

/// Builds a config whose `api_key` is the comma-separated pool under test.
///
/// Budgets here count the login too: a key's session and its data requests
/// share one limiter, because IG meters both against the same per-key
/// allowance. So "one data request" on a fresh key costs two tokens — one for
/// `/session`, one for the data — and the tests size `max_requests`
/// accordingly.
fn pool_config(base_url: &str, keys: &str, max_requests: u32, period_seconds: u64) -> Config {
    pool_config_burst(base_url, keys, max_requests, period_seconds, max_requests)
}

/// Same as [`pool_config`] with an explicit burst size.
///
/// The burst is the bucket's capacity, so it decides how many tokens can be
/// held at once: with a burst of 1 a key can never have a login and a data
/// request ready together, no matter how large the per-period budget is.
fn pool_config_burst(
    base_url: &str,
    keys: &str,
    max_requests: u32,
    period_seconds: u64,
    burst_size: u32,
) -> Config {
    Config {
        credentials: Credentials {
            username: "fake-user".to_string(),
            password: "fake-pass".to_string(),
            // The account budget is shared per process *and per account*, which
            // is the point of it — but it also means two tests sharing an
            // account id would steal each other's tokens while running in
            // parallel. Each config gets its own account unless a test pins one.
            account_id: format!(
                "TEST-{}",
                ACCOUNT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ),
            api_key: keys.to_string(),
            client_token: None,
            account_token: None,
        },
        rest_api: RestApiConfig {
            base_url: base_url.to_string(),
            timeout: 30,
        },
        websocket: WebSocketConfig {
            url: "wss://example.invalid".to_string(),
            reconnect_interval: 5,
        },
        database: DatabaseConfig {
            url: "postgres://localhost/none".to_string(),
            max_connections: 1,
        },
        rate_limiter: RateLimiterConfig {
            max_requests,
            period_seconds,
            burst_size,
        },
        sleep_hours: 1,
        page_size: 20,
        days_to_look_back: 7,
        api_version: Some(3),
    }
}

/// Mounts a `/session` endpoint that always authenticates.
async fn mount_login_ok(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "clientId": "FAKE-CLIENT-1",
            "accountId": "ABC12",
            "timezoneOffset": 1,
            "lightstreamerEndpoint": "https://example.invalid",
            "oauthToken": {
                "access_token": "FAKE-ACCESS",
                "refresh_token": "FAKE-REFRESH",
                "scope": "profile",
                "token_type": "Bearer",
                "expires_in": "600"
            }
        })))
        .mount(server)
        .await;
}

/// Mounts the data endpoint used by every test, always succeeding.
async fn mount_data_ok(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(server)
        .await;
}

/// The `X-IG-API-KEY` of every `/data` request the server received, in order.
async fn keys_used(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/data")
        .map(|r| {
            r.headers
                .get("X-IG-API-KEY")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

/// Two keys with one token each: both requests go out at once, on different
/// keys. A pool that waited for the first key instead of moving to the second
/// would serialise them.
#[tokio::test]
async fn test_pool_two_keys_first_two_requests_use_distinct_keys_immediately() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;

    // Capacity for exactly a login plus one data request per key, so a second
    // data request on the same key would have to wait for a refill.
    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a,key-b", 2, 60, 2))
        .expect("client builds");

    let started = Instant::now();
    client.get::<Dummy>("/data", Some(1)).await.expect("first");
    client.get::<Dummy>("/data", Some(1)).await.expect("second");
    let elapsed = started.elapsed();

    let used = keys_used(&server).await;
    assert_eq!(used.len(), 2, "both requests were sent");
    assert_ne!(used[0], used[1], "the two requests used different keys");
    // Neither request waited on a *key* refill, which is 60 s here. Four
    // physical requests (two logins, two data) still pass the account gate at
    // one per two seconds, so the floor is around six seconds, not zero.
    assert!(
        elapsed < Duration::from_secs(20),
        "neither request waited on a key refill, took {elapsed:?}"
    );
}

/// The third request has no token anywhere, so it must wait for a refill. With
/// a 60 s period and one token per key, it cannot complete quickly.
#[tokio::test]
async fn test_pool_third_request_waits_for_the_first_refill() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;

    // Login plus exactly one data request per key; nothing left for a third.
    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a,key-b", 2, 60, 2))
        .expect("client builds");

    client.get::<Dummy>("/data", Some(1)).await.expect("first");
    client.get::<Dummy>("/data", Some(1)).await.expect("second");

    // Both buckets are empty now; the third must block rather than be sent.
    let third = tokio::time::timeout(
        Duration::from_millis(300),
        client.get::<Dummy>("/data", Some(1)),
    )
    .await;

    assert!(third.is_err(), "the third request waited for a refill");
    assert_eq!(
        keys_used(&server).await.len(),
        2,
        "no third request reached the server while waiting"
    );
}

/// One sent request costs exactly one token.
///
/// This is the reserve-then-wait bug made visible: selection took a cell with
/// `check()` and the send took another with `until_ready()`, so every request
/// cost two and the effective rate was half the configured one. Counted
/// directly on the bucket rather than by timing, so the assertion is exact.
#[tokio::test]
async fn test_send_with_reservation_consumes_exactly_one_token() {
    let server = MockServer::start().await;
    mount_data_ok(&server).await;

    // Three tokens available at once, none refilling during the test.
    let limiter = RateLimiter::new(&RateLimiterConfig {
        max_requests: 3,
        period_seconds: 600,
        burst_size: 3,
    });

    // Reserve as the pool does, then send saying the token is already held.
    assert!(limiter.try_reserve(RateLimitClass::NonTrading), "token 1");
    let response = ig_client::application::http::make_http_request_reserved(
        &reqwest::Client::new(),
        &limiter,
        reqwest::Method::GET,
        &format!("{}/data", server.uri()),
        vec![],
        &None::<()>,
        RetryConfig {
            max_retry_count: Some(0),
            retry_delay_secs: Some(0),
        },
        true,
    )
    .await;
    assert!(response.is_ok(), "the request was sent");

    // Two of the three tokens must remain: the send spent none of its own.
    assert!(
        limiter.try_reserve(RateLimitClass::NonTrading),
        "second token still available"
    );
    assert!(
        limiter.try_reserve(RateLimitClass::NonTrading),
        "third token still available - the send did not take a second one"
    );
    assert!(
        !limiter.try_reserve(RateLimitClass::NonTrading),
        "and the budget is now genuinely spent"
    );
}

/// A failed login must cost only the login request. If the data token were
/// reserved first, it would be spent on a request that is never sent.
#[tokio::test]
async fn test_pool_failed_login_does_not_consume_a_data_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorCode": "error.security.invalid-details"
        })))
        .mount(&server)
        .await;
    mount_data_ok(&server).await;

    // Capacity for the failed login, the retried login and one data request. If
    // the failed attempt also took a data token, the budget would not cover the
    // request that follows it.
    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a", 3, 60, 3))
        .expect("client builds");

    let failed = client.get::<Dummy>("/data", Some(1)).await;
    assert!(failed.is_err(), "the request fails because login fails");
    assert!(
        keys_used(&server).await.is_empty(),
        "no data request was sent"
    );

    // The token survived the failed login, so a working login can spend it now
    // without waiting for a refill.
    server.reset().await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;

    let started = Instant::now();
    let second = tokio::time::timeout(
        Duration::from_secs(5),
        client.get::<Dummy>("/data", Some(1)),
    )
    .await;
    assert!(
        second.is_ok(),
        "the data token was not spent by the failed login, waited {:?}",
        started.elapsed()
    );
}

/// Concurrent requests spread over the pool instead of stacking on the first
/// key. Without the round-robin cursor every caller starts its scan at index 0.
#[tokio::test]
async fn test_pool_concurrent_requests_spread_across_keys() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;

    let client = std::sync::Arc::new(
        HttpClient::new_lazy(pool_config_burst(
            &server.uri(),
            "key-a,key-b,key-c,key-d",
            2,
            60,
            2,
        ))
        .expect("client builds"),
    );

    let mut tasks = Vec::new();
    for _ in 0..4 {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client
                .get::<Dummy>("/data", Some(1))
                .await
                .map(|_: Dummy| ())
        }));
    }
    for task in tasks {
        let _ = task.await;
    }

    let mut used = keys_used(&server).await;
    used.sort();
    used.dedup();
    assert_eq!(
        used.len(),
        4,
        "four concurrent requests used four distinct keys, got {used:?}"
    );
}

/// When every key is empty the pool waits on all of them and takes whichever
/// refills first, rather than parking on an arbitrary one. Key B refills in 1 s
/// while key A would need 60 s, so the wait must be the short one.
#[tokio::test]
async fn test_rate_limiter_waits_for_the_key_that_refills_first() {
    let slow = RateLimiter::new(&RateLimiterConfig {
        max_requests: 1,
        period_seconds: 60,
        burst_size: 1,
    });
    let fast = RateLimiter::new(&RateLimiterConfig {
        max_requests: 1,
        period_seconds: 1,
        burst_size: 1,
    });

    // Drain both buckets.
    assert!(slow.try_reserve(RateLimitClass::NonTrading));
    assert!(fast.try_reserve(RateLimitClass::NonTrading));

    let started = Instant::now();
    let waits: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>> = vec![
        Box::pin(async move { slow.reserve(RateLimitClass::NonTrading).await }),
        Box::pin(async move { fast.reserve(RateLimitClass::NonTrading).await }),
    ];
    let (_, _, _) = futures::future::select_all(waits).await;

    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the wait ended on the fast bucket, took {:?}",
        started.elapsed()
    );
}

/// A per-key allowance rejection moves to another key straight away, and the
/// request succeeds there.
#[tokio::test]
async fn test_pool_api_key_allowance_rotates_immediately() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;

    // First data call is rejected for the key's allowance; the retry on the
    // other key succeeds.
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorCode": "error.public-api.exceeded-api-key-allowance"
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_data_ok(&server).await;

    let client = HttpClient::new_lazy(pool_config(&server.uri(), "key-a,key-b", 5, 1))
        .expect("client builds");

    let result = client.get::<Dummy>("/data", Some(1)).await;
    assert!(result.is_ok(), "the request succeeded on another key");

    let used = keys_used(&server).await;
    assert_eq!(used.len(), 2, "one rejection plus one success");
    assert_ne!(used[0], used[1], "the retry went to a different key");
}

/// The account-wide allowance belongs to the account every key authenticates,
/// so it must surface as its own error without spending a second key.
#[tokio::test]
async fn test_pool_account_allowance_does_not_rotate() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorCode": "error.public-api.exceeded-account-allowance"
        })))
        .mount(&server)
        .await;

    let client = HttpClient::new_lazy(pool_config(&server.uri(), "key-a,key-b", 5, 1))
        .expect("client builds");

    let err = client
        .get::<Dummy>("/data", Some(1))
        .await
        .expect_err("account allowance is an error");
    assert!(
        matches!(err, AppError::AccountAllowanceExceeded),
        "got {err:?}"
    );

    let used = keys_used(&server).await;
    assert!(
        used.iter().collect::<std::collections::HashSet<_>>().len() <= 1,
        "the account allowance did not burn a second key, used {used:?}"
    );
}

/// Trading traffic stays pinned to one key: the trading allowance is metered
/// against the account, so rotating buys nothing and would scatter order
/// traffic over sessions.
#[tokio::test]
async fn test_pool_trading_allowance_does_not_rotate() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    Mock::given(method("POST"))
        .and(path("/positions/otc"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorCode": "error.public-api.exceeded-account-trading-allowance"
        })))
        .mount(&server)
        .await;

    let client = HttpClient::new_lazy(pool_config(&server.uri(), "key-a,key-b", 5, 1))
        .expect("client builds");

    let err = client
        .post::<_, Dummy>("/positions/otc", serde_json::json!({}), Some(2))
        .await
        .expect_err("trading allowance is an error");
    assert!(
        matches!(err, AppError::TradingAllowanceExceeded),
        "got {err:?}"
    );

    let orders: Vec<String> = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/positions/otc")
        .map(|r| {
            r.headers
                .get("X-IG-API-KEY")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert_eq!(orders.len(), 1, "the order was sent once, got {orders:?}");
}

/// A single key behaves exactly as before the pool existed: requests go out on
/// that key, paced by its budget.
#[tokio::test]
async fn test_pool_single_key_keeps_previous_behaviour() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;

    let client =
        HttpClient::new_lazy(pool_config(&server.uri(), "only-key", 5, 1)).expect("client builds");

    client.get::<Dummy>("/data", Some(1)).await.expect("first");
    client.get::<Dummy>("/data", Some(1)).await.expect("second");

    let used = keys_used(&server).await;
    assert_eq!(used, vec!["only-key".to_string(), "only-key".to_string()]);
}

/// A per-key allowance rejection *during login* rotates at once, instead of
/// spending three backoffs on a key that has already said it is empty.
#[tokio::test]
async fn test_pool_key_allowance_during_login_rotates_without_backoff() {
    let server = MockServer::start().await;

    // The first login attempt is refused for the key's allowance; the next one
    // succeeds. If the client retried the same key, this test would take the
    // backoff (tens of seconds) instead of finishing immediately.
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorCode": "error.public-api.exceeded-api-key-allowance"
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;

    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a,key-b", 4, 60, 4))
        .expect("client builds");

    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(8),
        client.get::<Dummy>("/data", Some(1)),
    )
    .await;

    assert!(
        matches!(result, Ok(Ok(_))),
        "the request succeeded on another key, got {result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "no backoff was paid on the refused key, took {:?}",
        started.elapsed()
    );

    let used = keys_used(&server).await;
    assert_eq!(used.len(), 1, "exactly one data request was sent");
}

/// Two consecutive trading calls use the same slot. Order traffic is metered
/// against the account, so spreading it buys nothing and would scatter a
/// position's requests over sessions a reconciliation then has to match up.
#[tokio::test]
async fn test_pool_consecutive_trading_requests_use_the_same_slot() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    Mock::given(method("POST"))
        .and(path("/positions/otc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    let client = HttpClient::new_lazy(pool_config_burst(
        &server.uri(),
        "key-a,key-b,key-c",
        20,
        1,
        20,
    ))
    .expect("client builds");

    for _ in 0..2 {
        client
            .post::<_, Dummy>("/positions/otc", serde_json::json!({}), Some(2))
            .await
            .expect("order accepted");
    }

    let orders: Vec<String> = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/positions/otc")
        .map(|r| {
            r.headers
                .get("X-IG-API-KEY")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_string()
        })
        .collect();

    assert_eq!(orders.len(), 2, "both orders were sent");
    assert_eq!(
        orders[0], orders[1],
        "both used the same key, got {orders:?}"
    );
}

/// Serving one request must not authenticate the whole pool. With a burst of 1
/// a key holds at most one token, so a selection that probes every key with
/// `get_session` would log every one of them in.
#[tokio::test]
async fn test_pool_first_request_does_not_authenticate_every_key() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;

    let client = HttpClient::new_lazy(pool_config_burst(
        &server.uri(),
        "key-a,key-b,key-c,key-d,key-e",
        10,
        1,
        1,
    ))
    .expect("client builds");

    client.get::<Dummy>("/data", Some(1)).await.expect("first");

    let logins = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/session")
        .count();

    // One login for the key that serves it. A second is possible when that key
    // turns out to have no token and the wait is won by another key, but the
    // pool must never authenticate all five to serve one request.
    assert!(
        logins <= 2,
        "one request did not authenticate the pool, got {logins} logins"
    );
}

/// When IG refuses the last key too, that key is marked as well: the pool must
/// not hand the next request straight back to a key that has just said it is
/// empty.
#[tokio::test]
async fn test_pool_last_key_is_also_marked_on_allowance() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorCode": "error.public-api.exceeded-api-key-allowance"
        })))
        .mount(&server)
        .await;

    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a,key-b", 20, 1, 20))
        .expect("client builds");

    let err = client
        .get::<Dummy>("/data", Some(1))
        .await
        .expect_err("every key refused");
    assert!(
        matches!(err, AppError::ApiKeyAllowanceExceeded),
        "got {err:?}"
    );

    let after_first = keys_used(&server).await.len();
    assert_eq!(after_first, 2, "both keys were tried once");

    // Both are now in cooldown, so a second call must not send anything more
    // while it waits for one of them to come back.
    let second = tokio::time::timeout(
        Duration::from_millis(300),
        client.get::<Dummy>("/data", Some(1)),
    )
    .await;
    assert!(second.is_err(), "the second call waited on the cooldown");
    assert_eq!(
        keys_used(&server).await.len(),
        after_first,
        "no request was sent to a key still in cooldown"
    );
}

/// The account allowance is spent for every key at once, so retrying it only
/// burns more of an allowance that is already gone: exactly one request.
#[tokio::test]
async fn test_account_allowance_sends_exactly_one_request() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorCode": "error.public-api.exceeded-account-allowance"
        })))
        .mount(&server)
        .await;

    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a", 20, 1, 20))
        .expect("client builds");

    let err = client
        .get::<Dummy>("/data", Some(1))
        .await
        .expect_err("account allowance is an error");
    assert!(
        matches!(err, AppError::AccountAllowanceExceeded),
        "got {err:?}"
    );
    assert_eq!(
        keys_used(&server).await.len(),
        1,
        "one request, no retries against an exhausted account budget"
    );
}

/// An expired OAuth token on a key other than the first refreshes *that* key's
/// session. Refreshing slot 0 instead would replay the same rejected token.
#[tokio::test]
async fn test_pool_oauth_refresh_targets_the_failing_slot() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;

    // First data call is answered with an invalidated OAuth token, the next one
    // succeeds. Two keys with one data token each force the first call onto one
    // key and make the replay observable.
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorCode": "error.security.oauth-token-invalid"
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_data_ok(&server).await;

    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a,key-b", 20, 1, 20))
        .expect("client builds");

    let result = client.get::<Dummy>("/data", Some(1)).await;
    assert!(result.is_ok(), "the replay succeeded, got {result:?}");

    let used = keys_used(&server).await;
    assert_eq!(used.len(), 2, "the rejected call plus its replay");
    assert_eq!(
        used[0], used[1],
        "the replay stayed on the key whose token was refreshed, got {used:?}"
    );
}

/// Every request the server received, whatever its path.
///
/// The account budget is charged per *physical* request, so counting only
/// `/data` would miss the logins, refreshes and retries it also has to pay for.
async fn all_requests(server: &MockServer) -> usize {
    server.received_requests().await.unwrap_or_default().len()
}

/// The account ceiling caps everything the process sends, logins included.
///
/// Ten keys with a generous per-key budget would otherwise pace ten times what
/// the account allows.
#[tokio::test]
async fn test_account_budget_caps_every_physical_request() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;

    let keys: Vec<String> = (0..10).map(|i| format!("key-{i}")).collect();
    let client = HttpClient::new_lazy(pool_config_burst(
        &server.uri(),
        &keys.join(","),
        100,
        60,
        100,
    ))
    .expect("client builds");

    for _ in 0..40 {
        if tokio::time::timeout(
            Duration::from_millis(50),
            client.get::<Dummy>("/data", Some(1)),
        )
        .await
        .is_err()
        {
            break;
        }
    }

    let total = all_requests(&server).await;
    assert!(
        total <= 30,
        "the account ceiling capped every request, not just the data ones: {total} sent"
    );
    assert!(total > 0, "the pool still served requests");
}

/// Logins draw on the account budget too. Ten keys logging in cannot exceed the
/// ceiling just because each one has its own key budget.
#[tokio::test]
async fn test_account_budget_covers_logins() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;

    let keys: Vec<String> = (0..10).map(|i| format!("key-{i}")).collect();
    // Per-key burst of 1 forces a login on many keys, so most of the traffic
    // here is `/session` rather than `/data`.
    let client = HttpClient::new_lazy(pool_config_burst(
        &server.uri(),
        &keys.join(","),
        100,
        60,
        1,
    ))
    .expect("client builds");

    for _ in 0..40 {
        if tokio::time::timeout(
            Duration::from_millis(50),
            client.get::<Dummy>("/data", Some(1)),
        )
        .await
        .is_err()
        {
            break;
        }
    }

    let total = all_requests(&server).await;
    let logins = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/session")
        .count();

    assert!(logins > 0, "the test exercised the login path");
    assert!(
        total <= 30,
        "logins and data together stayed under the ceiling: {total} sent ({logins} logins)"
    );
}

/// Retries pay the account budget as well: each attempt is a request IG counts.
#[tokio::test]
async fn test_account_budget_covers_retries() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    // A persistently failing endpoint, so the client exhausts its retry budget.
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a", 100, 60, 100))
        .expect("client builds");

    let before = all_requests(&server).await;
    let _ = tokio::time::timeout(
        Duration::from_secs(2),
        client.get::<Dummy>("/data", Some(1)),
    )
    .await;
    let sent = all_requests(&server).await - before;

    // Whatever the retry budget allows, the account limiter is a burst of one
    // per two seconds, so a burst of attempts cannot slip through untimed.
    assert!(
        sent <= 2,
        "each retry waited on the account budget, {sent} attempts went out at once"
    );
}

/// `HttpClient::new` logs in on the first key that accepts it. One spent key
/// must not throw away a pool whose other keys are fine.
#[tokio::test]
async fn test_new_rotates_when_the_first_key_is_exhausted() {
    let server = MockServer::start().await;

    // The first login is refused for that key's allowance; the next succeeds.
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorCode": "error.public-api.exceeded-api-key-allowance"
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_login_ok(&server).await;

    let client = tokio::time::timeout(
        Duration::from_secs(10),
        HttpClient::new(pool_config_burst(&server.uri(), "key-a,key-b", 20, 1, 20)),
    )
    .await;

    assert!(
        matches!(client, Ok(Ok(_))),
        "construction rotated to the second key, got {:?}",
        client.as_ref().map(std::result::Result::is_ok)
    );

    let logins = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/session")
        .count();
    assert_eq!(
        logins, 2,
        "the refused key was not retried, the next one was"
    );

    // Construction succeeding is not enough: the client must have *adopted* the
    // key that authenticated. Trading is pinned to the primary slot, so an order
    // shows which key that is - and it must not be the exhausted one.
    let client = client.expect("timeout").expect("client built");
    Mock::given(method("POST"))
        .and(path("/positions/otc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;
    client
        .post::<_, Dummy>("/positions/otc", serde_json::json!({}), Some(2))
        .await
        .expect("order accepted");

    let order_key = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/positions/otc")
        .filter_map(|r| {
            r.headers
                .get("X-IG-API-KEY")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        })
        .next_back()
        .expect("an order was sent");
    assert_eq!(
        order_key, "key-b",
        "trading uses the key that authenticated, not the exhausted one"
    );
}

/// The account bucket is one flat budget: market data and historical prices
/// draw on the same 30/min, instead of getting 30 each.
#[tokio::test]
async fn test_account_budget_is_shared_between_data_and_history() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;
    Mock::given(method("GET"))
        .and(path("/prices/EPIC"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    // Account id keys the shared bucket; a unique one keeps this test
    // independent of the others running in the same process.
    let mut config = pool_config_burst(&server.uri(), "key-hist", 100, 60, 100);
    config.credentials.account_id = "SHARED-BUDGET-TEST".to_string();
    let client = HttpClient::new_lazy(config).expect("client builds");

    // Alternate the two classes over a fixed window. The account bucket refills
    // one token every two seconds, so a shared bucket lets about three requests
    // through in six seconds; two separate buckets would let through roughly
    // twice that, since each class would refill on its own.
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut i = 0;
    while Instant::now() < deadline {
        let endpoint = if i % 2 == 0 { "/data" } else { "/prices/EPIC" };
        if tokio::time::timeout(
            Duration::from_secs(3),
            client.get::<Dummy>(endpoint, Some(1)),
        )
        .await
        .is_err()
        {
            break;
        }
        i += 1;
    }

    let total = all_requests(&server).await;
    assert!(
        i >= 2,
        "the window exercised both classes, only {i} calls made"
    );
    assert!(
        total <= 5,
        "history and market data shared one account budget: {total} requests in six seconds"
    );
}

/// A single key keeps failing construction: rotation must not turn a real
/// authentication failure into a silent success.
#[tokio::test]
async fn test_new_fails_when_no_key_can_authenticate() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorCode": "error.public-api.exceeded-api-key-allowance"
        })))
        .mount(&server)
        .await;

    let client = tokio::time::timeout(
        Duration::from_secs(10),
        HttpClient::new(pool_config_burst(&server.uri(), "key-a,key-b", 20, 1, 20)),
    )
    .await;

    match client {
        Ok(Err(AppError::ApiKeyAllowanceExceeded)) => {}
        other => panic!(
            "expected ApiKeyAllowanceExceeded, got {:?}",
            other.map(|r| r.is_ok())
        ),
    }
}

/// The lazy path settles the primary too.
///
/// `Client::try_new` and `Client::with_config` build lazily, so the first login
/// happens when a request arrives rather than at construction. If the first key
/// is refused then, the primary has to move with it — otherwise trading and
/// streaming keep pointing at the key that could not authenticate.
#[tokio::test]
async fn test_lazy_client_moves_the_primary_to_the_key_that_authenticates() {
    let server = MockServer::start().await;

    // key-a is refused at login; key-b authenticates.
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorCode": "error.public-api.exceeded-api-key-allowance"
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_login_ok(&server).await;
    mount_data_ok(&server).await;
    Mock::given(method("POST"))
        .and(path("/positions/otc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a,key-b", 20, 1, 20))
        .expect("client builds");

    // A non-trading read triggers the first login and the rotation.
    client
        .get::<Dummy>("/data", Some(1))
        .await
        .expect("read served");

    // Trading pins to the primary. It must have followed the rotation.
    client
        .post::<_, Dummy>("/positions/otc", serde_json::json!({}), Some(2))
        .await
        .expect("order accepted");

    let order_key = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/positions/otc")
        .filter_map(|r| {
            r.headers
                .get("X-IG-API-KEY")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        })
        .next_back()
        .expect("an order was sent");
    assert_eq!(
        order_key, "key-b",
        "the primary followed the key that authenticated on the lazy path"
    );
}

/// `switch_account` is refused with a pool rather than silently leaving the
/// other slots on the previous account.
#[tokio::test]
async fn test_switch_account_is_refused_with_a_key_pool() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;

    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "key-a,key-b", 20, 1, 20))
        .expect("client builds");

    let err = client
        .switch_account("OTHER", Some(false))
        .await
        .expect_err("switching is refused with a pool");
    assert!(
        matches!(err, AppError::UnsupportedWithKeyPool(_)),
        "got {err:?}"
    );

    let switches = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path() == "/session")
        .filter(|r| r.method == wiremock::http::Method::PUT)
        .count();
    assert_eq!(switches, 0, "no key was switched behind the others' backs");
}

/// A single key is not stopped by the pool guard.
///
/// It can still fail for its own reasons — switching is unsupported on OAuth
/// sessions, which is a pre-existing limitation of `Auth` and unrelated to the
/// pool — but the refusal must not be `UnsupportedWithKeyPool`.
#[tokio::test]
async fn test_switch_account_still_works_with_one_key() {
    let server = MockServer::start().await;
    mount_login_ok(&server).await;
    Mock::given(method("PUT"))
        .and(path("/session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "trailingStopsEnabled": false,
            "dealingEnabled": true,
            "hasActiveDemoAccounts": true,
            "hasActiveLiveAccounts": false
        })))
        .mount(&server)
        .await;

    let client = HttpClient::new_lazy(pool_config_burst(&server.uri(), "only-key", 20, 1, 20))
        .expect("client builds");

    let result = client.switch_account("OTHER", Some(false)).await;
    assert!(
        !matches!(result, Err(AppError::UnsupportedWithKeyPool(_))),
        "the pool guard did not fire for a single key, got {result:?}"
    );
}
