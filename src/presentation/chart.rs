use crate::presentation::serialization::string_as_float_opt;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

/// Time scale for chart data aggregation
#[repr(u8)]
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum ChartScale {
    /// Second-level aggregation
    #[serde(rename = "SECOND")]
    Second,
    /// One-minute aggregation
    #[serde(rename = "1MINUTE")]
    OneMinute,
    /// Five-minute aggregation
    #[serde(rename = "5MINUTE")]
    FiveMinute,
    /// Hourly aggregation
    #[serde(rename = "HOUR")]
    Hour,
    /// Tick-by-tick data (no aggregation)
    #[serde(rename = "TICK")]
    #[default]
    Tick,
}

impl fmt::Debug for ChartScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ChartScale::Second => "SECOND",
            ChartScale::OneMinute => "1MINUTE",
            ChartScale::FiveMinute => "5MINUTE",
            ChartScale::Hour => "HOUR",
            ChartScale::Tick => "TICK",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for ChartScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
/// Chart data structure that represents price chart information
/// Contains both tick and candle data depending on the chart scale
pub struct ChartData {
    /// The full Lightstreamer item name (e.g., `CHART:EPIC:TIMESCALE`)
    pub item_name: String,
    /// The 1-based position of the item in the subscription
    pub item_pos: usize,
    /// Resolved chart scale for this update (derived from item name or `scale`)
    #[serde(default)]
    pub scale: ChartScale, // Derived from the item name or the {scale} field
    /// All current field values for the item
    pub fields: ChartFields,
    /// Only the fields that changed in this update
    pub changed_fields: ChartFields,
    /// Whether this update is part of the initial snapshot
    pub is_snapshot: bool,
}

/// Chart field data containing price, volume, and timestamp information
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
pub struct ChartFields {
    // Common fields for both chart types
    #[serde(rename = "LTV")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Last traded volume for the period (or tick)
    pub last_traded_volume: Option<f64>,

    #[serde(rename = "TTV")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Incremental trading volume for the period
    pub incremental_trading_volume: Option<f64>,

    #[serde(rename = "UTM")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Update time timestamp for the data point
    pub update_time: Option<f64>,

    #[serde(rename = "DAY_OPEN_MID")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Day opening mid price
    pub day_open_mid: Option<f64>,

    #[serde(rename = "DAY_NET_CHG_MID")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Day net change in mid price
    pub day_net_change_mid: Option<f64>,

    #[serde(rename = "DAY_PERC_CHG_MID")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Day percentage change in mid price
    pub day_percentage_change_mid: Option<f64>,

    #[serde(rename = "DAY_HIGH")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Day high price
    pub day_high: Option<f64>,

    #[serde(rename = "DAY_LOW")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Day low price
    pub day_low: Option<f64>,

    // Fields specific to TICK
    #[serde(rename = "BID")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Current bid price (for tick data)
    pub bid: Option<f64>,

    #[serde(rename = "OFR")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Current offer/ask price (for tick data)
    pub offer: Option<f64>,

    #[serde(rename = "LTP")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Last traded price (for tick data)
    pub last_traded_price: Option<f64>,

    // Fields specific to CANDLE
    #[serde(rename = "OFR_OPEN")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Offer opening price for the candle period
    pub offer_open: Option<f64>,

    #[serde(rename = "OFR_HIGH")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Offer high price for the candle period
    pub offer_high: Option<f64>,

    #[serde(rename = "OFR_LOW")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Offer low price for the candle period
    pub offer_low: Option<f64>,

    #[serde(rename = "OFR_CLOSE")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Offer closing price for the candle period
    pub offer_close: Option<f64>,

    #[serde(rename = "BID_OPEN")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Bid opening price for the candle period
    pub bid_open: Option<f64>,

    #[serde(rename = "BID_HIGH")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Bid high price for the candle period
    pub bid_high: Option<f64>,

    #[serde(rename = "BID_LOW")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Bid low price for the candle period
    pub bid_low: Option<f64>,

    #[serde(rename = "BID_CLOSE")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Bid closing price for the candle period
    pub bid_close: Option<f64>,

    #[serde(rename = "LTP_OPEN")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Last traded price opening for the candle period
    pub ltp_open: Option<f64>,

    #[serde(rename = "LTP_HIGH")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Last traded price high for the candle period
    pub ltp_high: Option<f64>,

    #[serde(rename = "LTP_LOW")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Last traded price low for the candle period
    pub ltp_low: Option<f64>,

