use ig_client::application::streaming_convert::chart_data_from_item_update;
use ig_client::presentation::chart::{ChartData, ChartFields};
use lightstreamer_rs::subscription::ItemUpdate;
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
    let item_update = ItemUpdate {
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

    let item_update = ItemUpdate {
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
