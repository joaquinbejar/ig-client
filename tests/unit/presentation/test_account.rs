use ig_client::application::streaming_convert::StreamingUpdate;
use ig_client::application::streaming_convert::account_data_from_item_update;
use ig_client::presentation::account::{AccountData, AccountFields};
use std::collections::HashMap;

#[test]
fn test_account_fields_default() {
    let fields = AccountFields::default();
    let _json = serde_json::to_string(&fields).unwrap();
}

#[test]
fn test_account_data_default() {
    let account = AccountData::default();
    let _display = format!("{}", account);
}

#[test]
fn test_account_data_from_item_update_empty() {
    let item_update = StreamingUpdate {
        item_name: Some("ACCOUNT:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields: HashMap::new(),
        changed_fields: HashMap::new(),
    };

    let result = account_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_account_data_from_item_update_with_fields() {
    let mut fields = HashMap::new();
    fields.insert("PNL".to_string(), Some("1000.50".to_string()));
    fields.insert("DEPOSIT".to_string(), Some("10000.00".to_string()));
    fields.insert("USED_MARGIN".to_string(), Some("500.00".to_string()));
    fields.insert("AVAILABLE_CASH".to_string(), Some("9500.00".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("ACCOUNT:TEST".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = account_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_account_data_clone() {
    let account = AccountData::default();
    let _cloned = account.clone();
}