    #[serde(rename = "LTP_CLOSE")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Last traded price closing for the candle period
    pub ltp_close: Option<f64>,

    #[serde(rename = "CONS_END")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Candle end timestamp
    pub candle_end: Option<f64>,

    #[serde(rename = "CONS_TICK_COUNT")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    /// Number of ticks consolidated in this candle
    pub candle_tick_count: Option<f64>,
}

impl ChartData {
    /// Builds a [`ChartData`] from pre-extracted streaming fields.
    ///
    /// This is transport-agnostic: it takes plain field maps rather than a
    /// Lightstreamer `ItemUpdate`, so the presentation layer carries no
    /// dependency on the streaming transport. The `ItemUpdate` adapter lives in
    /// [`crate::application::streaming_convert`]. The chart scale is derived from
    /// the item name suffix (or the `{scale}` field as a fallback).
    ///
    /// # Arguments
    /// * `item_name` - Subscription item name (`None` when subscribed by position)
    /// * `item_pos` - 1-based position of the item in the subscription
    /// * `is_snapshot` - Whether this update is a snapshot
    /// * `fields` - Current field values for the item
    /// * `changed_fields` - Field values that changed in this update
    ///
    /// # Returns
    /// A Result containing either the parsed ChartData or an error message
    pub fn from_fields(
        item_name: Option<&str>,
        item_pos: usize,
        is_snapshot: bool,
        fields: &HashMap<String, Option<String>>,
        changed_fields: &HashMap<String, Option<String>>,
    ) -> Result<Self, String> {
        // Determine the chart scale from the item name
        let scale = if let Some(name) = item_name {
            if name.ends_with(":TICK") {
                ChartScale::Tick
            } else if name.ends_with(":SECOND") {
                ChartScale::Second
            } else if name.ends_with(":1MINUTE") {
                ChartScale::OneMinute
            } else if name.ends_with(":5MINUTE") {
                ChartScale::FiveMinute
            } else if name.ends_with(":HOUR") {
                ChartScale::Hour
            } else {
                // Try to determine the scale from a {scale} field if it exists
                match fields.get("{scale}").and_then(|s| s.as_ref()) {
                    Some(s) if s == "SECOND" => ChartScale::Second,
                    Some(s) if s == "1MINUTE" => ChartScale::OneMinute,
                    Some(s) if s == "5MINUTE" => ChartScale::FiveMinute,
                    Some(s) if s == "HOUR" => ChartScale::Hour,
                    _ => ChartScale::Tick, // Default
                }
            }
        } else {
            ChartScale::default()
        };

        // Convert fields
        let parsed_fields = Self::create_chart_fields(fields)?;
        let parsed_changed_fields = Self::create_chart_fields(changed_fields)?;

        Ok(ChartData {
            item_name: item_name.unwrap_or_default().to_string(),
            item_pos,
            scale,
            fields: parsed_fields,
            changed_fields: parsed_changed_fields,
            is_snapshot,
        })
    }

    // Helper method to create ChartFields from a HashMap
    fn create_chart_fields(
        fields_map: &HashMap<String, Option<String>>,
    ) -> Result<ChartFields, String> {
        // Helper function to safely get a field value
        let get_field = |key: &str| -> Option<String> { fields_map.get(key).cloned().flatten() };

        // Helper function to parse float values
        let parse_float = |key: &str| -> Result<Option<f64>, String> {
            match get_field(key) {
                Some(val) if !val.is_empty() => val
                    .parse::<f64>()
                    .map(Some)
                    .map_err(|_| format!("Failed to parse {key} as float: {val}")),
                _ => Ok(None),
            }
        };

        Ok(ChartFields {
            // Common fields
            last_traded_volume: parse_float("LTV")?,
            incremental_trading_volume: parse_float("TTV")?,
            update_time: parse_float("UTM")?,
            day_open_mid: parse_float("DAY_OPEN_MID")?,
            day_net_change_mid: parse_float("DAY_NET_CHG_MID")?,
            day_percentage_change_mid: parse_float("DAY_PERC_CHG_MID")?,
            day_high: parse_float("DAY_HIGH")?,
            day_low: parse_float("DAY_LOW")?,

            // Fields specific to TICK
            bid: parse_float("BID")?,
            offer: parse_float("OFR")?,
            last_traded_price: parse_float("LTP")?,

            // Fields specific to CANDLE
            offer_open: parse_float("OFR_OPEN")?,
            offer_high: parse_float("OFR_HIGH")?,
            offer_low: parse_float("OFR_LOW")?,
            offer_close: parse_float("OFR_CLOSE")?,
            bid_open: parse_float("BID_OPEN")?,
            bid_high: parse_float("BID_HIGH")?,
            bid_low: parse_float("BID_LOW")?,
            bid_close: parse_float("BID_CLOSE")?,
            ltp_open: parse_float("LTP_OPEN")?,
            ltp_high: parse_float("LTP_HIGH")?,
            ltp_low: parse_float("LTP_LOW")?,
            ltp_close: parse_float("LTP_CLOSE")?,
            candle_end: parse_float("CONS_END")?,
            candle_tick_count: parse_float("CONS_TICK_COUNT")?,
        })
    }

