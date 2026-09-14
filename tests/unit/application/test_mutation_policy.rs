//! Hermetic mutation acceptance tests. Every config and session is fake, and
//! every URL is a local listener. No environment convenience constructor is used.

use ig_client::application::client::Client;
use ig_client::application::config::{Config, Credentials, RateLimiterConfig, RestApiConfig};
use ig_client::application::http::HttpClient;
use ig_client::application::interfaces::order::OrderService;
use ig_client::error::AppError;
use ig_client::model::requests::{
    ClosePositionRequest, CreateOrderRequest, CreateWorkingOrderRequest, UpdatePositionRequest,
    UpdateWorkingOrderRequest,
};
use ig_client::model::retry::RequestPolicy;
use ig_client::presentation::order::{Direction, OrderType, TimeInForce};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
static ACCOUNT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);
const ACK: &str = "FAKE-ACK";

fn config(base_url: &str, api_version: u8) -> Config {
    let credentials = Credentials::new(
        "fake-user".into(),
        "fake-password".into(),
        format!(
            "MUTATION-TEST-{}",
            ACCOUNT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ),
        "FAKE-KEY-A,FAKE-KEY-B".into(),
    );
    Config {
        rest_api: RestApiConfig {
            base_url: base_url.into(),
            timeout: 5,
        },
        rate_limiter: RateLimiterConfig {
            max_requests: 1000,
            period_seconds: 1,
            burst_size: 1000,
        },
        api_version: Some(api_version),
        ..Config::from_credentials(credentials)
    }
}

async fn mount_login(server: &MockServer, config: &Config) {
    // The same captured session shapes as test_auth_flow, with fake values and
    // a matching account so v2 never needs an account-switch request.
    let body = if config.api_version == Some(2) {
        json!({
            "accountType": "CFD", "accountInfo": {
                "balance": 10000.0, "deposit": 2000.0,
                "profitLoss": 150.5, "available": 8000.0
            },
            "currencyIsoCode": "EUR", "currencySymbol": "E",
            "currentAccountId": config.credentials.account_id,
            "lightstreamerEndpoint": "https://example.invalid",
            "accounts": [{ "accountId": config.credentials.account_id,
                "accountName": "Fake", "preferred": true, "accountType": "CFD" }],
            "clientId": "FAKE-CLIENT", "timezoneOffset": 1,
            "hasActiveDemoAccounts": true, "hasActiveLiveAccounts": false,
            "trailingStopsEnabled": false, "reroutingEnvironment": null,
            "dealingEnabled": true
        })
    } else {
        json!({
            "clientId": "FAKE-CLIENT", "accountId": config.credentials.account_id,
            "timezoneOffset": 1, "lightstreamerEndpoint": "https://example.invalid",
            "oauthToken": { "access_token": "FAKE-ACCESS", "refresh_token": "FAKE-REFRESH",
                "scope": "profile", "token_type": "Bearer", "expires_in": "3600" }
        })
    };
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(body)
                .insert_header("CST", "FAKE-CST")
                .insert_header("X-SECURITY-TOKEN", "FAKE-XST"),
        )
        .mount(server)
        .await;
}

#[derive(Debug, Clone, Copy)]
enum Mutation {
    Create,
    Amend,
    AmendLevel,
    Close,
    CreateWorking,
    CancelWorking,
    AmendWorking,
}

const MUTATIONS: [Mutation; 7] = [
    Mutation::Create,
    Mutation::Amend,
    Mutation::AmendLevel,
    Mutation::Close,
    Mutation::CreateWorking,
    Mutation::CancelWorking,
    Mutation::AmendWorking,
];

impl Mutation {
    fn method(self) -> Method {
        match self {
            Self::Create | Self::Close | Self::CreateWorking => Method::POST,
            Self::Amend | Self::AmendLevel | Self::AmendWorking => Method::PUT,
            Self::CancelWorking => Method::DELETE,
        }
    }

