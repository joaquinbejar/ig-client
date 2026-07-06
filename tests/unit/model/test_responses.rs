use chrono::{Duration, Utc};
use ig_client::model::responses::*;
use ig_client::presentation::account::{Position, PositionDetails, PositionMarket};
use ig_client::presentation::instrument::InstrumentType;
use ig_client::presentation::market::*;
use ig_client::presentation::order::{Direction, Status};

fn json_value<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).unwrap()
}

#[test]
fn dbentry_from_marketnode_and_marketdata() {
    // Build basic MarketData
    let md = MarketData {
        epic: "CS.D.EURUSD.TODAY.IP".to_string(),
        instrument_name: "EUR/USD".to_string(),
        instrument_type: InstrumentType::Currencies,
        expiry: "-".to_string(),
        high_limit_price: None,
        low_limit_price: None,
        market_status: "TRADEABLE".to_string(),
        net_change: Some(0.12),
        percentage_change: Some(0.01),
        update_time: Some("12:00".to_string()),
        update_time_utc: Some("11:00".to_string()),
        bid: Some(1.1),
        offer: Some(1.2),
    };

    // From MarketData
    let entry_from_md: DBEntryResponse = DBEntryResponse::from(md.clone());
    assert_eq!(entry_from_md.epic, md.epic);
    assert_eq!(entry_from_md.name, md.instrument_name);
    assert_eq!(entry_from_md.instrument_type, md.instrument_type);
    assert_eq!(entry_from_md.exchange, "IG");
    assert_eq!(entry_from_md.expiry, md.expiry);
    assert_eq!(entry_from_md.symbol, "EURUSD");
    // last_update is now-ish
    let now = Utc::now();
    assert!(entry_from_md.last_update <= now);
    assert!(entry_from_md.last_update >= now - Duration::seconds(10));

    // From &MarketData
    let entry_from_md_ref: DBEntryResponse = DBEntryResponse::from(&md);
    assert_eq!(entry_from_md_ref.epic, md.epic);

    // From MarketNode (takes first market)
    let node = MarketNode {
        id: "root".into(),
        name: "Root".into(),
        children: vec![],
        markets: vec![md.clone()],
    };
    let entry_from_node: DBEntryResponse = DBEntryResponse::from(node.clone());
    assert_eq!(entry_from_node.epic, md.epic);
    // From &MarketNode
    let entry_from_node_ref: DBEntryResponse = DBEntryResponse::from(&node);
    assert_eq!(entry_from_node_ref.name, md.instrument_name);
}

#[test]
fn multiple_market_details_response_helpers_and_display() {
    let instrument = Instrument {
        epic: "IX.D.DAX.IFD.IP".into(),
        name: "Germany 40 Cash".into(),
        expiry: "-".into(),
        contract_size: "10".into(),
        lot_size: Some(1.0),
        high_limit_price: None,
        low_limit_price: None,
        margin_factor: None,
        margin_factor_unit: None,
        currencies: None,
        value_of_one_pip: "1".into(),
        instrument_type: Some(InstrumentType::Indices),
        expiry_details: None,
        slippage_factor: None,
        limited_risk_premium: None,
        news_code: None,
        chart_code: None,
    };
    let snapshot = MarketSnapshot {
        market_status: "TRADEABLE".into(),
        net_change: Some(10.0),
        percentage_change: Some(0.5),
        update_time: Some("12:34".into()),
        delay_time: Some(0),
        bid: Some(18000.0),
        offer: Some(18001.0),
        high: Some(18100.0),
        low: Some(17900.0),
        binary_odds: None,
        decimal_places_factor: Some(2),
        scaling_factor: Some(1),
        controlled_risk_extra_spread: None,
    };
    let dealing_rules = DealingRules {
        min_step_distance: Some(StepDistance {
            unit: Some(StepUnit::Points),
            value: Some(1.0),
        }),
        min_deal_size: Some(StepDistance {
            unit: Some(StepUnit::Points),
            value: Some(1.0),
        }),
        min_controlled_risk_stop_distance: Some(StepDistance {
            unit: Some(StepUnit::Points),
            value: Some(5.0),
        }),
        min_normal_stop_or_limit_distance: Some(StepDistance {
            unit: Some(StepUnit::Points),
            value: Some(1.0),
        }),
        max_stop_or_limit_distance: Some(StepDistance {
            unit: Some(StepUnit::Points),
            value: Some(1000.0),
        }),
        controlled_risk_spacing: Some(StepDistance {
            unit: Some(StepUnit::Points),
            value: Some(1.0),
        }),
        market_order_preference: "AVAILABLE_DEFAULT_OFF".into(),
        trailing_stops_preference: "AVAILABLE_DEFAULT_OFF".into(),
        max_deal_size: Some(100.0),
    };
    let details = MarketDetails {
        instrument,
        snapshot,
        dealing_rules,
    };
    let resp = MultipleMarketDetailsResponse {
        market_details: vec![details.clone()],
    };

    assert_eq!(resp.len(), 1);
    assert!(!resp.is_empty());
    assert_eq!(resp.market_details().len(), 1);
    assert_eq!(resp.iter().count(), 1);

    let s = format!("{}", resp);
    assert!(s.contains("INSTRUMENT NAME"));
    assert!(s.contains("EPIC"));
    assert!(s.contains("HIGH/LOW"));
    assert!(s.contains("Germany 40 Cash"));
    assert!(s.contains("IX.D.DAX.IFD.IP"));
}

