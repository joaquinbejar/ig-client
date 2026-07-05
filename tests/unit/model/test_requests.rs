use ig_client::constants::{DEFAULT_ORDER_BUY_LEVEL, DEFAULT_ORDER_SELL_LEVEL};
use ig_client::model::requests::{
    ClosePositionRequest, CreateOrderRequest, CreateWorkingOrderRequest, RecentPricesRequest,
};
use ig_client::presentation::order::{Direction, OrderType, TimeInForce};

fn json_value<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).unwrap()
}

#[test]
fn recent_prices_request_builders() {
    let req = RecentPricesRequest::new("CS.D.EURUSD.TODAY.IP")
        .with_resolution("MINUTE")
        .with_from("2025-10-19T10:00:00")
        .with_to("2025-10-19T12:00:00")
        .with_max_points(100)
        .with_page_size(50)
        .with_page_number(2);

    assert_eq!(req.epic, "CS.D.EURUSD.TODAY.IP");
    assert_eq!(req.resolution, Some("MINUTE"));
    assert_eq!(req.from, Some("2025-10-19T10:00:00"));
    assert_eq!(req.to, Some("2025-10-19T12:00:00"));
    assert_eq!(req.max_points, Some(100));
    assert_eq!(req.page_size, Some(50));
    assert_eq!(req.page_number, Some(2));
}

#[test]
fn create_order_market_defaults_and_rounding() {
    let order = CreateOrderRequest::market(
        "CS.D.EURUSD.TODAY.IP".to_string(),
        Direction::Buy,
        1.23456,
        None,
        Some("REF123".to_string()),
    );

    assert_eq!(order.epic, "CS.D.EURUSD.TODAY.IP");
    assert_eq!(order.direction, Direction::Buy);
    // rounded down to 2 decimals
    assert!((order.size - 1.23).abs() < 1e-9);
    assert_eq!(order.order_type, OrderType::Market);
    assert_eq!(order.time_in_force, TimeInForce::FillOrKill);
    assert_eq!(order.level, None);
    assert!(!order.guaranteed_stop);
    assert_eq!(order.expiry.as_deref(), Some("-"));
    assert_eq!(order.deal_reference.as_deref(), Some("REF123"));
    assert!(order.force_open);
    assert_eq!(order.currency_code, "EUR"); // default
    assert_eq!(order.trailing_stop, Some(false));

    // serde field names
    let json = json_value(&order);
    assert_eq!(json.get("orderType").unwrap(), "MARKET");
    assert_eq!(json.get("timeInForce").unwrap(), "FILL_OR_KILL");
    assert_eq!(json.get("currencyCode").unwrap(), "EUR");
}

#[test]
fn create_order_limit_and_chainers() {
    let order = CreateOrderRequest::limit(
        "IX.D.DAX.IFD.IP".to_string(),
        Direction::Sell,
        2.999,
        16000.5,
        Some("USD".to_string()),
        None,
    )
    .with_stop_loss(15900.0)
    .with_take_profit(15000.0)
    .with_trailing_stop_loss(10.0)
    .with_reference("XREF".to_string())
    .with_stop_distance(25.0)
    .with_limit_distance(50.0)
    .with_guaranteed_stop(true);

    assert_eq!(order.order_type, OrderType::Limit);
    assert_eq!(order.time_in_force, TimeInForce::GoodTillCancelled);
    assert_eq!(order.level, Some(16000.5));
    assert_eq!(order.currency_code, "USD");
    // rounded to nearest two decimals: 2.999 -> 3.00
    assert!((order.size - 3.0).abs() < 1e-9);

    assert_eq!(order.stop_level, Some(15900.0));
    assert_eq!(order.limit_level, Some(15000.0));
    assert_eq!(order.trailing_stop, Some(true));
    assert_eq!(order.trailing_stop_increment, Some(10.0));
    assert_eq!(order.deal_reference.as_deref(), Some("XREF"));
    assert_eq!(order.stop_distance, Some(25.0));
    assert_eq!(order.limit_distance, Some(50.0));
    assert!(order.guaranteed_stop);
}

#[test]
fn create_option_helpers_sell_and_buy_default_levels() {
    let sell = CreateOrderRequest::sell_option_to_market(
        "OP.D.FUT.TEST.IP".to_string(),
        5.555,
        Some("-".to_string()),
        None,
        None,
    );
    assert_eq!(sell.direction, Direction::Sell);
    assert_eq!(sell.order_type, OrderType::Limit);
    assert_eq!(sell.level, Some(DEFAULT_ORDER_SELL_LEVEL));
    // rounded to nearest two decimals: 5.555 -> 5.56
    assert!((sell.size - 5.56).abs() < 1e-9);
    assert_eq!(sell.currency_code, "EUR");
    assert!(sell.deal_reference.is_some()); // auto generated when None

    let buy = CreateOrderRequest::buy_option_to_market(
        "OP.D.FUT.TEST.IP".to_string(),
        std::f64::consts::PI,
        None,
        None,
        Some("GBP".to_string()),
    );
    assert_eq!(buy.direction, Direction::Buy);
    assert_eq!(buy.level, Some(DEFAULT_ORDER_BUY_LEVEL));
    #[allow(clippy::approx_constant)]
    {
        assert!((buy.size - 3.14).abs() < 1e-9);
    }
    assert_eq!(buy.currency_code, "GBP");
}