    fn path(self) -> &'static str {
        match self {
            Self::Create | Self::Close => "/positions/otc",
            Self::Amend | Self::AmendLevel => "/positions/otc/FAKE-DEAL",
            Self::CreateWorking => "/workingorders/otc",
            Self::CancelWorking | Self::AmendWorking => "/workingorders/otc/FAKE-DEAL",
        }
    }

    async fn send(self, client: &Client) -> Result<String, AppError> {
        match self {
            Self::Create => client
                .create_order(&CreateOrderRequest::market(
                    "FAKE-EPIC".into(),
                    Direction::Buy,
                    1.0,
                    Some("EUR".into()),
                    Some("FAKE-REFERENCE".into()),
                ))
                .await
                .map(|r| r.deal_reference),
            Self::Amend => client
                .update_position(
                    "FAKE-DEAL",
                    &UpdatePositionRequest {
                        guaranteed_stop: None,
                        limit_level: Some(2.0),
                        stop_level: None,
                        trailing_stop: None,
                        trailing_stop_distance: None,
                        trailing_stop_increment: None,
                    },
                )
                .await
                .map(|r| r.deal_reference),
            Self::AmendLevel => client
                .update_level_in_position("FAKE-DEAL", Some(2.0))
                .await
                .map(|r| r.deal_reference),
            Self::Close => client
                .close_position(&ClosePositionRequest::market(
                    "FAKE-DEAL".into(),
                    Direction::Sell,
                    1.0,
                ))
                .await
                .map(|r| r.deal_reference),
            Self::CreateWorking => client
                .create_working_order(&CreateWorkingOrderRequest::limit(
                    "FAKE-EPIC".into(),
                    Direction::Buy,
                    1.0,
                    2.0,
                    "EUR".into(),
                    "-".into(),
                ))
                .await
                .map(|r| r.deal_reference),
            Self::CancelWorking => client
                .delete_working_order("FAKE-DEAL")
                .await
                .map(|()| ACK.into()),
            Self::AmendWorking => client
                .update_working_order(
                    "FAKE-DEAL",
                    &UpdateWorkingOrderRequest::new(
                        2.0,
                        OrderType::Limit,
                        TimeInForce::GoodTillCancelled,
                    ),
                )
                .await
                .map(|r| r.deal_reference),
        }
    }
}

async fn mutation_requests(server: &MockServer) -> Vec<Request> {
    let requests = server
        .received_requests()
        .await
        .expect("request recording is enabled");
    let logins = requests
        .iter()
        .filter(|r| r.url.path() == "/session")
        .count();
    assert_eq!(logins, 1, "only initial authentication is allowed");
    // Include every non-login request, regardless of method/path/headers. A
    // malformed replay or refresh-token call must not escape the count.
    requests
        .into_iter()
        .filter(|r| r.url.path() != "/session")
        .collect()
}

fn assert_wire_request(operation: Mutation, requests: &[Request]) {
    assert_eq!(requests.len(), 1, "{operation:?} must have one actual send");
    let request = requests.first().expect("the one recorded mutation");
    assert_eq!(request.method, operation.method());
    assert_eq!(request.url.path(), operation.path());
    let version = if matches!(operation, Mutation::Close) {
        "1"
    } else {
        "2"
    };
    assert_eq!(
        request.headers.get("version").and_then(|v| v.to_str().ok()),
        Some(version)
    );
    let override_method = request.headers.get("_method").and_then(|v| v.to_str().ok());
    assert_eq!(
        override_method,
        matches!(operation, Mutation::Close).then_some("DELETE")
    );
}

async fn mount_result(server: &MockServer, operation: Mutation, response: ResponseTemplate) {
    Mock::given(path(operation.path()))
        .respond_with(response)
        .with_priority(1)
        .up_to_n_times(1)
        .mount(server)
        .await;
    // A replay would succeed, but the first error must still be returned.
    Mock::given(path(operation.path()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "dealReference": ACK })))
        .with_priority(2)
        .mount(server)
        .await;
}