#[test]
fn historical_prices_response_helpers_and_display() {
    let p1 = HistoricalPrice {
        snapshot_time: "2025-10-19T10:00:00".into(),
        snapshot_time_utc: Some("2025-10-19T09:00:00".into()),
        open_price: PricePoint {
            bid: Some(1.1234),
            ask: Some(1.1236),
            last_traded: None,
        },
        high_price: PricePoint {
            bid: Some(1.1240),
            ask: Some(1.1242),
            last_traded: None,
        },
        low_price: PricePoint {
            bid: Some(1.1220),
            ask: Some(1.1222),
            last_traded: None,
        },
        close_price: PricePoint {
            bid: Some(1.1230),
            ask: Some(1.1232),
            last_traded: None,
        },
        last_traded_volume: Some(100),
    };
    let p2 = HistoricalPrice {
        snapshot_time: "2025-10-19T10:01:00".into(),
        snapshot_time_utc: None,
        open_price: PricePoint {
            bid: Some(1.2234),
            ask: Some(1.2236),
            last_traded: None,
        },
        high_price: PricePoint {
            bid: Some(1.2240),
            ask: Some(1.2242),
            last_traded: None,
        },
        low_price: PricePoint {
            bid: Some(1.2220),
            ask: Some(1.2222),
            last_traded: None,
        },
        close_price: PricePoint {
            bid: Some(1.2230),
            ask: Some(1.2232),
            last_traded: None,
        },
        last_traded_volume: None,
    };
    let resp = HistoricalPricesResponse {
        prices: vec![p1.clone(), p2.clone()],
        instrument_type: InstrumentType::Currencies,
        allowance: Some(PriceAllowance {
            remaining_allowance: 99,
            total_allowance: 1000,
            allowance_expiry: 60,
        }),
    };

    assert_eq!(resp.len(), 2);
    assert!(!resp.is_empty());
    assert_eq!(resp.prices().len(), 2);
    assert_eq!(resp.iter().count(), 2);

    let s = format!("{}", resp);
    assert!(s.contains("SNAPSHOT TIME"));
    assert!(s.contains("OPEN BID"));
    assert!(s.contains("1.1234"));
    assert!(s.contains("1.2232"));
    assert!(s.contains("Total price points: 2"));
    assert!(s.contains("Instrument type:"));
    assert!(s.contains("CURRENCIES"));
    assert!(s.contains("Remaining allowance: 99"));
}

#[test]
fn market_search_response_helpers_and_display() {
    let m1 = MarketData {
        epic: "IX.D.DAX.IFD.IP".into(),
        instrument_name: "Germany 40 Cash".into(),
        instrument_type: InstrumentType::Indices,
        expiry: "-".into(),
        high_limit_price: None,
        low_limit_price: None,
        market_status: "TRADEABLE".into(),
        net_change: Some(10.0),
        percentage_change: Some(0.5),
        update_time: Some("12:34".into()),
        update_time_utc: Some("11:34".into()),
        bid: Some(18000.0),
        offer: Some(18001.0),
    };
    let m2 = MarketData {
        instrument_name: "EUR/USD".into(),
        epic: "CS.D.EURUSD.TODAY.IP".into(),
        instrument_type: InstrumentType::Currencies,
        expiry: "-".into(),
        high_limit_price: None,
        low_limit_price: None,
        market_status: "TRADEABLE".into(),
        net_change: Some(0.1),
        percentage_change: Some(0.01),
        update_time: Some("12:35".into()),
        update_time_utc: Some("11:35".into()),
        bid: Some(1.1),
        offer: Some(1.2),
    };
    let resp = MarketSearchResponse {
        markets: vec![m1, m2],
    };

    assert_eq!(resp.len(), 2);
    assert!(!resp.is_empty());
    assert_eq!(resp.markets().len(), 2);
    assert_eq!(resp.iter().count(), 2);

    let s = format!("{}", resp);
    assert!(s.contains("INSTRUMENT NAME"));
    assert!(s.contains("EPIC"));
    assert!(s.contains("TYPE"));
    assert!(s.contains("Total markets found: 2"));
}

#[test]
fn market_navigation_response_deserializes_null_as_empty() {
    let json = r#"{
        "nodes": null,
        "markets": null
    }"#;
    let resp: MarketNavigationResponse = serde_json::from_str(json).unwrap();
    assert!(resp.nodes.is_empty());
    assert!(resp.markets.is_empty());
}