    /// Checks if these chart data are of type TICK
    pub fn is_tick(&self) -> bool {
        matches!(self.scale, ChartScale::Tick)
    }

    /// Checks if these chart data are of type CANDLE (any time scale)
    pub fn is_candle(&self) -> bool {
        !self.is_tick()
    }

    /// Gets the time scale of the data
    pub fn get_scale(&self) -> &ChartScale {
        &self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_scale_default() {
        let scale = ChartScale::default();
        assert_eq!(scale, ChartScale::Tick);
    }

    #[test]
    fn test_chart_scale_debug() {
        assert_eq!(format!("{:?}", ChartScale::Second), "SECOND");
        assert_eq!(format!("{:?}", ChartScale::OneMinute), "1MINUTE");
        assert_eq!(format!("{:?}", ChartScale::FiveMinute), "5MINUTE");
        assert_eq!(format!("{:?}", ChartScale::Hour), "HOUR");
        assert_eq!(format!("{:?}", ChartScale::Tick), "TICK");
    }

    #[test]
    fn test_chart_scale_display() {
        assert_eq!(format!("{}", ChartScale::Second), "SECOND");
        assert_eq!(format!("{}", ChartScale::Tick), "TICK");
    }

    #[test]
    fn test_chart_scale_serialization() {
        let scale = ChartScale::OneMinute;
        let json = serde_json::to_string(&scale).expect("serialize failed");
        assert_eq!(json, "\"1MINUTE\"");

        let deserialized: ChartScale = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized, ChartScale::OneMinute);
    }

    #[test]
    fn test_chart_data_default() {
        let data = ChartData::default();
        assert!(data.item_name.is_empty());
        assert_eq!(data.item_pos, 0);
        assert_eq!(data.scale, ChartScale::Tick);
        assert!(!data.is_snapshot);
    }

    #[test]
    fn test_chart_data_is_tick() {
        let data = ChartData {
            scale: ChartScale::Tick,
            ..Default::default()
        };
        assert!(data.is_tick());
        assert!(!data.is_candle());
    }

    #[test]
    fn test_chart_data_is_candle() {
        let data = ChartData {
            scale: ChartScale::OneMinute,
            ..Default::default()
        };
        assert!(!data.is_tick());
        assert!(data.is_candle());

        let data_hour = ChartData {
            scale: ChartScale::Hour,
            ..Default::default()
        };
        assert!(data_hour.is_candle());
    }

    #[test]
    fn test_chart_data_get_scale() {
        let data = ChartData {
            scale: ChartScale::FiveMinute,
            ..Default::default()
        };
        assert_eq!(*data.get_scale(), ChartScale::FiveMinute);
    }

    #[test]
    fn test_chart_fields_default() {
        let fields = ChartFields::default();
        assert!(fields.bid.is_none());
        assert!(fields.offer.is_none());
        assert!(fields.last_traded_price.is_none());
        assert!(fields.day_high.is_none());
        assert!(fields.day_low.is_none());
    }

    #[test]
    fn test_chart_fields_creation() {
        let fields = ChartFields {
            bid: Some(100.5),
            offer: Some(101.0),
            last_traded_price: Some(100.75),
            day_high: Some(102.0),
            day_low: Some(99.0),
            ..Default::default()
        };
        assert_eq!(fields.bid, Some(100.5));
        assert_eq!(fields.offer, Some(101.0));
        assert_eq!(fields.last_traded_price, Some(100.75));
    }

    #[test]
    fn test_chart_scale_equality() {
        assert_eq!(ChartScale::Tick, ChartScale::Tick);
        assert_ne!(ChartScale::Tick, ChartScale::Hour);
    }

    #[test]
    fn test_chart_scale_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ChartScale::Tick);
        set.insert(ChartScale::Tick);
        assert_eq!(set.len(), 1);
    }
}
