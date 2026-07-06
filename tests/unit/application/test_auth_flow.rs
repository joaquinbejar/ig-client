//! Offline auth / session lifecycle tests driving `Auth` and `HttpClient`
//! against a `wiremock::MockServer`. No live network, no real credentials —
//! all identifiers are obvious `FAKE-*` / `ABC12` placeholders.

use ig_client::application::auth::Auth;
use ig_client::application::config::{
    Config, Credentials, RateLimiterConfig, RestApiConfig, WebSocketConfig,
};
use ig_client::application::http::HttpClient;
use ig_client::error::{AppError, AuthError};
use ig_client::storage::config::DatabaseConfig;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Builds a hermetic `Config` pointing at `base_url` with the given API version.
/// Constructed directly (never via `Config::default`) so the tests do not read
/// `.env` / environment variables and stay deterministic. The rate limiter is
/// deliberately permissive so pacing never slows the tests.
fn test_config(base_url: &str, api_version: u8) -> Config {
    Config {
        credentials: Credentials {
            username: "fake-user".to_string(),
            password: "fake-pass".to_string(),
            account_id: "ABC12".to_string(),
            api_key: "FAKE-API-KEY".to_string(),
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
            max_requests: 1000,
            period_seconds: 1,
            burst_size: 100,
        },
        sleep_hours: 1,
        page_size: 20,
        days_to_look_back: 7,
        api_version: Some(api_version),
    }
}

/// Captured-shape v2 `/session` body. `currentAccountId` matches the configured
/// `ABC12` so `login` does not attempt an account switch.
const V2_BODY: &str = r#"{
    "accountType": "CFD",
    "accountInfo": { "balance": 10000.0, "deposit": 2000.0, "profitLoss": 150.5, "available": 8000.0 },
    "currencyIsoCode": "EUR",
    "currencySymbol": "E",
    "currentAccountId": "ABC12",
    "lightstreamerEndpoint": "https://demo-apd.marketdatasystems.com",
    "accounts": [ { "accountId": "ABC12", "accountName": "Demo", "preferred": true, "accountType": "CFD" } ],
    "clientId": "FAKE-CLIENT-1",
    "timezoneOffset": 1,
    "hasActiveDemoAccounts": true,
    "hasActiveLiveAccounts": false,
    "trailingStopsEnabled": false,
    "reroutingEnvironment": null,
    "dealingEnabled": true
}"#;

/// v3 (OAuth) `/session` body with a configurable `expires_in`. Fake tokens only.
fn v3_body(expires_in: &str) -> String {
    format!(
        r#"{{
            "clientId": "FAKE-CLIENT-1",
            "accountId": "ABC12",
            "timezoneOffset": 1,
            "lightstreamerEndpoint": "https://demo-apd.marketdatasystems.com",
            "oauthToken": {{
                "access_token": "FAKE-ACCESS",
                "refresh_token": "FAKE-REFRESH",
                "scope": "profile",
                "token_type": "Bearer",
                "expires_in": "{expires_in}"
            }}
        }}"#
    )
}

/// IG-shaped 401 body signalling an invalidated OAuth token.
const OAUTH_INVALID_BODY: &str = r#"{"errorCode":"error.security.oauth-token-invalid"}"#;

/// A `200 OK` response carrying a JSON body with the correct content type.
fn json_ok(body: impl Into<Vec<u8>>) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body, "application/json")
}

#[tokio::test]
async fn test_login_v2_returns_session_with_cst_and_security_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(
            json_ok(V2_BODY)
                .insert_header("CST", "FAKE-CST")
                .insert_header("X-SECURITY-TOKEN", "FAKE-XST"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let auth = Auth::new(Arc::new(test_config(&server.uri(), 2)));
    let session = auth.login().await.expect("v2 login should succeed");

    assert!(!session.is_oauth());
    assert_eq!(session.cst.as_deref(), Some("FAKE-CST"));
    assert_eq!(session.x_security_token.as_deref(), Some("FAKE-XST"));
    assert_eq!(session.account_id, "ABC12");
    assert_eq!(session.api_version, 2);
}

#[tokio::test]
async fn test_login_v2_missing_cst_header_yields_missing_session_token() {
    let server = MockServer::start().await;
    // Body present, but the CST response header is missing.
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(json_ok(V2_BODY).insert_header("X-SECURITY-TOKEN", "FAKE-XST"))
        .mount(&server)
        .await;

    let auth = Auth::new(Arc::new(test_config(&server.uri(), 2)));
    // A rejected / malformed auth response maps to a typed auth error naming
    // the missing header — NOT AppError::InvalidInput (which means bad caller
    // input).
    match auth.login().await {
        Err(AppError::Auth(AuthError::MissingSessionToken(header))) => assert_eq!(header, "cst"),
        other => panic!("expected MissingSessionToken for missing CST, got {other:?}"),
    }
}

#[tokio::test]
async fn test_login_v2_missing_security_token_header_yields_missing_session_token() {
    let server = MockServer::start().await;
    // CST present, X-SECURITY-TOKEN missing.
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(json_ok(V2_BODY).insert_header("CST", "FAKE-CST"))
        .mount(&server)
        .await;

    let auth = Auth::new(Arc::new(test_config(&server.uri(), 2)));
    match auth.login().await {
        Err(AppError::Auth(AuthError::MissingSessionToken(header))) => {
            assert_eq!(header, "x-security-token");
        }
        other => panic!("expected MissingSessionToken for missing X-SECURITY-TOKEN, got {other:?}"),
    }
}

#[tokio::test]
async fn test_login_v3_returns_oauth_session() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(json_ok(v3_body("60")))
        .expect(1)
        .mount(&server)
        .await;

    let auth = Auth::new(Arc::new(test_config(&server.uri(), 3)));
    let session = auth.login().await.expect("v3 login should succeed");

    assert!(session.is_oauth());
    assert_eq!(session.api_version, 3);
    assert_eq!(session.account_id, "ABC12");
}

