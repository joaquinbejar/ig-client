use ig_client::application::streaming_convert::trade_data_from_item_update;
use ig_client::presentation::trade::{TradeData, TradeFields};
use lightstreamer_rs::subscription::ItemUpdate;
use std::collections::HashMap;

#[test]
fn test_trade_fields_default() {
    let fields = TradeFields::default();

    assert_eq!(fields.confirms, None);
    // opu and wou don't implement PartialEq, so we can't compare them
    assert!(fields.opu.is_none());
    assert!(fields.wou.is_none());
}

#[test]
fn test_trade_data_display() {
    let trade = TradeData {
        item_name: "TRADE:TEST".to_string(),
        item_pos: 1,
        fields: TradeFields::default(),
        changed_fields: TradeFields::default(),
        is_snapshot: false,
    };

    let display = format!("{}", trade);
    assert!(display.contains("TRADE:TEST") || display.contains("item_name"));
}

#[test]
fn test_trade_data_from_item_update_empty() {
    let item_update = ItemUpdate {
        item_name: Some("TRADE:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields: HashMap::new(),
        changed_fields: HashMap::new(),
    };

    let result = trade_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_trade_data_from_item_update_with_confirms() {
    let mut fields = HashMap::new();
    fields.insert("CONFIRMS".to_string(), Some("DEAL123".to_string()));

    let item_update = ItemUpdate {
        item_name: Some("TRADE:TEST".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = trade_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_trade_data_clone() {
    let trade = TradeData {
        item_name: "TRADE:TEST".to_string(),
        item_pos: 1,
        fields: TradeFields::default(),
        changed_fields: TradeFields::default(),
        is_snapshot: false,
    };

    let cloned = trade.clone();
    assert_eq!(trade.item_name, cloned.item_name);
    assert_eq!(trade.item_pos, cloned.item_pos);
    assert_eq!(trade.is_snapshot, cloned.is_snapshot);
}
