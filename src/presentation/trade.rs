use crate::presentation::order::{Direction, OrderType, Status, TimeInForce};
use crate::presentation::serialization::{option_string_empty_as_none, string_as_float_opt};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

/// Main structure for trade data received from the IG Markets API
/// Contains information about trades, positions and working orders
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
pub struct TradeData {
    /// Name of the subscribed item
    pub item_name: String,
    /// Position of the item in the subscription
    pub item_pos: usize,
    /// Trade fields data
    pub fields: TradeFields,
    /// Changed fields data
    pub changed_fields: TradeFields,
    /// Whether this is a snapshot
    pub is_snapshot: bool,
}

/// Main fields for a trade update, containing core trade data.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub struct TradeFields {
    /// Optional confirmation details for the trade.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirms: Option<String>,
    /// Optional open position update details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opu: Option<OpenPositionUpdate>,
    /// Optional working order update details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wou: Option<WorkingOrderUpdate>,
}

/// Structure representing details of an open position update.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
pub struct OpenPositionUpdate {
    /// Unique deal reference for the open position.
    #[serde(rename = "dealReference")]
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub deal_reference: Option<String>,
    /// Unique deal identifier for the position.
    #[serde(rename = "dealId")]
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub deal_id: Option<String>,
    /// Direction of the trade position (buy or sell).
    #[serde(default)]
    pub direction: Option<Direction>,
    /// Epic identifier for the instrument.
    #[serde(default)]
    pub epic: Option<String>,
    /// Status of the position.
    #[serde(default)]
    pub status: Option<Status>,
    /// Deal status of the position.
    #[serde(rename = "dealStatus")]
    #[serde(default)]
    pub deal_status: Option<Status>,
    /// Price level of the position.
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub level: Option<f64>,
    /// Position size.
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub size: Option<f64>,
    /// Currency of the position.
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub currency: Option<String>,
    /// Timestamp of the position update.
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub timestamp: Option<String>,
    /// Channel through which the update was received.
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub channel: Option<String>,
    /// Expiry date of the position, if applicable.
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub expiry: Option<String>,
    /// Original deal identifier for the position.
    #[serde(rename = "dealIdOrigin")]
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub deal_id_origin: Option<String>,
}

/// Structure representing details of a working order update.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
pub struct WorkingOrderUpdate {
    /// Unique deal reference for the working order.
    #[serde(rename = "dealReference")]
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub deal_reference: Option<String>,
    /// Unique deal identifier for the working order.
    #[serde(rename = "dealId")]
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub deal_id: Option<String>,
    /// Direction of the working order (buy or sell).
    #[serde(default)]
    pub direction: Option<Direction>,
    /// Epic identifier for the working order instrument.
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub epic: Option<String>,
    /// Status of the working order.
    #[serde(default)]
    pub status: Option<Status>,
    /// Deal status of the working order.
    #[serde(rename = "dealStatus")]
    #[serde(default)]
    pub deal_status: Option<Status>,
    /// Price level at which the working order is set.
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub level: Option<f64>,
    /// Working order size.
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub size: Option<f64>,
    /// Currency of the working order.
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub currency: Option<String>,
    /// Timestamp of the working order update.
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub timestamp: Option<String>,
    /// Channel through which the working order update was received.
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub channel: Option<String>,
    /// Expiry date of the working order.
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub expiry: Option<String>,
    /// Stop distance for guaranteed stop orders.
    #[serde(rename = "stopDistance")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub stop_distance: Option<f64>,
    /// Limit distance for guaranteed stop orders.
    #[serde(rename = "limitDistance")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub limit_distance: Option<f64>,
    /// Whether the stop is guaranteed.
    #[serde(rename = "guaranteedStop")]
    #[serde(default)]
    pub guaranteed_stop: Option<bool>,
    /// Type of the order (e.g., market, limit).
    #[serde(rename = "orderType")]
    #[serde(default)]
    pub order_type: Option<OrderType>,
    /// Time in force for the order.
    #[serde(rename = "timeInForce")]
    #[serde(default)]
    pub time_in_force: Option<TimeInForce>,
    /// Good till date for the working order.
    #[serde(rename = "goodTillDate")]
    #[serde(with = "option_string_empty_as_none")]
    #[serde(default)]
    pub good_till_date: Option<String>,
}