#[test]
fn positions_response_compact_by_epic_merges_positions() {
    // Two positions for the same epic with opposite directions
    // Build a complete PositionMarket as the struct has many required fields
    let pm = PositionMarket {
        instrument_name: "Germany 40".into(),
        expiry: "-".into(),
        epic: "IX.D.DAX.IFD.IP".into(),
        instrument_type: "INDEX".into(),
        lot_size: 1.0,
        high: Some(100.0),
        low: Some(90.0),
        percentage_change: 0.0,
        net_change: 0.0,
        bid: Some(95.0),
        offer: Some(96.0),
        update_time: "10:00:00".into(),
        update_time_utc: "08:00:00".into(),
        delay_time: 0,
        streaming_prices_available: true,
        market_status: "OPEN".into(),
        scaling_factor: 1,
    };

    let pos_a = Position {
        position: PositionDetails {
            contract_size: 10.0,
            created_date: "2025-10-19T10:00:00".into(),
            created_date_utc: "2025-10-19T08:00:00Z".into(),
            deal_id: "D1".into(),
            deal_reference: "R1".into(),
            direction: Direction::Buy,
            limit_level: Some(100.0),
            level: 50.0,
            size: 5.0,
            stop_level: None,
            trailing_step: None,
            trailing_stop_distance: None,
            currency: "EUR".into(),
            controlled_risk: false,
            limited_risk_premium: None,
        },
        market: pm.clone(),
        pnl: Some(10.0),
    };

    let pos_b = Position {
        position: PositionDetails {
            contract_size: 6.0,
            created_date: "2025-10-19T10:05:00".into(),
            created_date_utc: "2025-10-19T08:05:00Z".into(),
            deal_id: "D2".into(),
            deal_reference: "R2".into(),
            direction: Direction::Sell,
            limit_level: None,
            level: 60.0,
            size: 3.0,
            stop_level: Some(90.0),
            trailing_step: None,
            trailing_stop_distance: None,
            currency: "EUR".into(),
            controlled_risk: false,
            limited_risk_premium: None,
        },
        market: pm.clone(),
        pnl: Some(-4.0),
    };

    let merged = PositionsResponse::compact_by_epic(vec![pos_a, pos_b]);
    assert_eq!(merged.len(), 1);
    let m = &merged[0];
    assert_eq!(m.market.epic, "IX.D.DAX.IFD.IP");
    // Opposite directions => abs differences
    assert_eq!(m.position.contract_size, 4.0);
    assert_eq!(m.position.size, 2.0);
    // Level averaged
    assert!((m.position.level - 55.0).abs() < 1e-9);
    // PnL added
    assert_eq!(m.pnl, Some(6.0));
}

#[test]
fn order_confirmation_response_deserialize_status_and_fields() {
    // Status can be null -> should become default (Open)
    let json_null = r#"{
        "date": "2025-10-19T10:00:00",
        "status": null,
        "reason": null,
        "dealId": null,
        "dealReference": "REF123",
        "dealStatus": null,
        "epic": null,
        "expiry": null,
        "guaranteedStop": null,
        "level": 1.234,
        "limitDistance": null,
        "limitLevel": null,
        "size": 1.0,
        "stopDistance": null,
        "stopLevel": null,
        "trailingStop": null,
        "direction": "BUY"
    }"#;
    let r1: OrderConfirmationResponse = serde_json::from_str(json_null).unwrap();
    assert_eq!(r1.status, Status::Open);
    assert_eq!(r1.deal_reference, "REF123");
    assert_eq!(r1.direction, Some(Direction::Buy));

    // Non-null status maps correctly
    let json_ok = r#"{
        "date": "2025-10-19T10:00:00",
        "status": "ACCEPTED",
        "reason": null,
        "dealId": "D1",
        "dealReference": "R1",
        "dealStatus": "ACCEPTED",
        "epic": "CS.D.EURUSD.TODAY.IP",
        "expiry": "-",
        "guaranteedStop": false,
        "level": 1.0,
        "limitDistance": null,
        "limitLevel": null,
        "size": 1.0,
        "stopDistance": null,
        "stopLevel": null,
        "trailingStop": false,
        "direction": "SELL"
    }"#;
    let r2: OrderConfirmationResponse = serde_json::from_str(json_ok).unwrap();
    assert_eq!(r2.status, Status::Accepted);
    assert_eq!(r2.direction, Some(Direction::Sell));
}

#[test]
fn simple_deal_reference_responses_serde_field_names() {
    let c = CreateOrderResponse {
        deal_reference: "ABC".into(),
    };
    let j = json_value(&c);
    assert_eq!(j.get("dealReference").unwrap(), "ABC");

    let u = UpdatePositionResponse {
        deal_reference: "U1".into(),
    };
    let j = json_value(&u);
    assert_eq!(j.get("dealReference").unwrap(), "U1");

    let w = CreateWorkingOrderResponse {
        deal_reference: "W1".into(),
    };
    let j = json_value(&w);
    assert_eq!(j.get("dealReference").unwrap(), "W1");

    let x = ClosePositionResponse {
        deal_reference: "X1".into(),
    };
    let j = json_value(&x);
    assert_eq!(j.get("dealReference").unwrap(), "X1");
}
