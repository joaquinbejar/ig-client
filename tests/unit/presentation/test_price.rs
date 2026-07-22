use ig_client::application::streaming_convert::StreamingUpdate;
use ig_client::application::streaming_convert::price_data_from_item_update;
use ig_client::presentation::price::{DealingFlag, PriceData, PriceFields};
use std::collections::HashMap;

#[test]
fn test_dealing_flag_default() {
    let flag = DealingFlag::default();
    assert_eq!(flag, DealingFlag::Closed);
}

#[test]
fn test_dealing_flag_copy() {
    let flag = DealingFlag::Deal;
    let copied = flag; // Copy trait
    assert_eq!(flag, copied);
}

#[test]
fn test_dealing_flag_serialization() -> Result<(), serde_json::Error> {
    let flag = DealingFlag::Deal;
    let json = serde_json::to_string(&flag)?;
    let deserialized: DealingFlag = serde_json::from_str(&json)?;
    assert_eq!(flag, deserialized);
    Ok(())
}

#[test]
fn test_dealing_flag_all_variants() {
    let flags = vec![
        DealingFlag::Closed,
        DealingFlag::Call,
        DealingFlag::Deal,
        DealingFlag::Edit,
        DealingFlag::ClosingsOnly,
        DealingFlag::DealNoEdit,
        DealingFlag::Auction,
        DealingFlag::AuctionNoEdit,
        DealingFlag::Suspend,
    ];

    for flag in flags {
        let json = serde_json::to_string(&flag).unwrap();
        let _deserialized: DealingFlag = serde_json::from_str(&json).unwrap();
    }
}

#[test]
fn test_price_fields_default() {
    let fields = PriceFields::default();
    let _json = serde_json::to_string(&fields).unwrap();
}

#[test]
fn test_price_data_default() {
    let price = PriceData::default();
    assert_eq!(price.item_name, "");
    assert_eq!(price.item_pos, 0);
    assert!(!price.is_snapshot);
}

#[test]
fn test_price_data_display() {
    let price = PriceData {
        item_name: "MARKET:TEST".to_string(),
        item_pos: 1,
        fields: PriceFields::default(),
        changed_fields: PriceFields::default(),
        is_snapshot: false,
    };

    let display = format!("{}", price);
    assert!(!display.is_empty());
}