#[test]
fn create_option_helpers_with_force_open_flag() {
    let sell = CreateOrderRequest::sell_option_to_market_w_force(
        "OP.D.FUT.TEST.IP".to_string(),
        1.111,
        None,
        Some("DR1".to_string()),
        Some("USD".to_string()),
        false,
    );
    assert!(!sell.force_open);
    assert_eq!(sell.deal_reference.as_deref(), Some("DR1"));
    assert_eq!(sell.currency_code, "USD");

    let buy = CreateOrderRequest::buy_option_to_market_w_force(
        "OP.D.FUT.TEST.IP".to_string(),
        2.222,
        None,
        Some("DR2".to_string()),
        None,
        true,
    );
    assert!(buy.force_open);
    assert_eq!(buy.deal_reference.as_deref(), Some("DR2"));
    assert_eq!(buy.currency_code, "EUR"); // default when None
}

#[test]
fn close_position_requests() {
    let mkt = ClosePositionRequest::market("DID1".to_string(), Direction::Sell, 1.0);
    assert_eq!(mkt.deal_id.as_deref(), Some("DID1"));
    assert_eq!(mkt.order_type, OrderType::Market);
    assert_eq!(mkt.time_in_force, TimeInForce::FillOrKill);
    assert_eq!(mkt.level, None);

    let lim = ClosePositionRequest::limit("DID2".to_string(), Direction::Buy, 2.0, 123.45);
    assert_eq!(lim.level, Some(123.45));
    assert_eq!(lim.order_type, OrderType::Limit);

    let opt_id =
        ClosePositionRequest::close_option_to_market_by_id("DID3".to_string(), Direction::Buy, 3.0);
    assert_eq!(opt_id.level, Some(DEFAULT_ORDER_BUY_LEVEL));

    let opt_epic = ClosePositionRequest::close_option_to_market_by_epic(
        "EPIC1".to_string(),
        "-".to_string(),
        Direction::Sell,
        4.0,
    );
    assert_eq!(opt_epic.level, Some(DEFAULT_ORDER_SELL_LEVEL));
    assert_eq!(opt_epic.epic.as_deref(), Some("EPIC1"));
    assert_eq!(opt_epic.expiry.as_deref(), Some("-"));
}

#[test]
fn create_working_order_builders() {
    let wo = CreateWorkingOrderRequest::limit(
        "IX.D.DAX.IFD.IP".to_string(),
        Direction::Buy,
        1.0,
        17000.0,
        "EUR".to_string(),
        "24-OCT-25".to_string(),
    )
    .with_stop_loss(16900.0)
    .with_take_profit(18000.0)
    .with_reference("WO1".to_string())
    .expires_at("2025-12-31".to_string());

    assert_eq!(wo.order_type, OrderType::Limit);
    assert_eq!(wo.time_in_force, TimeInForce::GoodTillDate);
    assert_eq!(wo.stop_level, Some(16900.0));
    assert_eq!(wo.limit_level, Some(18000.0));
    assert_eq!(wo.deal_reference.as_deref(), Some("WO1"));
    assert_eq!(wo.good_till_date.as_deref(), Some("2025-12-31"));

    let ws = CreateWorkingOrderRequest::stop(
        "IX.D.DAX.IFD.IP".to_string(),
        Direction::Sell,
        2.0,
        16500.0,
        "EUR".to_string(),
        "24-OCT-25".to_string(),
    );
    assert_eq!(ws.order_type, OrderType::Stop);
    assert_eq!(ws.time_in_force, TimeInForce::GoodTillCancelled);
}

/// Regression for the floor-rounding bug: `(size * 100.0).floor() / 100.0`
/// silently shrank sizes (0.29 -> 0.28) because `0.29 * 100.0` is
/// `28.999999999999996` in f64. Every `CreateOrderRequest` constructor must send
/// the requested size back unchanged for values with a clean two-decimal form.
#[test]
fn create_order_constructors_preserve_two_decimal_sizes() {
    // Values whose `* 100.0` product falls just below an integer in f64, which
    // `floor` truncated downward.
    let sizes = [0.29_f64, 0.57, 0.58, 1.13, 2.29, 10.57];

    for &s in &sizes {
        let expect = |got: f64| {
            assert!(
                (got - s).abs() < 1e-9,
                "size {s} was corrupted to {got} by the constructor"
            );
        };

        expect(CreateOrderRequest::market("EPIC".to_string(), Direction::Buy, s, None, None).size);
        expect(
            CreateOrderRequest::limit("EPIC".to_string(), Direction::Buy, s, 100.0, None, None)
                .size,
        );
        expect(
            CreateOrderRequest::sell_option_to_market("EPIC".to_string(), s, None, None, None).size,
        );
        expect(
            CreateOrderRequest::sell_option_to_market_w_force(
                "EPIC".to_string(),
                s,
                None,
                None,
                None,
                true,
            )
            .size,
        );
        expect(
            CreateOrderRequest::buy_option_to_market("EPIC".to_string(), s, None, None, None).size,
        );
        expect(
            CreateOrderRequest::buy_option_to_market_w_force(
                "EPIC".to_string(),
                s,
                None,
                None,
                None,
                true,
            )
            .size,
        );
    }
}
