use chrono::{DateTime, TimeZone, Utc};
use ig_client::presentation::instrument::InstrumentType;
use ig_client::presentation::market::MarketData;
use ig_client::storage::market_database::MarketDatabaseService;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;

fn make_service() -> MarketDatabaseService {
    // Use a lazy pool so it doesn't require an actual database connection
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://user:pass@localhost/testdb")
        .expect("connect_lazy should not attempt real connection");
    MarketDatabaseService::new(pool, "IG".to_string())
}

#[tokio::test]
async fn is_valid_epic_format_checks_dot_count() {
    let svc = make_service();

    // Exactly 4 dots => valid
    assert!(svc.is_valid_epic_format("CS.D.EURUSD.TODAY.IP"));
    assert!(svc.is_valid_epic_format("IX.D.DAX.DAILY.IP"));

    // Too few / too many dots => invalid
    assert!(!svc.is_valid_epic_format("EURUSD"));
    assert!(!svc.is_valid_epic_format("A.B.C"));
    assert!(!svc.is_valid_epic_format("A.B.C.D.E.F"));
}

#[tokio::test]
async fn find_symbol_for_market_picks_first_matching_key_or_unknown() {
    let svc = make_service();

    let mut map: HashMap<&str, &str> = HashMap::new();
    map.insert("germany 40", "DAX");
    map.insert("eur/usd", "EURUSD");

    // Matching is case-insensitive and substring-based
    assert_eq!(svc.find_symbol_for_market("Germany 40 Cash", &map), "DAX");
    assert_eq!(
        svc.find_symbol_for_market("Spot EUR/USD Pair", &map),
        "EURUSD"
    );

    // No match => UNKNOWN
    assert_eq!(
        svc.find_symbol_for_market("Unmapped Instrument", &map),
        "UNKNOWN"
    );
}

#[tokio::test]
async fn convert_update_time_handles_valid_invalid_and_none() {
    let svc = make_service();

    // 1 Jan 2025 00:00:00.123 UTC in milliseconds
    let ms: i64 = 1735689600123; // 2025-01-01T00:00:00.123Z
    let dt = svc.convert_update_time(&Some(ms.to_string())).unwrap();
    let expected: DateTime<Utc> = Utc
        .timestamp_opt(ms / 1000, ((ms % 1000) as u32) * 1_000_000)
        .single()
        .unwrap();
    assert_eq!(dt, expected);

    // Invalid number => None
    assert!(
        svc.convert_update_time(&Some("not-a-number".into()))
            .is_none()
    );

    // None => None
    assert!(svc.convert_update_time(&None).is_none());
}

#[tokio::test]
async fn convert_market_data_to_instrument_maps_fields() {
    let svc = make_service();

    let md = MarketData {
        epic: "IX.D.DAX.IFD.IP".into(),
        instrument_name: "Germany 40 Cash".into(),
        instrument_type: InstrumentType::Indices,
        expiry: "-".into(),
        high_limit_price: Some(20000.0),
        low_limit_price: Some(10000.0),
        market_status: "TRADEABLE".into(),
        net_change: Some(12.3),
        percentage_change: Some(0.5),
        update_time: Some("1735689600123".into()),
        update_time_utc: Some("2025-01-01T00:00:00.123Z".into()),
        bid: Some(18000.5),
        offer: Some(18001.5),
    };

    let inst = svc.convert_market_data_to_instrument(&md, "node-1");

    assert_eq!(inst.epic, md.epic);
    assert_eq!(inst.instrument_name, md.instrument_name);
    // The serde wire value is persisted without surrounding quotes.
    assert_eq!(inst.instrument_type, "INDICES");
    assert!(!inst.instrument_type.contains('"'));
    assert_eq!(inst.node_id, "node-1");
    assert_eq!(inst.exchange, "IG");
    assert_eq!(inst.expiry, md.expiry);
    assert_eq!(inst.high_limit_price, md.high_limit_price);
    assert_eq!(inst.low_limit_price, md.low_limit_price);
    assert_eq!(inst.market_status, md.market_status);
    assert_eq!(inst.net_change, md.net_change);
    assert_eq!(inst.percentage_change, md.percentage_change);
    assert_eq!(inst.update_time, md.update_time);
    assert_eq!(inst.bid, md.bid);
    assert_eq!(inst.offer, md.offer);
}