impl TradeData {
    /// Builds a [`TradeData`] from pre-extracted streaming fields.
    ///
    /// This is transport-agnostic: it takes plain field maps rather than a
    /// Lightstreamer `ItemUpdate`, so the presentation layer carries no
    /// dependency on the streaming transport. The `ItemUpdate` adapter lives in
    /// [`crate::application::streaming_convert`].
    ///
    /// # Arguments
    /// * `item_name` - Subscription item name (`None` when subscribed by position)
    /// * `item_pos` - 1-based position of the item in the subscription
    /// * `is_snapshot` - Whether this update is a snapshot
    /// * `fields` - Current field values for the item
    /// * `changed_fields` - Field values that changed in this update
    ///
    /// # Returns
    /// A Result containing either the parsed TradeData or an error message
    pub fn from_fields(
        item_name: Option<&str>,
        item_pos: usize,
        is_snapshot: bool,
        fields: &HashMap<String, Option<String>>,
        changed_fields: &HashMap<String, Option<String>>,
    ) -> Result<Self, String> {
        let fields = Self::create_trade_fields(fields)?;
        let changed_fields = Self::create_trade_fields(changed_fields)?;

        Ok(TradeData {
            item_name: item_name.unwrap_or_default().to_string(),
            item_pos,
            fields,
            changed_fields,
            is_snapshot,
        })
    }

