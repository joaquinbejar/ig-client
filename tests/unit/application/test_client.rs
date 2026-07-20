use ig_client::application::client::Client;
use ig_client::application::config::{Config, Credentials, RestApiConfig};
use ig_client::application::interfaces::market::MarketService;
use ig_client::error::AppError;

#[tokio::test]
async fn get_multiple_market_details_empty_returns_default() {
    let client = Client::try_new().expect("client construction should succeed");
    let resp = client
        .get_multiple_market_details(&[])
        .await
        .expect("should be Ok for empty");
    assert!(resp.market_details.is_empty());
}

#[tokio::test]
async fn get_multiple_market_details_more_than_50_returns_error() {
    let client = Client::try_new().expect("client construction should succeed");
    // Build 51 dummy EPICs
    let epics: Vec<String> = (0..51).map(|i| format!("EPIC{}", i)).collect();
    let err = client
        .get_multiple_market_details(&epics)
        .await
        .expect_err("should be Err");
    match err {
        AppError::InvalidInput(msg) => {
            assert!(msg.contains("maximum number of EPICs is 50"));
        }
        other => panic!("Unexpected error: {:?}", other),
    }
}

#[test]
fn client_try_new_succeeds() {
    // `try_new` is the environment convenience constructor and must not panic;
    // it returns the built client on a healthy TLS backend.
    let result = Client::try_new();
    assert!(result.is_ok(), "client try_new should succeed");
}

#[test]
fn test_client_with_config_uses_injected_base_url() {
    // The injected values differ from the crate defaults and from anything the
    // `IG_*` namespace or a `.env` file could plausibly hold, so seeing them on
    // the built client proves the config came from the caller, not the
    // environment. No env var is mutated: `set_var` is `unsafe` on edition 2024
    // and racy under the parallel test harness.
    let credentials = Credentials::new(
        "embedder-user".to_string(),
        "embedder-password".to_string(),
        "EMBEDDER-ACC".to_string(),
        "embedder-api-key".to_string(),
    );
    let config = Config {
        rest_api: RestApiConfig {
            base_url: "https://injected.example.invalid/gateway/deal".to_string(),
            timeout: 11,
        },
        ..Config::from_credentials(credentials)
    };

    let client = Client::with_config(config).expect("client with_config should succeed");

    assert_eq!(
        client.config().rest_api.base_url,
        "https://injected.example.invalid/gateway/deal"
    );
    assert_eq!(client.config().rest_api.timeout, 11);
    assert_eq!(client.config().credentials.username, "embedder-user");
    assert_eq!(client.config().credentials.account_id, "EMBEDDER-ACC");
}

#[test]
fn test_client_try_new_still_uses_environment_config() {
    // Back-compat: `try_new` keeps reading the environment / `.env` path and is
    // unaffected by the new injection constructor.
    let client = Client::try_new().expect("client try_new should succeed");
    assert_eq!(
        client.config().rest_api.base_url,
        Config::new().rest_api.base_url
    );
}