#[test]
fn test_price_data_from_item_update_empty() {
    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields: HashMap::new(),
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_price_data_from_item_update_with_bid_offer() {
    let mut fields = HashMap::new();
    fields.insert("BID".to_string(), Some("100.5".to_string()));
    fields.insert("OFFER".to_string(), Some("101.0".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields: fields.clone(),
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());

    let price_data = result.unwrap();
    let json = serde_json::to_string(&price_data).unwrap();
    assert!(json.contains("MARKET:TEST"));
}

#[test]
fn test_price_data_from_item_update_with_all_fields() {
    let mut fields = HashMap::new();
    fields.insert("BID".to_string(), Some("100.5".to_string()));
    fields.insert("OFFER".to_string(), Some("101.0".to_string()));
    fields.insert("HIGH".to_string(), Some("105.0".to_string()));
    fields.insert("LOW".to_string(), Some("95.0".to_string()));
    fields.insert("MID_OPEN".to_string(), Some("100.0".to_string()));
    fields.insert("CHANGE".to_string(), Some("2.5".to_string()));
    fields.insert("CHANGE_PCT".to_string(), Some("2.5".to_string()));
    fields.insert("UPDATE_TIME".to_string(), Some("12:34:56".to_string()));
    fields.insert("MARKET_DELAY".to_string(), Some("0".to_string()));
    fields.insert("MARKET_STATE".to_string(), Some("TRADEABLE".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:FULL".to_string()),
        item_pos: 2,
        is_snapshot: true,
        fields: fields.clone(),
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_price_data_from_item_update_invalid_float() {
    let mut fields = HashMap::new();
    fields.insert("BID".to_string(), Some("invalid".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    // An unparseable float in a numeric field is a hard error: `parse_float`
    // fails and the error propagates out of `from_item_update` as a typed
    // `AppError::Deserialization`. The message names the offending field and
    // value.
    assert!(result.is_err());
    let err = result
        .expect_err("invalid float must yield an error")
        .to_string();
    assert!(
        err.contains("BID"),
        "error should name the offending field, got: {err}"
    );
    assert!(
        err.contains("invalid"),
        "error should include the offending value, got: {err}"
    );
}

#[test]
fn test_price_data_from_item_update_empty_strings() {
    let mut fields = HashMap::new();
    fields.insert("BID".to_string(), Some("".to_string()));
    fields.insert("OFFER".to_string(), Some("".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_price_data_from_item_update_with_changed_fields() {
    let mut fields = HashMap::new();
    fields.insert("BID".to_string(), Some("100.5".to_string()));

    let mut changed_fields = HashMap::new();
    changed_fields.insert("BID".to_string(), Some("101.0".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields,
        changed_fields,
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());
}

#[test]
fn test_price_data_clone() {
    let price = PriceData {
        item_name: "MARKET:TEST".to_string(),
        item_pos: 1,
        fields: PriceFields::default(),
        changed_fields: PriceFields::default(),
        is_snapshot: false,
    };

    let cloned = price.clone();
    assert_eq!(price.item_name, cloned.item_name);
    assert_eq!(price.item_pos, cloned.item_pos);
}

#[test]
fn test_price_data_from_item_update_empty_dlg_flag_is_none() {
    let mut fields = HashMap::new();
    // An empty DLG_FLAG is a *value*, not an absence: the server sent `$`
    // (empty text), which the seam maps to `Some("")`. A `#` (null) would map
    // to `None` — see `test_price_data_from_item_update_null_dlg_flag_is_none`.
    fields.insert("DLG_FLAG".to_string(), Some("".to_string()));
    fields.insert("BID".to_string(), Some("100.5".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok(), "empty DLG_FLAG should not cause an error");
    let price_data = result.unwrap();
    assert!(
        price_data.fields.dealing_flag.is_none(),
        "empty DLG_FLAG should be parsed as None"
    );
}

#[test]
fn test_price_data_from_item_update_null_dlg_flag_is_none() {
    // The other half of the null-versus-empty distinction the streaming crate
    // preserves: a null field arrives as `None` rather than `Some("")`, and the
    // parser must treat it as absent just the same.
    let mut fields = HashMap::new();
    fields.insert("DLG_FLAG".to_string(), None);
    fields.insert("BID".to_string(), Some("100.5".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok(), "null DLG_FLAG should not cause an error");
    let price_data = result.expect("null DLG_FLAG parse checked ok above");
    assert!(
        price_data.fields.dealing_flag.is_none(),
        "null DLG_FLAG should be parsed as None"
    );
    assert_eq!(
        price_data.fields.bid,
        Some(100.5),
        "a null field must not disturb its neighbours"
    );
}

#[test]
fn test_price_data_from_item_update_closingsonly_flag() {
    let mut fields = HashMap::new();
    fields.insert("DLG_FLAG".to_string(), Some("CLOSINGSONLY".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());
    let price_data = result.unwrap();
    assert_eq!(
        price_data.fields.dealing_flag,
        Some(DealingFlag::ClosingsOnly)
    );
}

#[test]
fn test_price_data_from_item_update_closingonly_backward_compat() {
    let mut fields = HashMap::new();
    // Old spelling without 'S' should also work
    fields.insert("DLG_FLAG".to_string(), Some("CLOSINGONLY".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());
    let price_data = result.unwrap();
    assert_eq!(
        price_data.fields.dealing_flag,
        Some(DealingFlag::ClosingsOnly)
    );
}

#[test]
fn test_price_data_from_item_update_with_net_chg_fields() {
    let mut fields = HashMap::new();
    fields.insert("BID".to_string(), Some("100.5".to_string()));
    fields.insert("NET_CHG".to_string(), Some("-0.5".to_string()));
    fields.insert("NET_CHG_PCT".to_string(), Some("-0.49".to_string()));
    fields.insert("DELAY".to_string(), Some("0".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());

    let price_data = result.unwrap();
    assert_eq!(price_data.fields.net_chg, Some(-0.5));
    assert_eq!(price_data.fields.net_chg_pct, Some(-0.49));
    assert_eq!(price_data.fields.delay, Some(0.0));
}

#[test]
fn test_price_data_from_trait_does_not_panic_on_error() {
    let mut fields = HashMap::new();
    // An unknown dealing flag that should cause from_item_update to return Err
    fields.insert("DLG_FLAG".to_string(), Some("UNKNOWN_FLAG".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields,
        changed_fields: HashMap::new(),
    };

    // From<&StreamingUpdate> should not panic; it returns a default PriceData
    let price_data = PriceData::from(&item_update);
    assert_eq!(price_data.item_name, "");
    assert_eq!(price_data.item_pos, 0);
}

#[test]
fn test_price_fields_round_trip_with_none_fields() {
    // Regression guard for the skip-without-default asymmetry: `PriceFields`
    // omits `None` fields on serialize (`skip_serializing_if`), so without a
    // matching `#[serde(default)]` it could not deserialize its own output.
    // Build a realistic, sparse update (only a few populated fields) and prove
    // it round-trips.
    let fields = PriceFields {
        bid: Some(18000.5),
        offer: Some(18001.5),
        market_delay: Some(false),
        market_state: Some("TRADEABLE".to_string()),
        update_time: Some("12:34:56".to_string()),
        timestamp: Some(1_700_000_000_000),
        ..Default::default()
    };

    let json = serde_json::to_string(&fields).expect("serialize should succeed");
    // The absent fields must not appear in the serialized output.
    assert!(!json.contains("MID_OPEN"), "None fields must be skipped");

    let restored: PriceFields =
        serde_json::from_str(&json).expect("PriceFields must deserialize its own output");
    assert_eq!(restored.bid, Some(18000.5));
    assert_eq!(restored.offer, Some(18001.5));
    assert_eq!(restored.market_delay, Some(false));
    assert_eq!(restored.market_state.as_deref(), Some("TRADEABLE"));
    assert_eq!(restored.update_time.as_deref(), Some("12:34:56"));
    assert_eq!(restored.timestamp, Some(1_700_000_000_000));
    // Absent fields round-trip back to None.
    assert!(restored.mid_open.is_none());
    assert!(restored.high.is_none());
}

#[test]
fn test_price_data_from_item_update_market_delay_flag() {
    // MARKET_DELAY is a 0/1 delayed-data flag, parsed as a bool (not seconds).
    let mut fields = HashMap::new();
    fields.insert("MARKET_DELAY".to_string(), Some("1".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());
    let price_data = result.expect("market delay flag should parse");
    assert_eq!(price_data.fields.market_delay, Some(true));
}

#[test]
fn test_price_data_from_item_update_market_delay_invalid_is_error() {
    let mut fields = HashMap::new();
    fields.insert("MARKET_DELAY".to_string(), Some("7".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(
        result.is_err(),
        "a non-0/1 MARKET_DELAY value must surface an error"
    );
}

#[test]
fn test_price_data_from_item_update_timestamp_is_epoch_millis_i64() {
    // TIMESTAMP is epoch milliseconds (UTC), parsed as i64 to avoid float
    // rounding on large integer values.
    let mut fields = HashMap::new();
    fields.insert("TIMESTAMP".to_string(), Some("1700000000123".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: true,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(result.is_ok());
    let price_data = result.expect("timestamp should parse");
    assert_eq!(price_data.fields.timestamp, Some(1_700_000_000_123));
}

#[test]
fn test_price_data_from_item_update_dlg_flag_with_trailing_spaces() {
    let mut fields = HashMap::new();
    // Server sends DLG_FLAG with trailing whitespace padding
    fields.insert("DLG_FLAG".to_string(), Some("DEAL         ".to_string()));

    let item_update = StreamingUpdate {
        item_name: Some("MARKET:TEST".to_string()),
        item_pos: 1,
        is_snapshot: false,
        fields,
        changed_fields: HashMap::new(),
    };

    let result = price_data_from_item_update(&item_update);
    assert!(
        result.is_ok(),
        "DLG_FLAG with trailing spaces should not error"
    );
    let price_data = result.unwrap();
    assert_eq!(price_data.fields.dealing_flag, Some(DealingFlag::Deal));
}