#[tokio::test]
async fn test_order_mutations_success_preserves_wire_contract() -> TestResult {
    for operation in MUTATIONS {
        let server = MockServer::start().await;
        let config = config(&server.uri(), 3);
        mount_login(&server, &config).await;
        mount_result(
            &server,
            operation,
            ResponseTemplate::new(200).set_body_json(json!({ "dealReference": ACK })),
        )
        .await;
        let client = Client::with_config(config)?;
        assert_eq!(operation.send(&client).await?, ACK);
        let requests = mutation_requests(&server).await;
        assert_wire_request(operation, &requests);
        let request = requests.first().expect("one mutation");
        if matches!(operation, Mutation::CancelWorking) {
            assert!(request.body.is_empty());
        } else {
            let body: Value = serde_json::from_slice(&request.body)?;
            match operation {
                Mutation::Create | Mutation::CreateWorking => {
                    assert_eq!(body.get("epic"), Some(&json!("FAKE-EPIC")));
                    assert_eq!(body.get("direction"), Some(&json!("BUY")));
                    assert_eq!(body.get("size"), Some(&json!(1.0)));
                }
                Mutation::Close => {
                    assert_eq!(body.get("dealId"), Some(&json!("FAKE-DEAL")));
                    assert_eq!(body.get("direction"), Some(&json!("SELL")));
                    assert_eq!(body.get("size"), Some(&json!(1.0)));
                }
                Mutation::Amend | Mutation::AmendLevel => {
                    assert_eq!(body.get("limitLevel"), Some(&json!(2.0)))
                }
                Mutation::AmendWorking => assert_eq!(body.get("level"), Some(&json!(2.0))),
                Mutation::CancelWorking => unreachable!("cancel has no body"),
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_order_mutations_http_errors_are_not_retried() -> TestResult {
    for operation in MUTATIONS {
        for status in [400, 429, 500, 503] {
            let server = MockServer::start().await;
            let config = config(&server.uri(), 3);
            mount_login(&server, &config).await;
            mount_result(&server, operation, ResponseTemplate::new(status)).await;
            let client = Client::with_config(config)?;
            let error = operation
                .send(&client)
                .await
                .expect_err("initial failure must be returned");
            match (status, error) {
                (429, AppError::RateLimitExceeded) => {}
                (status, AppError::Unexpected(actual)) if status != 429 => {
                    assert_eq!(actual.as_u16(), status);
                }
                _ => panic!("unexpected error variant for {operation:?} status {status}"),
            }
            assert_wire_request(operation, &mutation_requests(&server).await);
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_order_mutations_401_never_refresh_or_replay() -> TestResult {
    for operation in MUTATIONS {
        for api_version in [2, 3] {
            for oauth_marker in [false, true] {
                let server = MockServer::start().await;
                let config = config(&server.uri(), api_version);
                mount_login(&server, &config).await;
                let code = if oauth_marker {
                    "error.security.oauth-token-invalid"
                } else {
                    "error.security.client-token-invalid"
                };
                mount_result(
                    &server,
                    operation,
                    ResponseTemplate::new(401).set_body_json(json!({ "errorCode": code })),
                )
                .await;
                let client = Client::with_config(config)?;
                let error = operation
                    .send(&client)
                    .await
                    .expect_err("401 must be returned");
                assert!(if oauth_marker {
                    matches!(error, AppError::OAuthTokenExpired)
                } else {
                    matches!(error, AppError::Unauthorized)
                });
                assert_wire_request(operation, &mutation_requests(&server).await);
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_order_mutations_allowances_never_rotate() -> TestResult {
    for operation in MUTATIONS {
        for allowance in [
            "api-key",
            "account",
            "account-trading",
            "account-historical-data",
        ] {
            let server = MockServer::start().await;
            let config = config(&server.uri(), 3);
            mount_login(&server, &config).await;
            mount_result(&server, operation, ResponseTemplate::new(403)
                .set_body_json(json!({ "errorCode": format!("error.public-api.exceeded-{allowance}-allowance") }))).await;
            let client = Client::with_config(config)?;
            let error = operation
                .send(&client)
                .await
                .expect_err("allowance failure must be returned");
            assert!(match allowance {
                "api-key" => matches!(error, AppError::ApiKeyAllowanceExceeded),
                "account" => matches!(error, AppError::AccountAllowanceExceeded),
                "account-trading" => matches!(error, AppError::TradingAllowanceExceeded),
                _ => matches!(error, AppError::HistoricalDataAllowanceExceeded { .. }),
            });
            assert_wire_request(operation, &mutation_requests(&server).await);
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_order_mutations_redirects_never_reach_destination() -> TestResult {
    for operation in MUTATIONS {
        for status in [301, 302, 303, 307, 308] {
            let server = MockServer::start().await;
            let destination = MockServer::start().await;
            let config = config(&server.uri(), 3);
            mount_login(&server, &config).await;
            mount_result(
                &server,
                operation,
                ResponseTemplate::new(status)
                    .insert_header("Location", format!("{}/redirected", destination.uri())),
            )
            .await;
            let client = Client::with_config(config)?;
            let error = operation
                .send(&client)
                .await
                .expect_err("redirect must be returned");
            assert!(matches!(error, AppError::Unexpected(actual) if actual.as_u16() == status));
            assert_wire_request(operation, &mutation_requests(&server).await);
            assert!(
                destination
                    .received_requests()
                    .await
                    .expect("recording enabled")
                    .is_empty()
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_order_mutations_invalid_success_is_not_replayed() -> TestResult {
    for operation in MUTATIONS {
        let server = MockServer::start().await;
        let config = config(&server.uri(), 3);
        mount_login(&server, &config).await;
        mount_result(
            &server,
            operation,
            ResponseTemplate::new(200).set_body_string("invalid-json"),
        )
        .await;
        let client = Client::with_config(config)?;
        assert!(matches!(
            operation.send(&client).await,
            Err(AppError::Deserialization(_))
        ));
        assert_wire_request(operation, &mutation_requests(&server).await);
    }
    Ok(())
}

#[tokio::test]
async fn test_order_mutations_failed_session_sends_no_order() -> TestResult {
    for operation in MUTATIONS {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/session"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let client = Client::with_config(config(&server.uri(), 3))?;
        assert!(matches!(
            operation.send(&client).await,
            Err(AppError::Unauthorized)
        ));
        assert!(mutation_requests(&server).await.is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn test_http_wrappers_cannot_weaken_trading_policy() -> TestResult {
    for operation in MUTATIONS {
        for absolute_url in [false, true] {
            for explicit_standard in [false, true] {
                let server = MockServer::start().await;
                let config = config(&server.uri(), 3);
                mount_login(&server, &config).await;
                mount_result(&server, operation, ResponseTemplate::new(401)).await;
                let client = HttpClient::new_lazy(config)?;
                let target = if absolute_url {
                    format!("{}{}", server.uri(), operation.path())
                } else {
                    operation.path().to_owned()
                };
                let result: Result<Value, AppError> = if explicit_standard {
                    client
                        .request_with_policy(
                            operation.method(),
                            &target,
                            Some(json!({})),
                            Some(2),
                            RequestPolicy::Standard,
                        )
                        .await
                } else {
                    match operation {
                        Mutation::Close => {
                            client
                                .post_with_delete_method(&target, json!({}), Some(1))
                                .await
                        }
                        Mutation::Create | Mutation::CreateWorking => {
                            client.post(&target, json!({}), Some(2)).await
                        }
                        Mutation::Amend | Mutation::AmendLevel | Mutation::AmendWorking => {
                            client.put(&target, json!({}), Some(2)).await
                        }
                        Mutation::CancelWorking => client.delete(&target, Some(2)).await,
                    }
                };
                assert!(matches!(result, Err(AppError::Unauthorized)));
                assert_eq!(mutation_requests(&server).await.len(), 1);
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_configured_base_path_cannot_bypass_trading_policy() -> TestResult {
    for base_path in ["/positions", "/positions/x/..", "/positions/%2E"] {
        for status in [401, 429, 503] {
            let server = MockServer::start().await;
            // The client uses the trading parent as its base URL. Its login is served
            // locally at that base too, so every request remains hermetic.
            let mut nested = config(&server.uri(), 3);
            nested.rest_api.base_url = format!("{}{base_path}", server.uri());
            Mock::given(method("POST"))
                .and(path("/positions/session"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "clientId": "FAKE-CLIENT", "accountId": nested.credentials.account_id,
                    "timezoneOffset": 1, "lightstreamerEndpoint": "https://example.invalid",
                    "oauthToken": {"access_token": "FAKE-ACCESS", "refresh_token": "FAKE-REFRESH",
                        "scope": "profile", "token_type": "Bearer", "expires_in": "3600"}
                })))
                .mount(&server)
                .await;
            let client = HttpClient::new_lazy(nested)?;
            Mock::given(path("/positions/otc"))
                .respond_with(ResponseTemplate::new(status))
                .mount(&server)
                .await;
            let result: Result<Value, AppError> = client.post("otc", json!({}), Some(2)).await;
            assert!(match status {
                401 => matches!(result, Err(AppError::Unauthorized)),
                429 => matches!(result, Err(AppError::RateLimitExceeded)),
                _ => matches!(
                    result,
                    Err(AppError::Unexpected(StatusCode::SERVICE_UNAVAILABLE))
                ),
            });
            let requests = server.received_requests().await.expect("recording enabled");
            assert_eq!(
                requests
                    .iter()
                    .filter(|r| r.url.path() == "/positions/otc")
                    .count(),
                1
            );
            assert_eq!(
                requests
                    .iter()
                    .filter(|r| r.url.path() == "/positions/session")
                    .count(),
                1
            );
            assert_eq!(
                requests.len(),
                2,
                "only a login and one mutation are allowed"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_canonical_dot_paths_cannot_replay_mutations() -> TestResult {
    for (target, canonical) in [
        ("positions/./otc", "/positions/otc"),
        ("positions/x/../otc", "/positions/otc"),
        ("positions/%2e/otc", "/positions/otc"),
        ("positions/x/%2E%2E/otc", "/positions/otc"),
        ("workingorders/./otc", "/workingorders/otc"),
        ("workingorders/x/../otc", "/workingorders/otc"),
    ] {
        for absolute in [false, true] {
            for status in [401, 429, 503] {
                let server = MockServer::start().await;
                let config = config(&server.uri(), 3);
                mount_login(&server, &config).await;
                Mock::given(path(canonical))
                    .respond_with(ResponseTemplate::new(status))
                    .with_priority(1)
                    .up_to_n_times(1)
                    .mount(&server)
                    .await;
                Mock::given(path(canonical))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
                    .with_priority(2)
                    .mount(&server)
                    .await;
                let client = HttpClient::new_lazy(config)?;
                let target = if absolute {
                    format!("{}/{target}", server.uri())
                } else {
                    target.into()
                };
                let result: Result<Value, AppError> = client
                    .request_with_policy(
                        Method::POST,
                        &target,
                        Some(json!({})),
                        Some(2),
                        RequestPolicy::Standard,
                    )
                    .await;
                assert!(match status {
                    401 => matches!(result, Err(AppError::Unauthorized)),
                    429 => matches!(result, Err(AppError::RateLimitExceeded)),
                    _ => matches!(
                        result,
                        Err(AppError::Unexpected(StatusCode::SERVICE_UNAVAILABLE))
                    ),
                });
                let requests = mutation_requests(&server).await;
                assert_eq!(requests.len(), 1, "canonical mutation must have one send");
                assert_eq!(requests.first().expect("one send").url.path(), canonical);
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_escaped_mutation_aliases_cannot_enable_replay() -> TestResult {
    for target in ["/positions/%6Ftc", "/%70ositions/otc", "/positions%2Fotc"] {
        for verb in [Method::POST, Method::PUT, Method::DELETE] {
            let server = MockServer::start().await;
            let config = config(&server.uri(), 3);
            mount_login(&server, &config).await;
            Mock::given(path(target))
                .respond_with(ResponseTemplate::new(401))
                .mount(&server)
                .await;
            let client = HttpClient::new_lazy(config)?;
            let result: Result<Value, AppError> = client
                .request_with_policy(
                    verb,
                    target,
                    Some(json!({})),
                    Some(2),
                    RequestPolicy::Standard,
                )
                .await;
            assert!(matches!(result, Err(AppError::Unauthorized)));
            let requests = mutation_requests(&server).await;
            assert_eq!(requests.len(), 1);
            assert_eq!(requests.first().expect("one send").url.path(), target);
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_explicit_single_attempt_prevents_nontrading_replay_and_rotation() -> TestResult {
    for status in [401, 403, 429, 503] {
        let server = MockServer::start().await;
        let config = config(&server.uri(), 3);
        mount_login(&server, &config).await;
        Mock::given(path("/custom-mutation"))
            .respond_with(
                ResponseTemplate::new(status).set_body_json(
                    json!({"errorCode": "error.public-api.exceeded-api-key-allowance"}),
                ),
            )
            .mount(&server)
            .await;
        let client = HttpClient::new_lazy(config)?;
        let result = client
            .request_with_policy::<_, Value>(
                Method::POST,
                "/custom-mutation",
                Some(json!({})),
                Some(1),
                RequestPolicy::SingleAttempt,
            )
            .await;
        assert!(match status {
            401 => matches!(result, Err(AppError::Unauthorized)),
            403 => matches!(result, Err(AppError::ApiKeyAllowanceExceeded)),
            429 => matches!(result, Err(AppError::RateLimitExceeded)),
            _ => matches!(
                result,
                Err(AppError::Unexpected(StatusCode::SERVICE_UNAVAILABLE))
            ),
        });
        assert_eq!(mutation_requests(&server).await.len(), 1);
    }
    Ok(())
}

#[tokio::test]
async fn test_standard_get_still_follows_redirects() -> TestResult {
    let server = MockServer::start().await;
    let config = config(&server.uri(), 3);
    mount_login(&server, &config).await;
    Mock::given(method("GET"))
        .and(path("/positions/otc"))
        .respond_with(ResponseTemplate::new(307).insert_header("Location", "/positions"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/positions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"positions": []})))
        .expect(1)
        .mount(&server)
        .await;
    let client = HttpClient::new_lazy(config)?;
    let response: Value = client.get("/positions/otc", Some(2)).await?;
    assert_eq!(response, json!({"positions": []}));
    assert_eq!(mutation_requests(&server).await.len(), 2);
    Ok(())
}

#[tokio::test]
async fn test_eager_client_also_enforces_single_attempt_transport() -> TestResult {
    let server = MockServer::start().await;
    let config = config(&server.uri(), 3);
    mount_login(&server, &config).await;
    Mock::given(path("/positions/otc"))
        .respond_with(ResponseTemplate::new(307).insert_header("Location", "/replayed"))
        .mount(&server)
        .await;
    let client = HttpClient::new(config).await?;
    let result: Result<Value, AppError> = client.post("positions/otc", json!({}), Some(2)).await;
    assert!(matches!(
        result,
        Err(AppError::Unexpected(StatusCode::TEMPORARY_REDIRECT))
    ));
    assert_eq!(mutation_requests(&server).await.len(), 1);
    Ok(())
}