#[tokio::test]
async fn test_get_session_refreshes_expired_session_with_relogin() {
    let server = MockServer::start().await;
    // `expires_in: 0` makes every issued session already expired, so the cached
    // session is never reused — `get_session` must perform a fresh login.
    // Hit #1 = the explicit `login`; hit #2 = the refresh-driven re-login.
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(json_ok(v3_body("0")))
        .expect(2)
        .mount(&server)
        .await;

    let auth = Auth::new(Arc::new(test_config(&server.uri(), 3)));
    let first = auth.login().await.expect("initial login should succeed");
    assert!(first.is_oauth());

    // The cached session is expired, so this triggers a re-login (2nd hit).
    let refreshed = auth
        .get_session()
        .await
        .expect("get_session should re-login an expired session");
    assert!(refreshed.is_oauth());
    // On drop the MockServer verifies `expect(2)` — exactly one extra re-login.
}

#[tokio::test]
async fn test_request_refreshes_once_and_replays_on_oauth_token_expired() {
    // Mandated contract: a 401 `oauth-token-invalid` triggers exactly one
    // transparent refresh + replay, and the replayed request succeeds.
    let server = MockServer::start().await;

    // Session established during construction (a valid 60s OAuth session).
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(json_ok(v3_body("60")))
        .mount(&server)
        .await;
    let client = HttpClient::new(test_config(&server.uri(), 3))
        .await
        .expect("client construction (initial login) should succeed");

    // Forget the construction-time login so the refresh below is counted in
    // isolation: after `reset`, the ONLY `POST /session` is the refresh.
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(json_ok(v3_body("60")))
        .expect(1) // exactly one refresh
        .named("single refresh login")
        .mount(&server)
        .await;

    // First business call 401s (token invalid); the replay returns 200.
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(
            ResponseTemplate::new(401).set_body_raw(OAUTH_INVALID_BODY, "application/json"),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(json_ok(r#"{"value":42}"#))
        .with_priority(2)
        .mount(&server)
        .await;

    let value: serde_json::Value = client
        .get("accounts", Some(1))
        .await
        .expect("request should succeed after one refresh and replay");
    assert_eq!(
        value.get("value").and_then(serde_json::Value::as_i64),
        Some(42)
    );
    // MockServer drop verifies the refresh mock's `expect(1)`: refresh happened
    // exactly once.
}

#[tokio::test]
async fn test_request_does_not_loop_on_repeated_oauth_token_expired() {
    // A SECOND consecutive 401 after the single refresh must propagate as a
    // typed error — never an infinite refresh loop.
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(json_ok(v3_body("60")))
        .mount(&server)
        .await;
    let client = HttpClient::new(test_config(&server.uri(), 3))
        .await
        .expect("client construction should succeed");

    server.reset().await;
    // Only ONE refresh is permitted even though the business call keeps 401ing.
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(json_ok(v3_body("60")))
        .expect(1)
        .named("single refresh login")
        .mount(&server)
        .await;
    // Business call always 401s: initial attempt + one replay = exactly 2 hits.
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .respond_with(
            ResponseTemplate::new(401).set_body_raw(OAUTH_INVALID_BODY, "application/json"),
        )
        .expect(2)
        .mount(&server)
        .await;

    match client.get::<serde_json::Value>("accounts", Some(1)).await {
        Err(AppError::OAuthTokenExpired) => {}
        other => panic!(
            "expected OAuthTokenExpired after the single refresh was exhausted, got {other:?}"
        ),
    }
    // MockServer drop verifies refresh `expect(1)` and business `expect(2)`.
}
