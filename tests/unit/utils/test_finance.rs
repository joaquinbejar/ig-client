use ig_client::presentation::account::{Position, PositionDetails, PositionMarket};
use ig_client::presentation::instrument::InstrumentType;
use ig_client::presentation::market::MarketState;
use ig_client::presentation::order::Direction;
use ig_client::utils::finance::{calculate_percentage_return, calculate_pnl};

fn create_test_position(
    direction: Direction,
    level: f64,
    size: f64,
    bid: Option<f64>,
    offer: Option<f64>,
) -> Position {
    let market = PositionMarket {
        instrument_name: "Test Instrument".into(),
        expiry: "-".into(),
        epic: "TEST.EPIC".into(),
        instrument_type: InstrumentType::Shares,
        lot_size: 1.0,
        high: None,
        low: None,
        percentage_change: 0.0,
        net_change: 0.0,
        bid,
        offer,
        update_time: "00:00:00".into(),
        update_time_utc: "2024-01-01T00:00:00Z".into(),
        delay_time: 0,
        streaming_prices_available: true,
        market_status: MarketState::Tradeable,
        scaling_factor: 1,
    };

    let details = PositionDetails {
        contract_size: 1.0,
        created_date: "2024-01-01T00:00:00".into(),
        created_date_utc: "2024-01-01T00:00:00Z".into(),
        deal_id: "D1".into(),
        deal_reference: "R1".into(),
        direction,
        limit_level: None,
        level,
        size,
        stop_level: None,
        trailing_step: None,
        trailing_stop_distance: None,
        currency: "USD".into(),
        controlled_risk: false,
        limited_risk_premium: None,
    };

    Position {
        position: details,
        market,
        pnl: None,
    }
}

#[test]
fn test_calculate_pnl_buy_position_profit() {
    let position = create_test_position(Direction::Buy, 100.0, 10.0, Some(110.0), Some(111.0));
    let pnl = calculate_pnl(&position);
    assert_eq!(pnl, Some(100.0));
}

#[test]
fn test_calculate_pnl_sell_position_profit() {
    let position = create_test_position(Direction::Sell, 100.0, 10.0, Some(89.0), Some(90.0));
    let pnl = calculate_pnl(&position);
    assert_eq!(pnl, Some(100.0));
}

#[test]
fn test_calculate_pnl_missing_prices() {
    // Buy needs bid
    let position_buy = create_test_position(Direction::Buy, 100.0, 10.0, None, Some(111.0));
    assert_eq!(calculate_pnl(&position_buy), None);

    // Sell needs offer
    let position_sell = create_test_position(Direction::Sell, 100.0, 10.0, Some(89.0), None);
    assert_eq!(calculate_pnl(&position_sell), None);
}

#[test]
fn test_calculate_percentage_return() {
    let position = create_test_position(Direction::Buy, 100.0, 10.0, Some(110.0), Some(111.0));
    let percentage = calculate_percentage_return(&position);
    assert!((percentage.unwrap() - 10.0).abs() < 1e-9);
}

#[test]
fn test_calculate_percentage_return_zero_initial_value() {
    // size = 0 -> initial value 0
    let position = create_test_position(Direction::Buy, 100.0, 0.0, Some(110.0), Some(111.0));
    assert_eq!(calculate_percentage_return(&position), None);
}
