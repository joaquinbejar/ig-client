use ig_client::application::streaming_convert::StreamingUpdate;
use ig_client::application::streaming_convert::chart_data_from_item_update;
use ig_client::presentation::chart::{ChartData, ChartFields};
use std::collections::HashMap;

#[test]
fn test_chart_fields_default() {
    let fields = ChartFields::default();
    let _json = serde_json::to_string(&fields).unwrap();
}

#[test]
fn test_chart_data_default() {
    let chart = ChartData::default();
    let _display = format!("{}", chart);
}

#[test]
fn test_chart_data_from_item_update_empty() {
    let item_update = StreamingUpdate {
        item_name: Some("CHART:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields: HashMap::new(),
        changed_fields: HashMap::new(),
    };

    let result = chart_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_chart_data_from_item_update_with_fields() {
    let mut fields = HashMap::new();
    fields.insert("BID".to_string(), Some("100.5".to_string()));
    fields.insert("OFFER".to_string(), Some("101.0".to_string()));
    fields.insert("HIGH".to_string(), Some("105.0".to_string()));
    fields.insert("LOW".to_string(), Some("95.0".to_string()));
    fields.insert("LTV".to_string(), Some("1000".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("CHART:TEST".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = chart_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_chart_data_clone() {
    let chart = ChartData::default();
    let _cloned = chart.clone();
}

#[test]
fn test_chart_data_serialization() {
    let chart = ChartData::default();
    let json = serde_json::to_string(&chart).unwrap();
    let _deserialized: ChartData = serde_json::from_str(&json).unwrap();
}

#[test]
fn test_chart_data_from_item_update_cons_end_is_flag() {
    // CONS_END is a 0/1 candle-completed flag (1 = ended, 0 = forming), not a
    // timestamp.
    let mut fields = HashMap::new();
    fields.insert("CONS_END".to_string(), Some("1".to_string()));
    fields.insert("UTM".to_string(), Some("1700000000123".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("CHART:CS.D.EURUSD.MINI.IP:1MINUTE".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    let data = chart_data_from_item_update(&item_update).expect("chart update should parse");
    assert_eq!(data.fields.candle_end, Some(true));
    // UTM is epoch milliseconds (UTC), parsed as i64.
    assert_eq!(data.fields.update_time, Some(1_700_000_000_123));
}

#[test]
fn test_chart_data_from_item_update_cons_end_zero_is_false() {
    let mut fields = HashMap::new();
    fields.insert("CONS_END".to_string(), Some("0".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("CHART:CS.D.EURUSD.MINI.IP:1MINUTE".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    let data = chart_data_from_item_update(&item_update).expect("chart update should parse");
    assert_eq!(data.fields.candle_end, Some(false));
}

#[test]
fn test_chart_data_from_item_update_cons_end_invalid_is_error() {
    let mut fields = HashMap::new();
    fields.insert("CONS_END".to_string(), Some("2".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("CHART:CS.D.EURUSD.MINI.IP:1MINUTE".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    assert!(
        chart_data_from_item_update(&item_update).is_err(),
        "a non-0/1 CONS_END value must surface an error"
    );
}

#[test]
fn test_chart_fields_round_trip_with_new_types() {
    // Round-trips a sparse candle update through serde, pinning that the
    // retyped fields (`candle_end: bool`, `update_time: i64`) survive.
    let fields = ChartFields {
        bid: Some(1.1000),
        offer: Some(1.1002),
        update_time: Some(1_700_000_000_123),
        candle_end: Some(true),
        candle_tick_count: Some(42.0),
        ..Default::default()
    };

    let json = serde_json::to_string(&fields).expect("serialize should succeed");
    let restored: ChartFields =
        serde_json::from_str(&json).expect("ChartFields must deserialize its own output");
    assert_eq!(restored.bid, Some(1.1000));
    assert_eq!(restored.offer, Some(1.1002));
    assert_eq!(restored.update_time, Some(1_700_000_000_123));
    assert_eq!(restored.candle_end, Some(true));
    assert_eq!(restored.candle_tick_count, Some(42.0));
    assert!(restored.day_high.is_none());
}
