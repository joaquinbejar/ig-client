use ig_client::application::client::Client;
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
    // `try_new` is the sole constructor and must not panic; it returns the
    // built client on a healthy TLS backend.
    let result = Client::try_new();
    assert!(result.is_ok(), "client try_new should succeed");
}
