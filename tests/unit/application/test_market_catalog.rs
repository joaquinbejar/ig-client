//! Offline public-client regressions using synthetic fixtures.
//!
//! Every market, category, date, and session below is invented test data. These
//! counterexamples do not describe captured IG responses or production incidents.

use ig_client::application::client::Client;
use ig_client::application::config::{Config, Credentials, RateLimiterConfig};
use ig_client::application::interfaces::market::MarketService;
use ig_client::error::AppError;
use ig_client::presentation::instrument::InstrumentType;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

type TestResult = Result<(), Box<dyn Error>>;

// Production pacing is shared per account. Independent fixtures must not share
// that budget when the test harness runs them concurrently.
static FIXTURE_ACCOUNT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Build an env-free client with fake credentials and a local mock session.
async fn fixture_client(server: &MockServer) -> Result<Client, Box<dyn Error>> {
    let account_id = format!(
        "CATALOG-FIXTURE-ACCOUNT-{}",
        FIXTURE_ACCOUNT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    Mock::given(method("POST"))
        .and(path("/session"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "clientId": "FIXTURE-CLIENT",
            "accountId": account_id,
            "timezoneOffset": 0,
            "lightstreamerEndpoint": "https://example.invalid",
            "oauthToken": {
                "access_token": "FIXTURE-NOT-A-REAL-ACCESS-TOKEN",
                "refresh_token": "FIXTURE-NOT-A-REAL-REFRESH-TOKEN",
                "scope": "profile",
                "token_type": "Bearer",
                "expires_in": "3600"
            }
        })))
        .expect(1)
        .mount(server)
        .await;
    let mut config = Config::from_credentials(Credentials::new(
        "FIXTURE-USER".into(),
        "FIXTURE-PASSWORD".into(),
        account_id,
        "FIXTURE-API-KEY".into(),
    ));
    config.rest_api.base_url = server.uri();
    config.api_version = Some(3);
    config.rate_limiter = RateLimiterConfig {
        max_requests: 100_000,
        period_seconds: 1,
        burst_size: 2_000,
    };
    Ok(Client::with_config(config)?)
}

fn fixture_instrument(epic: &str, expiry: &str, instrument_type: &str) -> Value {
    json!({
        "epic": epic,
        "instrumentName": format!("Synthetic fixture {epic}"),
        "expiry": expiry,
        "instrumentType": instrument_type,
        "otcTradeable": true,
        "marketStatus": "TRADEABLE",
        "bid": 123.4,
        "offer": 123.6,
        "netChange": 1.25,
        "percentageChange": 0.75,
        "updateTime": "12:34:56"
    })
}

fn fixture_page(page_number: i64, page_size: i64, instruments: Vec<Value>) -> Value {
    json!({
        "instruments": instruments,
        "metadata": {"pageNumber": page_number, "pageSize": page_size}
    })
}

async fn mount_categories(server: &MockServer, categories: &[&str], calls: u64) {
    let categories: Vec<_> = categories
        .iter()
        .map(|id| json!({"code": id, "nonTradeable": *id == "indices"}))
        .collect();
    Mock::given(method("GET"))
        .and(path("/categories"))
        .and(header("Version", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "categories": categories
        })))
        .expect(calls)
        .mount(server)
        .await;
}