    // Helper method to create TradeFields from a HashMap
    fn create_trade_fields(
        fields_map: &HashMap<String, Option<String>>,
    ) -> Result<TradeFields, String> {
        // Helper function to safely get a field value
        let get_field = |key: &str| -> Option<String> {
            let field = fields_map.get(key).cloned().flatten();
            match field {
                Some(ref s) if s.is_empty() => None,
                _ => field,
            }
        };

        // Parse CONFIRMS
        let confirms = get_field("CONFIRMS");

        // Parse OPU
        let opu_str = get_field("OPU");
        let opu = if let Some(opu_json) = opu_str {
            if !opu_json.is_empty() {
                match serde_json::from_str::<OpenPositionUpdate>(&opu_json) {
                    Ok(parsed_opu) => Some(parsed_opu),
                    Err(e) => return Err(format!("Failed to parse OPU JSON: {e}")),
                }
            } else {
                None
            }
        } else {
            None
        };
        // Parse WOU
        let wou_str = get_field("WOU");
        let wou = if let Some(wou_json) = wou_str {
            if !wou_json.is_empty() {
                match serde_json::from_str::<WorkingOrderUpdate>(&wou_json) {
                    Ok(parsed_wou) => Some(parsed_wou),
                    Err(e) => return Err(format!("Failed to parse WOU JSON: {e}")),
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(TradeFields { confirms, opu, wou })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Realistic, sanitized OPU payload. Field names mirror IG's Open Position
    // Update stream (camelCase); the deal ids are obviously synthetic and carry
    // no secret material.
    const SANITIZED_OPU_JSON: &str = r#"{
        "dealReference": "REF_SYNTHETIC_0001",
        "dealId": "DIAAAAF00SYNTH01",
        "dealIdOrigin": "DIAAAAF00SYNTH01",
        "direction": "BUY",
        "epic": "IX.D.DAX.DAILY.IP",
        "status": "OPEN",
        "dealStatus": "ACCEPTED",
        "level": "18000.5",
        "size": "1.0",
        "currency": "EUR",
        "timestamp": "2024-01-15T10:30:00.000",
        "channel": "PublicRestOTC",
        "expiry": "DFB"
    }"#;

    #[test]
    fn test_create_trade_fields_parses_opu_json() {
        // Exercise the JSON-parse helper directly (independent of ItemUpdate).
        let mut map: HashMap<String, Option<String>> = HashMap::new();
        map.insert("OPU".to_string(), Some(SANITIZED_OPU_JSON.to_string()));

        let result = TradeData::create_trade_fields(&map);
        assert!(
            result.is_ok(),
            "create_trade_fields should parse OPU: {result:?}"
        );
        let fields = result.expect("OPU parse checked ok above");
        let opu = fields.opu.expect("OPU should be populated");
        assert_eq!(opu.deal_id.as_deref(), Some("DIAAAAF00SYNTH01"));
        assert_eq!(opu.level, Some(18000.5));
    }

    #[test]
    fn test_trade_data_default() {
        let data = TradeData::default();
        assert!(data.item_name.is_empty());
        assert_eq!(data.item_pos, 0);
        assert!(!data.is_snapshot);
    }

    #[test]
    fn test_trade_fields_default() {
        let fields = TradeFields::default();
        assert!(fields.confirms.is_none());
        assert!(fields.opu.is_none());
        assert!(fields.wou.is_none());
    }

    #[test]
    fn test_open_position_update_default() {
        let opu = OpenPositionUpdate::default();
        assert!(opu.deal_reference.is_none());
        assert!(opu.deal_id.is_none());
        assert!(opu.direction.is_none());
        assert!(opu.epic.is_none());
        assert!(opu.status.is_none());
        assert!(opu.level.is_none());
        assert!(opu.size.is_none());
    }

    #[test]
    fn test_working_order_update_default() {
        let wou = WorkingOrderUpdate::default();
        assert!(wou.deal_reference.is_none());
        assert!(wou.deal_id.is_none());
        assert!(wou.direction.is_none());
        assert!(wou.epic.is_none());
        assert!(wou.status.is_none());
        assert!(wou.level.is_none());
        assert!(wou.size.is_none());
    }

    #[test]
    fn test_open_position_update_creation() {
        let opu = OpenPositionUpdate {
            deal_reference: Some("REF123".to_string()),
            deal_id: Some("DEAL456".to_string()),
            direction: Some(Direction::Buy),
            epic: Some("IX.D.DAX.DAILY.IP".to_string()),
            status: Some(Status::Open),
            level: Some(18000.0),
            size: Some(1.0),
            currency: Some("EUR".to_string()),
            ..Default::default()
        };
        assert_eq!(opu.deal_reference, Some("REF123".to_string()));
        assert_eq!(opu.deal_id, Some("DEAL456".to_string()));
        assert_eq!(opu.direction, Some(Direction::Buy));
        assert_eq!(opu.level, Some(18000.0));
    }

    #[test]
    fn test_working_order_update_creation() {
        let wou = WorkingOrderUpdate {
            deal_reference: Some("WO_REF".to_string()),
            deal_id: Some("WO_DEAL".to_string()),
            direction: Some(Direction::Sell),
            epic: Some("CS.D.EURUSD.CFD.IP".to_string()),
            status: Some(Status::Amended),
            level: Some(1.1000),
            size: Some(10000.0),
            order_type: Some(OrderType::Limit),
            time_in_force: Some(TimeInForce::GoodTillCancelled),
            ..Default::default()
        };
        assert_eq!(wou.deal_reference, Some("WO_REF".to_string()));
        assert_eq!(wou.direction, Some(Direction::Sell));
        assert_eq!(wou.order_type, Some(OrderType::Limit));
    }

    #[test]
    fn test_trade_data_creation() {
        let data = TradeData {
            item_name: "TRADE:123".to_string(),
            item_pos: 1,
            fields: TradeFields::default(),
            changed_fields: TradeFields::default(),
            is_snapshot: true,
        };
        assert_eq!(data.item_name, "TRADE:123");
        assert_eq!(data.item_pos, 1);
        assert!(data.is_snapshot);
    }

    #[test]
    fn test_trade_fields_with_confirms() {
        let fields = TradeFields {
            confirms: Some("confirmed".to_string()),
            opu: None,
            wou: None,
        };
        assert_eq!(fields.confirms, Some("confirmed".to_string()));
    }

    #[test]
    fn test_open_position_update_serialization() {
        let opu = OpenPositionUpdate {
            deal_id: Some("DEAL123".to_string()),
            level: Some(100.5),
            ..Default::default()
        };
        let json = serde_json::to_string(&opu).expect("serialize failed");
        assert!(json.contains("DEAL123"));
    }

    #[test]
    fn test_working_order_update_serialization() {
        let wou = WorkingOrderUpdate {
            deal_id: Some("WO123".to_string()),
            level: Some(50.0),
            ..Default::default()
        };
        let json = serde_json::to_string(&wou).expect("serialize failed");
        assert!(json.contains("WO123"));
    }
}
