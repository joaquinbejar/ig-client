use ig_client::presentation::instrument::InstrumentType;
use ig_client::presentation::market::MarketData;

#[test]
fn test_market_data_display() {
    let market = MarketData {
        bid: Some(100.5),
        epic: "TEST.EPIC".to_string(),
        expiry: "DEC-24".to_string(),
        high_limit_price: Some(105.0),
        instrument_name: "Test Instrument".to_string(),
        instrument_type: InstrumentType::Shares,
        low_limit_price: Some(95.0),
        market_status: "TRADEABLE".to_string(),
        net_change: Some(2.5),
        offer: Some(101.0),
        percentage_change: Some(2.5),
        update_time: Some("12:34:56".to_string()),
        update_time_utc: Some("2024-01-01T12:34:56".to_string()),
        ..Default::default()
    };

    let display = format!("{}", market);
    assert!(display.contains("TEST.EPIC"));
    assert!(display.contains("Test Instrument"));
}

#[test]
fn test_market_data_clone() {
    let market = MarketData {
        bid: Some(100.5),
        epic: "TEST.EPIC".to_string(),
        expiry: "DEC-24".to_string(),
        high_limit_price: Some(105.0),
        instrument_name: "Test Instrument".to_string(),
        instrument_type: InstrumentType::Shares,
        low_limit_price: Some(95.0),
        market_status: "TRADEABLE".to_string(),
        net_change: Some(2.5),
        offer: Some(101.0),
        percentage_change: Some(2.5),
        update_time: Some("12:34:56".to_string()),
        update_time_utc: Some("2024-01-01T12:34:56".to_string()),
        ..Default::default()
    };

    let cloned = market.clone();
    assert_eq!(market.epic, cloned.epic);
    assert_eq!(market.bid, cloned.bid);
    assert_eq!(market.offer, cloned.offer);
}

#[test]
fn test_market_data_serialization() {
    let market = MarketData {
        bid: Some(100.5),
        epic: "TEST.EPIC".to_string(),
        expiry: "DEC-24".to_string(),
        high_limit_price: Some(105.0),
        instrument_name: "Test Instrument".to_string(),
        instrument_type: InstrumentType::Shares,
        low_limit_price: Some(95.0),
        market_status: "TRADEABLE".to_string(),
        net_change: Some(2.5),
        offer: Some(101.0),
        percentage_change: Some(2.5),
        update_time: Some("12:34:56".to_string()),
        update_time_utc: Some("2024-01-01T12:34:56".to_string()),
        ..Default::default()
    };

    let json = serde_json::to_string(&market).unwrap();
    let deserialized: MarketData = serde_json::from_str(&json).unwrap();

    assert_eq!(market.epic, deserialized.epic);
    assert_eq!(market.bid, deserialized.bid);
}