async fn mount_page(
    server: &MockServer,
    category: &str,
    requested_page: u32,
    response: Value,
    calls: u64,
) {
    Mock::given(method("GET"))
        .and(path(format!("/categories/{category}/instruments")))
        .and(header("Version", "1"))
        .and(query_param("pageNumber", requested_page.to_string()))
        .and(query_param("pageSize", "500"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .expect(calls)
        .mount(server)
        .await;
}

async fn mount_failure(server: &MockServer, endpoint: &str, page: Option<u32>, calls: u64) {
    let mut mock = Mock::given(method("GET")).and(path(endpoint));
    if let Some(page) = page {
        mock = mock.and(query_param("pageNumber", page.to_string()));
    }
    mock.respond_with(ResponseTemplate::new(404).set_body_json(json!({
        "errorCode": "error.fixture.not-found"
    })))
    .expect(calls)
    .mount(server)
    .await;
}

fn assert_catalog_not_found<T>(result: Result<T, AppError>, expected_endpoint: &str) {
    let Err(AppError::CatalogRequest { endpoint, source }) = result else {
        panic!("expected the fixture's typed catalog request failure");
    };
    assert_eq!(endpoint, expected_endpoint);
    assert!(matches!(
        *source,
        AppError::Unexpected(status) if status == reqwest::StatusCode::NOT_FOUND
    ));
}

fn fixture_details(epic: &str, expiry: &str, last_dealing_date: &str) -> Value {
    json!({
        "instrument": {
            "epic": epic,
            "name": format!("Synthetic fixture {epic}"),
            "expiry": expiry,
            "contractSize": "1",
            "valueOfOnePip": "1",
            "type": "OPT_COMMODITIES",
            "expiryDetails": {"lastDealingDate": last_dealing_date}
        },
        "snapshot": {"marketStatus": "TRADEABLE"},
        "dealingRules": {
            "marketOrderPreference": "AVAILABLE_DEFAULT_OFF",
            "trailingStopsPreference": "AVAILABLE"
        }
    })
}

async fn mount_details(server: &MockServer, epic: &str, response: Value, calls: u64) {
    Mock::given(method("GET"))
        .and(path(format!("/markets/{epic}")))
        .and(header("Version", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .expect(calls)
        .mount(server)
        .await;
}

#[test]
fn test_synthetic_fixture_payloads_deserialize() -> TestResult {
    use ig_client::model::responses::{CategoriesResponse, CategoryInstrumentsResponse};
    use ig_client::presentation::market::MarketDetails;

    let categories: CategoriesResponse = serde_json::from_value(json!({
        "categories": [{"code": "synthetic", "nonTradeable": true}]
    }))?;
    assert_eq!(categories.categories.len(), 1);
    let page: CategoryInstrumentsResponse = serde_json::from_value(fixture_page(
        0,
        500,
        vec![fixture_instrument(
            "OP.D.FIXTURE.SEP.IP",
            "11-SEP-26",
            "OPT_COMMODITIES",
        )],
    ))?;
    assert_eq!(page.instruments.len(), 1);
    let details: MarketDetails = serde_json::from_value(fixture_details(
        "OP.D.FIXTURE.SEP.IP",
        "11-SEP-26",
        "2026-09-11T16:58:00",
    ))?;
    assert_eq!(details.instrument.expiry, "11-SEP-26");
    assert_eq!(
        details
            .instrument
            .expiry_details
            .ok_or("expiry details missing")?
            .last_dealing_date,
        "2026-09-11T16:58:00"
    );
    Ok(())
}

#[tokio::test]
async fn test_public_catalog_multiple_categories_pages_and_duplicates_are_complete() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    mount_categories(
        &server,
        &["shares", "options", "empty", "indices", "shares"],
        2,
    )
    .await;
    let share = fixture_instrument("CS.D.FIXTURE.SHARE.IP", "-", "SHARES");
    let option = fixture_instrument("OP.D.FIXTURE.SEP.IP", "11-SEP-26", "OPT_COMMODITIES");
    let index = fixture_instrument("IX.D.FIXTURE.CASH.IP", "DFB", "INDICES");
    let commodity = fixture_instrument("CS.D.FIXTURE.GOLD.IP", "DEC-26", "COMMODITIES");
    // The server's effective page size can be smaller than the requested limit.
    mount_page(
        &server,
        "shares",
        0,
        fixture_page(0, 2, vec![share.clone(), option.clone()]),
        2,
    )
    .await;
    mount_page(
        &server,
        "shares",
        1,
        fixture_page(1, 2, vec![index.clone()]),
        2,
    )
    .await;
    // A short nonempty page is not evidence that the category is exhausted.
    mount_page(&server, "shares", 2, fixture_page(2, 2, vec![commodity]), 2).await;
    mount_page(&server, "shares", 3, fixture_page(3, 2, vec![]), 2).await;
    mount_page(
        &server,
        "options",
        0,
        fixture_page(0, 2, vec![option, index.clone()]),
        2,
    )
    .await;
    mount_page(&server, "options", 1, fixture_page(1, 2, vec![]), 2).await;
    mount_page(&server, "empty", 0, fixture_page(0, 2, vec![]), 2).await;
    mount_page(&server, "indices", 0, fixture_page(0, 2, vec![index]), 2).await;
    mount_page(&server, "indices", 1, fixture_page(1, 2, vec![]), 2).await;

    let markets = client.get_all_markets().await?;
    let by_epic: HashMap<_, _> = markets
        .iter()
        .map(|market| (market.epic.as_str(), market))
        .collect();
    assert_eq!(markets.len(), 4);
    let share = by_epic
        .get("CS.D.FIXTURE.SHARE.IP")
        .ok_or("share missing")?;
    assert_eq!(share.instrument_type, InstrumentType::Shares);
    assert_eq!(share.expiry, "-");
    assert_eq!(share.market_status, "TRADEABLE");
    assert_eq!(share.bid, Some(123.4));
    assert_eq!(share.offer, Some(123.6));
    assert_eq!(share.net_change, Some(1.25));
    assert_eq!(share.percentage_change, Some(0.75));
    assert_eq!(share.update_time.as_deref(), Some("12:34:56"));
    assert!(share.update_time_utc.is_none());
    assert!(share.high_limit_price.is_none());
    assert!(share.low_limit_price.is_none());

    let entries = client.get_vec_db_entries().await?;
    assert_eq!(entries.len(), 4);
    assert!(entries.iter().all(|entry| entry.exchange == "IG"));
    assert!(
        entries
            .iter()
            .any(|entry| entry.instrument_type == InstrumentType::Indices)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.instrument_type == InstrumentType::OptCommodities)
    );
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn test_public_catalog_empty_categories_are_complete() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    mount_categories(&server, &[], 2).await;
    assert!(client.get_all_markets().await?.is_empty());
    assert!(client.get_vec_db_entries().await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_public_catalog_first_request_failure_is_an_error() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    mount_failure(&server, "/categories", None, 2).await;
    let expected_endpoint = "categories";
    assert_catalog_not_found(client.get_all_markets().await, expected_endpoint);
    assert_catalog_not_found(client.get_vec_db_entries().await, expected_endpoint);
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn test_public_catalog_category_failure_is_an_error() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    mount_categories(&server, &["good", "broken"], 2).await;
    mount_page(
        &server,
        "good",
        0,
        fixture_page(
            0,
            2,
            vec![fixture_instrument("IX.D.FIXTURE.CASH.IP", "DFB", "INDICES")],
        ),
        2,
    )
    .await;
    mount_page(&server, "good", 1, fixture_page(1, 2, vec![]), 2).await;
    mount_failure(&server, "/categories/broken/instruments", Some(0), 2).await;
    let expected_endpoint = "categories/broken/instruments?pageNumber=0&pageSize=500";
    assert_catalog_not_found(client.get_all_markets().await, expected_endpoint);
    assert_catalog_not_found(client.get_vec_db_entries().await, expected_endpoint);
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn test_public_catalog_intermediate_page_failure_is_an_error() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    mount_categories(&server, &["pages"], 2).await;
    mount_page(
        &server,
        "pages",
        0,
        fixture_page(
            0,
            1,
            vec![fixture_instrument("IX.D.FIXTURE.CASH.IP", "DFB", "INDICES")],
        ),
        2,
    )
    .await;
    mount_failure(&server, "/categories/pages/instruments", Some(1), 2).await;
    let expected_endpoint = "categories/pages/instruments?pageNumber=1&pageSize=500";
    assert_catalog_not_found(client.get_all_markets().await, expected_endpoint);
    assert_catalog_not_found(client.get_vec_db_entries().await, expected_endpoint);
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn test_public_catalog_repeated_page_with_advanced_metadata_is_an_error() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    mount_categories(&server, &["pages"], 2).await;
    let first = fixture_instrument("IX.D.FIXTURE.ONE.IP", "DFB", "INDICES");
    let second = fixture_instrument("IX.D.FIXTURE.TWO.IP", "DFB", "INDICES");
    mount_page(
        &server,
        "pages",
        0,
        fixture_page(0, 2, vec![first.clone(), second.clone()]),
        2,
    )
    .await;
    // Reordering instruments and updating their quotes must not defeat the guard.
    let mut repeated_first = first;
    repeated_first["bid"] = json!(999.0);
    mount_page(
        &server,
        "pages",
        1,
        fixture_page(1, 2, vec![second, repeated_first]),
        2,
    )
    .await;
    assert!(client.get_all_markets().await.is_err());
    assert!(client.get_vec_db_entries().await.is_err());
    Ok(())
}

#[tokio::test]
async fn test_public_catalog_incoherent_page_metadata_is_an_error() -> TestResult {
    // Synthetic protocol violations: wrong page, zero page size, oversized
    // response, missing metadata, and an effective page size above the request.
    let instrument = fixture_instrument("IX.D.FIXTURE.CASH.IP", "DFB", "INDICES");
    for response in [
        fixture_page(1, 2, vec![]),
        fixture_page(0, 0, vec![]),
        fixture_page(0, 1, vec![instrument.clone(), instrument.clone()]),
        json!({"instruments": []}),
        fixture_page(0, 501, vec![]),
    ] {
        let server = MockServer::start().await;
        let client = fixture_client(&server).await?;
        mount_categories(&server, &["pages"], 2).await;
        mount_page(&server, "pages", 0, response, 2).await;
        assert!(client.get_all_markets().await.is_err());
        assert!(client.get_vec_db_entries().await.is_err());
    }
    Ok(())
}

#[tokio::test]
async fn test_public_catalog_changing_effective_page_size_is_an_error() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    mount_categories(&server, &["pages"], 2).await;
    mount_page(
        &server,
        "pages",
        0,
        fixture_page(
            0,
            1,
            vec![fixture_instrument("IX.D.FIXTURE.CASH.IP", "DFB", "INDICES")],
        ),
        2,
    )
    .await;
    mount_page(&server, "pages", 1, fixture_page(1, 2, vec![]), 2).await;
    assert!(client.get_all_markets().await.is_err());
    assert!(client.get_vec_db_entries().await.is_err());
    Ok(())
}

// The 1,000-page safety-limit regression lives in the client's cfg(test)
// module. It exercises both public methods with an injected test-only account
// quota, so it does not spend over an hour on the real 30/minute allowance.

#[tokio::test]
async fn test_public_catalog_blank_category_or_epic_is_an_error() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    mount_categories(&server, &[" "], 2).await;
    assert!(matches!(
        client.get_all_markets().await,
        Err(AppError::CatalogPagination { .. })
    ));
    assert!(matches!(
        client.get_vec_db_entries().await,
        Err(AppError::CatalogPagination { .. })
    ));

    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    mount_categories(&server, &["malformed"], 2).await;
    mount_page(
        &server,
        "malformed",
        0,
        fixture_page(
            0,
            500,
            vec![fixture_instrument("", "DEC-26", "COMMODITIES")],
        ),
        2,
    )
    .await;
    assert!(matches!(
        client.get_all_markets().await,
        Err(AppError::CatalogPagination { .. })
    ));
    assert!(matches!(
        client.get_vec_db_entries().await,
        Err(AppError::CatalogPagination { .. })
    ));
    Ok(())
}

#[tokio::test]
async fn test_public_catalog_expiries_survive_detail_success_and_failure() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    let first_epic = "OP.D.FIXTURE.SEP.IP";
    let second_epic = "OP.D.FIXTURE.DEC.IP";
    mount_categories(&server, &["options"], 2).await;
    let mut first = fixture_instrument(first_epic, "11-SEP-26", "OPT_COMMODITIES");
    first["expiryTimestamp"] = json!(1_789_152_000_000_i64);
    let second = fixture_instrument(second_epic, "DEC-26", "OPT_COMMODITIES");
    mount_page(
        &server,
        "options",
        0,
        fixture_page(0, 500, vec![first, second]),
        2,
    )
    .await;
    mount_page(&server, "options", 1, fixture_page(1, 500, vec![]), 2).await;
    mount_details(
        &server,
        first_epic,
        fixture_details(first_epic, "11-SEP-26", "2026-09-11T16:58:00"),
        1,
    )
    .await;
    mount_failure(&server, &format!("/markets/{second_epic}"), None, 1).await;

    let details = client.get_market_details(first_epic).await?;
    assert_eq!(details.instrument.expiry, "11-SEP-26");
    assert_eq!(
        details
            .instrument
            .expiry_details
            .ok_or("expiry details missing")?
            .last_dealing_date,
        "2026-09-11T16:58:00"
    );
    assert!(client.get_market_details(second_epic).await.is_err());

    let markets = client.get_all_markets().await?;
    let entries = client.get_vec_db_entries().await?;
    for (epic, expiry) in [(first_epic, "11-SEP-26"), (second_epic, "DEC-26")] {
        let market = markets
            .iter()
            .find(|market| market.epic == epic)
            .ok_or("market missing")?;
        let entry = entries
            .iter()
            .find(|entry| entry.epic == epic)
            .ok_or("entry missing")?;
        assert_eq!(market.expiry, expiry);
        assert_eq!(entry.expiry, expiry);
        assert_eq!(entry.symbol, "FIXTURE");
        assert!(entry.last_dealing_date.is_none());
    }
    let first_market = markets
        .iter()
        .find(|market| market.epic == first_epic)
        .ok_or("first market missing")?;
    let first_entry = entries
        .iter()
        .find(|entry| entry.epic == first_epic)
        .ok_or("first entry missing")?;
    assert_eq!(first_market.expiry_timestamp, Some(1_789_152_000_000));
    assert_eq!(first_entry.expiry_timestamp, Some(1_789_152_000_000));
    // Exact detail expectations also prove known expiries require no enrichment.
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn test_public_db_entries_missing_expiry_enrichment_is_per_epic() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    let successful_epic = "OP.D.FIXTURE.MISSING1.IP";
    let failed_epic = "OP.D.FIXTURE.MISSING2.IP";
    let mut successful = fixture_instrument(successful_epic, "", "OPT_COMMODITIES");
    successful["expiryTimestamp"] = json!(1_789_757_000_000_i64);
    let mut failed = fixture_instrument(failed_epic, "", "OPT_COMMODITIES");
    failed["expiryTimestamp"] = json!(1_790_362_000_000_i64);
    mount_categories(&server, &["options"], 1).await;
    mount_page(
        &server,
        "options",
        0,
        fixture_page(
            0,
            500,
            vec![
                successful,
                failed,
                fixture_instrument("OP.D.FIXTURE.SEP.IP", "11-SEP-26", "OPT_COMMODITIES"),
                fixture_instrument("OP.D.FIXTURE.DEC.IP", "DEC-26", "OPT_COMMODITIES"),
            ],
        ),
        1,
    )
    .await;
    mount_page(&server, "options", 1, fixture_page(1, 500, vec![]), 1).await;
    mount_details(
        &server,
        successful_epic,
        fixture_details(successful_epic, "18-SEP-26", "2026-09-18T16:58:00"),
        1,
    )
    .await;
    mount_failure(&server, &format!("/markets/{failed_epic}"), None, 1).await;

    let entries = client.get_vec_db_entries().await?;
    assert_eq!(entries.len(), 4);
    assert!(entries.iter().all(|entry| entry.symbol == "FIXTURE"));
    for (epic, expiry, expiry_timestamp) in [
        (successful_epic, "18-SEP-26", Some(1_789_757_000_000)),
        (failed_epic, "", Some(1_790_362_000_000)),
        ("OP.D.FIXTURE.SEP.IP", "11-SEP-26", None),
        ("OP.D.FIXTURE.DEC.IP", "DEC-26", None),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.epic == epic)
            .ok_or("entry missing")?;
        assert_eq!(entry.expiry, expiry);
        assert_eq!(entry.expiry_timestamp, expiry_timestamp);
        if epic == successful_epic {
            assert_eq!(
                entry.last_dealing_date.as_deref(),
                Some("2026-09-18T16:58:00")
            );
        } else {
            assert!(entry.last_dealing_date.is_none());
        }
    }
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn test_public_db_entries_mismatched_detail_epic_does_not_enrich() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    let epic = "OP.D.FIXTURE.MISSING.IP";
    let mut listed = fixture_instrument(epic, " ", "OPT_COMMODITIES");
    listed["expiryTimestamp"] = json!(1_790_967_000_000_i64);
    mount_categories(&server, &["options"], 1).await;
    mount_page(&server, "options", 0, fixture_page(0, 500, vec![listed]), 1).await;
    mount_page(&server, "options", 1, fixture_page(1, 500, vec![]), 1).await;
    mount_details(
        &server,
        epic,
        fixture_details("OP.D.FIXTURE.OTHER.IP", "18-SEP-26", "2026-09-18T16:58:00"),
        1,
    )
    .await;
    let entries = client.get_vec_db_entries().await?;
    let entry = entries.first().ok_or("entry missing")?;
    assert_eq!(entry.epic, epic);
    assert_eq!(entry.expiry, " ");
    assert_eq!(entry.expiry_timestamp, Some(1_790_967_000_000));
    assert!(entry.last_dealing_date.is_none());
    Ok(())
}

#[tokio::test]
async fn test_public_db_entries_last_dealing_date_does_not_fill_missing_expiry() -> TestResult {
    let server = MockServer::start().await;
    let client = fixture_client(&server).await?;
    let epic = "OP.D.FIXTURE.MISSING.IP";
    mount_categories(&server, &["options"], 1).await;
    mount_page(
        &server,
        "options",
        0,
        fixture_page(
            0,
            500,
            vec![fixture_instrument(epic, "", "OPT_COMMODITIES")],
        ),
        1,
    )
    .await;
    mount_page(&server, "options", 1, fixture_page(1, 500, vec![]), 1).await;
    mount_details(
        &server,
        epic,
        fixture_details(epic, "", "2026-09-18T16:58:00"),
        1,
    )
    .await;
    let entries = client.get_vec_db_entries().await?;
    let entry = entries.first().ok_or("entry missing")?;
    assert_eq!(entry.expiry, "");
    assert!(entry.expiry_timestamp.is_none());
    assert_eq!(
        entry.last_dealing_date.as_deref(),
        Some("2026-09-18T16:58:00")
    );
    Ok(())
}
