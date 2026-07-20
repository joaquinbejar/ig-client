use crate::presentation::instrument::InstrumentType;
use crate::presentation::serialization::{string_as_bool_opt, string_as_float_opt};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Model for a market instrument with enhanced deserialization
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, PartialEq)]
pub struct Instrument {
    /// Unique identifier for the instrument
    pub epic: String,
    /// Human-readable name of the instrument
    pub name: String,
    /// Expiry date of the instrument
    pub expiry: String,
    /// Size of one contract
    #[serde(rename = "contractSize")]
    pub contract_size: String,
    /// Size of one lot
    #[serde(rename = "lotSize")]
    pub lot_size: Option<f64>,
    /// Upper price limit for the instrument
    #[serde(rename = "highLimitPrice")]
    pub high_limit_price: Option<f64>,
    /// Lower price limit for the instrument
    #[serde(rename = "lowLimitPrice")]
    pub low_limit_price: Option<f64>,
    /// Margin factor for the instrument
    #[serde(rename = "marginFactor")]
    pub margin_factor: Option<f64>,
    /// Unit for the margin factor
    #[serde(rename = "marginFactorUnit")]
    pub margin_factor_unit: Option<String>,
    /// Available currencies for trading this instrument
    pub currencies: Option<Vec<Currency>>,
    #[serde(rename = "valueOfOnePip")]
    /// Value of one pip for this instrument
    pub value_of_one_pip: String,
    /// Type of the instrument
    #[serde(rename = "instrumentType")]
    pub instrument_type: Option<InstrumentType>,
    /// Expiry details including last dealing date
    #[serde(rename = "expiryDetails")]
    pub expiry_details: Option<ExpiryDetails>,
    #[serde(rename = "slippageFactor")]
    /// Slippage factor for the instrument
    pub slippage_factor: Option<StepDistance>,
    #[serde(rename = "limitedRiskPremium")]
    /// Premium for limited risk trades
    pub limited_risk_premium: Option<StepDistance>,
    #[serde(rename = "newsCode")]
    /// Code used for news related to this instrument
    pub news_code: Option<String>,
    #[serde(rename = "chartCode")]
    /// Code used for charting this instrument
    pub chart_code: Option<String>,
}

/// Model for an instrument's currency
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, PartialEq)]
pub struct Currency {
    /// Currency code (e.g., "USD", "EUR")
    pub code: String,
    /// Currency symbol (e.g., "$", "€")
    pub symbol: Option<String>,
    /// Base exchange rate for the currency
    #[serde(rename = "baseExchangeRate")]
    pub base_exchange_rate: Option<f64>,
    /// Current exchange rate
    #[serde(rename = "exchangeRate")]
    pub exchange_rate: Option<f64>,
    /// Whether this is the default currency for the instrument
    #[serde(rename = "isDefault")]
    pub is_default: Option<bool>,
}

/// Model for market data with enhanced deserialization
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct MarketDetails {
    /// Detailed information about the instrument
    pub instrument: Instrument,
    /// Current market snapshot with prices
    pub snapshot: MarketSnapshot,
    /// Trading rules for the market
    #[serde(rename = "dealingRules")]
    pub dealing_rules: DealingRules,
}

/// Trading rules for a market with enhanced deserialization
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct DealingRules {
    /// Minimum step distance
    #[serde(rename = "minStepDistance")]
    pub min_step_distance: Option<StepDistance>,

    /// Minimum deal size allowed
    #[serde(rename = "minDealSize")]
    pub min_deal_size: Option<StepDistance>,

    /// Minimum distance for controlled risk stop
    #[serde(rename = "minControlledRiskStopDistance")]
    pub min_controlled_risk_stop_distance: Option<StepDistance>,

    /// Minimum distance for normal stop or limit orders
    #[serde(rename = "minNormalStopOrLimitDistance")]
    pub min_normal_stop_or_limit_distance: Option<StepDistance>,

    /// Maximum distance for stop or limit orders
    #[serde(rename = "maxStopOrLimitDistance")]
    pub max_stop_or_limit_distance: Option<StepDistance>,

    /// Controlled risk spacing
    #[serde(rename = "controlledRiskSpacing")]
    pub controlled_risk_spacing: Option<StepDistance>,

    /// Market order preference setting
    #[serde(rename = "marketOrderPreference")]
    pub market_order_preference: String,

    /// Trailing stops preference setting
    #[serde(rename = "trailingStopsPreference")]
    pub trailing_stops_preference: String,

    #[serde(rename = "maxDealSize")]
    /// Maximum deal size allowed
    pub max_deal_size: Option<f64>,
}

/// Market snapshot with enhanced deserialization
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    /// Current status of the market (e.g., "OPEN", "CLOSED")
    #[serde(rename = "marketStatus")]
    pub market_status: String,

    /// Net change in price since previous close
    #[serde(rename = "netChange")]
    pub net_change: Option<f64>,

    /// Percentage change in price since previous close
    #[serde(rename = "percentageChange")]
    pub percentage_change: Option<f64>,

    /// Time of the last price update
    #[serde(rename = "updateTime")]
    pub update_time: Option<String>,

    /// Delay time in minutes for market data
    #[serde(rename = "delayTime")]
    pub delay_time: Option<i64>,

    /// Current bid price
    pub bid: Option<f64>,

    /// Current offer/ask price
    pub offer: Option<f64>,

    /// Highest price of the current trading session
    pub high: Option<f64>,

    /// Lowest price of the current trading session
    pub low: Option<f64>,

    /// Odds for binary markets
    #[serde(rename = "binaryOdds")]
    pub binary_odds: Option<f64>,

    /// Factor for decimal places in price display
    #[serde(rename = "decimalPlacesFactor")]
    pub decimal_places_factor: Option<i64>,

    /// Factor for scaling prices
    #[serde(rename = "scalingFactor")]
    pub scaling_factor: Option<i64>,

    /// Extra spread for controlled risk trades
    #[serde(rename = "controlledRiskExtraSpread")]
    pub controlled_risk_extra_spread: Option<f64>,
}

/// Basic market data
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct MarketData {
    /// Unique identifier for the market
    pub epic: String,
    /// Human-readable name of the instrument
    #[serde(rename = "instrumentName")]
    pub instrument_name: String,
    /// Type of the instrument
    #[serde(rename = "instrumentType")]
    pub instrument_type: InstrumentType,
    /// Expiry date of the instrument
    pub expiry: String,
    /// Upper price limit for the market
    #[serde(rename = "highLimitPrice")]
    pub high_limit_price: Option<f64>,
    /// Lower price limit for the market
    #[serde(rename = "lowLimitPrice")]
    pub low_limit_price: Option<f64>,
    /// Current status of the market
    #[serde(rename = "marketStatus")]
    pub market_status: String,
    /// Net change in price since previous close
    #[serde(rename = "netChange")]
    pub net_change: Option<f64>,
    /// Percentage change in price since previous close
    #[serde(rename = "percentageChange")]
    pub percentage_change: Option<f64>,
    /// Time of the last price update
    #[serde(rename = "updateTime")]
    pub update_time: Option<String>,
    /// Time of the last price update in UTC
    #[serde(rename = "updateTimeUTC")]
    pub update_time_utc: Option<String>,
    /// Current bid price
    pub bid: Option<f64>,
    /// Current offer/ask price
    pub offer: Option<f64>,
}

impl MarketData {
    /// Checks if the current financial instrument is a call option.
    ///
    /// A call option is a financial derivative that gives the holder the right (but not the obligation)
    /// to buy an underlying asset at a specified price within a specified time period. This method checks
    /// whether the instrument represented by this instance is a call option by inspecting the `instrument_name`
    /// field.
    ///
    /// # Returns
    ///
    /// * `true` if the instrument's name contains the substring `"CALL"`, indicating it is a call option.
    /// * `false` otherwise.
    ///
    #[must_use]
    #[inline]
    pub fn is_call(&self) -> bool {
        self.instrument_name.contains("CALL")
    }

    /// Checks if the financial instrument is a "PUT" option.
    ///
    /// This method examines the `instrument_name` field of the struct to determine
    /// if it contains the substring "PUT". If the substring is found, the method
    /// returns `true`, indicating that the instrument is categorized as a "PUT" option.
    /// Otherwise, it returns `false`.
    ///
    /// # Returns
    /// * `true` - If `instrument_name` contains the substring "PUT".
    /// * `false` - If `instrument_name` does not contain the substring "PUT".
    ///
    #[must_use]
    #[inline]
    pub fn is_put(&self) -> bool {
        self.instrument_name.contains("PUT")
    }
}

/// Historical price data point
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct HistoricalPrice {
    /// Timestamp of the price data point, in the account's timezone
    /// (e.g. `"2024/01/15 14:30:00"`). Prefer [`snapshot_time_utc`] for storage.
    ///
    /// [`snapshot_time_utc`]: HistoricalPrice::snapshot_time_utc
    #[serde(rename = "snapshotTime")]
    pub snapshot_time: String,
    /// UTC timestamp of the price data point (`"2024-01-15T14:30:00"`), as IG
    /// returns it in `snapshotTimeUTC`. Authoritative for persistence — the
    /// account-timezone `snapshot_time` shifts stored values by the account's
    /// offset. `None` on older payloads that omit the field.
    #[serde(rename = "snapshotTimeUTC", default)]
    pub snapshot_time_utc: Option<String>,
    /// Opening price for the period
    #[serde(rename = "openPrice")]
    pub open_price: PricePoint,
    /// Highest price for the period
    #[serde(rename = "highPrice")]
    pub high_price: PricePoint,
    /// Lowest price for the period
    #[serde(rename = "lowPrice")]
    pub low_price: PricePoint,
    /// Closing price for the period
    #[serde(rename = "closePrice")]
    pub close_price: PricePoint,
    /// Volume traded during the period
    #[serde(rename = "lastTradedVolume")]
    pub last_traded_volume: Option<i64>,
}

/// Price point with bid, ask and last traded prices
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    /// Bid price at this point
    pub bid: Option<f64>,
    /// Ask/offer price at this point
    pub ask: Option<f64>,
    /// Last traded price at this point
    #[serde(rename = "lastTraded")]
    pub last_traded: Option<f64>,
}

/// Information about API usage allowance for price data
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct PriceAllowance {
    /// Remaining API calls allowed in the current period
    #[serde(rename = "remainingAllowance")]
    pub remaining_allowance: i64,
    /// Total API calls allowed per period
    #[serde(rename = "totalAllowance")]
    pub total_allowance: i64,
    /// Time until the allowance resets
    #[serde(rename = "allowanceExpiry")]
    pub allowance_expiry: i64,
}

/// Details about instrument expiry
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExpiryDetails {
    /// The last dealing date and time for the instrument
    #[serde(rename = "lastDealingDate")]
    pub last_dealing_date: String,

    /// Information about settlement
    #[serde(rename = "settlementInfo")]
    pub settlement_info: Option<String>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// Unit for step distances in trading rules
pub enum StepUnit {
    #[serde(rename = "POINTS")]
    /// Points (price movement units)
    Points,
    #[serde(rename = "PERCENTAGE")]
    /// Percentage value
    Percentage,
    #[serde(rename = "pct")]
    /// Alternative representation for percentage
    Pct,
}

/// A struct to handle the minStepDistance value which can be a complex object
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepDistance {
    /// Unit type for the distance
    pub unit: Option<StepUnit>,
    /// Numeric value of the distance
    pub value: Option<f64>,
}

/// Node in the market navigation hierarchy
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct MarketNavigationNode {
    /// Unique identifier for the node
    pub id: String,
    /// Display name of the node
    pub name: String,
}

/// Structure representing a node in the market hierarchy
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct MarketNode {
    /// Node ID
    pub id: String,
    /// Node name
    pub name: String,
    /// Child nodes
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<MarketNode>,
    /// Markets in this node
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub markets: Vec<MarketData>,
}

/// Represents the current state of a market
#[repr(u8)]
#[derive(
    DebugPretty, DisplaySimple, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketState {
    /// Market is closed for trading
    Closed,
    /// Market is offline and not available
    #[default]
    Offline,
    /// Market is open and available for trading
    Tradeable,
    /// Market is in edit mode
    Edit,
    /// Market is in edit mode only (no new positions, only edits allowed)
    EditsOnly,
    /// Market is in auction phase
    Auction,
    /// Market is in auction phase but editing is not allowed
    AuctionNoEdit,
    /// Market is temporarily suspended
    Suspended,
    /// Market is in auction phase
    OnAuction,
    /// Market is in auction phase but editing is not allowed
    OnAuctionNoEdits,
}

/// Representation of market data received from the IG Markets streaming API
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
pub struct PresentationMarketData {
    /// Name of the item this data belongs to
    pub item_name: String,
    /// Position of the item in the subscription
    pub item_pos: usize,
    /// All market fields
    pub fields: MarketFields,
    /// Fields that have changed in this update
    pub changed_fields: MarketFields,
    /// Whether this is a snapshot or an update
    pub is_snapshot: bool,
}

impl PresentationMarketData {
    /// Builds a [`PresentationMarketData`] from pre-extracted streaming fields.
    ///
    /// This is transport-agnostic: it takes plain field maps rather than a
    /// Lightstreamer `ItemUpdate`, so the presentation layer carries no
    /// dependency on the streaming transport. The `ItemUpdate` adapter lives in
    /// `application::streaming_convert` (feature `streaming`).
    ///
    /// # Arguments
    /// * `item_name` - Subscription item name (`None` when subscribed by position)
    /// * `item_pos` - 1-based position of the item in the subscription
    /// * `is_snapshot` - Whether this update is a snapshot
    /// * `fields` - Current field values for the item
    /// * `changed_fields` - Field values that changed in this update
    ///
    /// # Returns
    /// * `Result<Self, String>` - The converted MarketData or an error message
    pub fn from_fields(
        item_name: Option<&str>,
        item_pos: usize,
        is_snapshot: bool,
        fields: &HashMap<String, Option<String>>,
        changed_fields: &HashMap<String, Option<String>>,
    ) -> Result<Self, String> {
        let fields = Self::create_market_fields(fields)?;
        let changed_fields = Self::create_market_fields(changed_fields)?;

        Ok(PresentationMarketData {
            item_name: item_name.unwrap_or_default().to_string(),
            item_pos,
            fields,
            changed_fields,
            is_snapshot,
        })
    }

    /// Helper method to create MarketFields from a HashMap of field values
    ///
    /// # Arguments
    /// * `fields_map` - HashMap containing field names and their string values
    ///
    /// # Returns
    /// * `Result<MarketFields, String>` - The parsed MarketFields or an error message
    fn create_market_fields(
        fields_map: &HashMap<String, Option<String>>,
    ) -> Result<MarketFields, String> {
        // Helper function to safely get a field value
        let get_field = |key: &str| -> Option<String> { fields_map.get(key).cloned().flatten() };

        // Parse market state
        let market_state = match get_field("MARKET_STATE").as_deref() {
            Some("closed") => Some(MarketState::Closed),
            Some("offline") => Some(MarketState::Offline),
            Some("tradeable") => Some(MarketState::Tradeable),
            Some("edit") => Some(MarketState::Edit),
            Some("auction") => Some(MarketState::Auction),
            Some("auction_no_edit") => Some(MarketState::AuctionNoEdit),
            Some("suspended") => Some(MarketState::Suspended),
            Some("on_auction") => Some(MarketState::OnAuction),
            Some("on_auction_no_edit") => Some(MarketState::OnAuctionNoEdits),
            Some(unknown) => return Err(format!("Unknown market state: {unknown}")),
            None => None,
        };

        // Parse boolean field
        let market_delay = match get_field("MARKET_DELAY").as_deref() {
            Some("0") => Some(false),
            Some("1") => Some(true),
            Some(val) => return Err(format!("Invalid MARKET_DELAY value: {val}")),
            None => None,
        };

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

        Ok(MarketFields {
            mid_open: parse_float("MID_OPEN")?,
            high: parse_float("HIGH")?,
            offer: parse_float("OFFER")?,
            change: parse_float("CHANGE")?,
            market_delay,
            low: parse_float("LOW")?,
            bid: parse_float("BID")?,
            change_pct: parse_float("CHANGE_PCT")?,
            market_state,
            update_time: get_field("UPDATE_TIME"),
        })
    }
}

/// Represents a category of instruments in the IG Markets API
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Category {
    /// Category code identifier
    pub code: String,
    /// True if the category is non-tradeable
    #[serde(rename = "nonTradeable")]
    pub non_tradeable: bool,
}

/// Market status for category instruments
#[repr(u8)]
#[derive(
    DebugPretty, DisplaySimple, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CategoryMarketStatus {
    /// Market is offline
    #[default]
    Offline,
    /// Market is closed
    Closed,
    /// Market is suspended
    Suspended,
    /// Market is in auction mode
    OnAuction,
    /// Market is in no-edits mode
    OnAuctionNoEdits,
    /// Market is open for edits only
    EditsOnly,
    /// Market allows closings only
    ClosingsOnly,
    /// Market allows deals but not edits
    DealNoEdit,
    /// Market is open for trades
    Tradeable,
}

/// Represents an instrument within a category
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CategoryInstrument {
    /// Unique instrument identifier (EPIC)
    pub epic: String,
    /// Name of the instrument
    #[serde(rename = "instrumentName")]
    pub instrument_name: String,
    /// Expiry date of the instrument
    pub expiry: String,
    /// Type of the instrument
    #[serde(rename = "instrumentType")]
    pub instrument_type: InstrumentType,
    /// Size of an instrument lot
    #[serde(rename = "lotSize")]
    pub lot_size: Option<f64>,
    /// True if the instrument can be traded OTC
    #[serde(rename = "otcTradeable")]
    pub otc_tradeable: bool,
    /// Current status of the market
    #[serde(rename = "marketStatus")]
    pub market_status: CategoryMarketStatus,
    /// Price delay time for market data in minutes
    #[serde(rename = "delayTime")]
    pub delay_time: Option<i64>,
    /// Current bid price
    pub bid: Option<f64>,
    /// Current offer price
    pub offer: Option<f64>,
    /// Highest price for the current session
    pub high: Option<f64>,
    /// Lowest price for the current session
    pub low: Option<f64>,
    /// Net change in price
    #[serde(rename = "netChange")]
    pub net_change: Option<f64>,
    /// Percentage change in price
    #[serde(rename = "percentageChange")]
    pub percentage_change: Option<f64>,
    /// Time of last price update
    #[serde(rename = "updateTime")]
    pub update_time: Option<String>,
    /// Multiplying factor to determine actual pip value
    #[serde(rename = "scalingFactor")]
    pub scaling_factor: Option<i64>,
    /// Real expiry instant as Unix epoch in milliseconds (UTC).
    ///
    /// IG returns this as `expiryTimestamp` on the category-instruments list
    /// endpoint. It is the authoritative expiry moment (date **and** time) and
    /// is always UTC, so consumers should prefer it over parsing the
    /// human-readable `expiry` string, which carries no time-of-day.
    #[serde(rename = "expiryTimestamp")]
    pub expiry_timestamp: Option<i64>,
    /// Name of the underlying asset (e.g. "AUD/USD" for an FX option)
    #[serde(rename = "underlyingName")]
    pub underlying_name: Option<String>,
    /// Popularity ranking value; can exceed `i32` range
    pub popularity: Option<i64>,
    /// Market type of the underlying (e.g. "FX_PAIR")
    #[serde(rename = "marketType")]
    pub market_type: Option<String>,
    /// Market subtype of the underlying (e.g. "Fiat")
    #[serde(rename = "marketSubtype")]
    pub market_subtype: Option<String>,
}

/// Paging metadata for category instruments response
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CategoryInstrumentsMetadata {
    /// Current page number
    #[serde(rename = "pageNumber")]
    pub page_number: i64,
    /// Number of items per page
    #[serde(rename = "pageSize")]
    pub page_size: i64,
}

/// Fields containing market price and status information
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MarketFields {
    /// The mid-open price of the market
    #[serde(rename = "MID_OPEN")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub mid_open: Option<f64>,

    /// The highest price reached by the market in the current trading session
    #[serde(rename = "HIGH")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub high: Option<f64>,

    /// The current offer (ask) price of the market
    #[serde(rename = "OFFER")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub offer: Option<f64>,

    /// The absolute price change since the previous close
    #[serde(rename = "CHANGE")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub change: Option<f64>,

    /// Indicates if there is a delay in market data
    #[serde(rename = "MARKET_DELAY")]
    #[serde(with = "string_as_bool_opt")]
    #[serde(default)]
    pub market_delay: Option<bool>,

    /// The lowest price reached by the market in the current trading session
    #[serde(rename = "LOW")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub low: Option<f64>,

    /// The current bid price of the market
    #[serde(rename = "BID")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub bid: Option<f64>,

    /// The percentage price change since the previous close
    #[serde(rename = "CHANGE_PCT")]
    #[serde(with = "string_as_float_opt")]
    #[serde(default)]
    pub change_pct: Option<f64>,

    /// The current state of the market (e.g., Tradeable, Closed, etc.)
    #[serde(rename = "MARKET_STATE")]
    #[serde(default)]
    pub market_state: Option<MarketState>,

    /// The timestamp of the last market update
    #[serde(rename = "UPDATE_TIME")]
    #[serde(default)]
    pub update_time: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_historical_price_captures_snapshot_time_utc() {
        // IG returns both snapshotTime (account tz) and snapshotTimeUTC.
        let json = r#"{
            "snapshotTime": "2024/01/15 15:30:00",
            "snapshotTimeUTC": "2024-01-15T14:30:00",
            "openPrice": { "bid": 1.1, "ask": 1.2, "lastTraded": null },
            "highPrice": { "bid": 1.3, "ask": 1.4, "lastTraded": null },
            "lowPrice": { "bid": 1.0, "ask": 1.05, "lastTraded": null },
            "closePrice": { "bid": 1.25, "ask": 1.26, "lastTraded": null },
            "lastTradedVolume": 42
        }"#;
        let hp: HistoricalPrice = serde_json::from_str(json).expect("deserialize");
        assert_eq!(hp.snapshot_time, "2024/01/15 15:30:00");
        assert_eq!(hp.snapshot_time_utc.as_deref(), Some("2024-01-15T14:30:00"));

        // Round-trips through serialize -> deserialize.
        let round: HistoricalPrice =
            serde_json::from_str(&serde_json::to_string(&hp).expect("ser")).expect("de");
        assert_eq!(
            round.snapshot_time_utc.as_deref(),
            Some("2024-01-15T14:30:00")
        );

        // Older payloads without the field default to None.
        let legacy = r#"{
            "snapshotTime": "2024/01/15 15:30:00",
            "openPrice": { "bid": 1.1, "ask": 1.2, "lastTraded": null },
            "highPrice": { "bid": 1.3, "ask": 1.4, "lastTraded": null },
            "lowPrice": { "bid": 1.0, "ask": 1.05, "lastTraded": null },
            "closePrice": { "bid": 1.25, "ask": 1.26, "lastTraded": null },
            "lastTradedVolume": null
        }"#;
        let hp2: HistoricalPrice = serde_json::from_str(legacy).expect("deserialize legacy");
        assert!(hp2.snapshot_time_utc.is_none());
    }

    #[test]
    fn test_market_data_is_call_returns_true_for_call_option() {
        let market = MarketData {
            epic: "test".to_string(),
            instrument_name: "DAX CALL 18000".to_string(),
            instrument_type: InstrumentType::default(),
            expiry: "-".to_string(),
            high_limit_price: None,
            low_limit_price: None,
            market_status: "TRADEABLE".to_string(),
            net_change: None,
            percentage_change: None,
            update_time: None,
            update_time_utc: None,
            bid: Some(100.0),
            offer: Some(101.0),
        };
        assert!(market.is_call());
        assert!(!market.is_put());
    }

    #[test]
    fn test_market_data_is_put_returns_true_for_put_option() {
        let market = MarketData {
            epic: "test".to_string(),
            instrument_name: "DAX PUT 17000".to_string(),
            instrument_type: InstrumentType::default(),
            expiry: "-".to_string(),
            high_limit_price: None,
            low_limit_price: None,
            market_status: "TRADEABLE".to_string(),
            net_change: None,
            percentage_change: None,
            update_time: None,
            update_time_utc: None,
            bid: Some(50.0),
            offer: Some(51.0),
        };
        assert!(market.is_put());
        assert!(!market.is_call());
    }

    #[test]
    fn test_market_data_neither_call_nor_put() {
        let market = MarketData {
            epic: "IX.D.DAX.DAILY.IP".to_string(),
            instrument_name: "Germany 40".to_string(),
            instrument_type: InstrumentType::default(),
            expiry: "-".to_string(),
            high_limit_price: None,
            low_limit_price: None,
            market_status: "TRADEABLE".to_string(),
            net_change: None,
            percentage_change: None,
            update_time: None,
            update_time_utc: None,
            bid: Some(18000.0),
            offer: Some(18001.0),
        };
        assert!(!market.is_call());
        assert!(!market.is_put());
    }

    #[test]
    fn test_category_instruments_metadata_deserialize_and_roundtrip() {
        // Sanitized shape of the `metadata` object IG returns from
        // `categories/{id}/instruments`. `pageNumber` / `pageSize` are i64.
        let json = r#"{ "pageNumber": 2, "pageSize": 1000 }"#;

        let meta: CategoryInstrumentsMetadata =
            serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(meta.page_number, 2);
        assert_eq!(meta.page_size, 1000);

        let serialized = serde_json::to_string(&meta).expect("serialize failed");
        let re: CategoryInstrumentsMetadata =
            serde_json::from_str(&serialized).expect("re-deserialize failed");
        assert_eq!(re, meta);
        assert!(serialized.contains("\"pageNumber\":2"));
        assert!(serialized.contains("\"pageSize\":1000"));
    }

    #[test]
    fn test_market_state_default() {
        let state = MarketState::default();
        assert_eq!(state, MarketState::Offline);
    }

    #[test]
    fn test_category_market_status_default() {
        let status = CategoryMarketStatus::default();
        assert_eq!(status, CategoryMarketStatus::Offline);
    }

    #[test]
    fn test_step_unit_serialization() {
        let points = StepUnit::Points;
        let json = serde_json::to_string(&points).expect("serialize failed");
        assert_eq!(json, "\"POINTS\"");

        let pct = StepUnit::Percentage;
        let json = serde_json::to_string(&pct).expect("serialize failed");
        assert_eq!(json, "\"PERCENTAGE\"");
    }

    #[test]
    fn test_step_distance_creation() {
        let distance = StepDistance {
            unit: Some(StepUnit::Points),
            value: Some(1.5),
        };
        assert_eq!(distance.unit, Some(StepUnit::Points));
        assert_eq!(distance.value, Some(1.5));
    }

    #[test]
    fn test_market_fields_default() {
        let fields = MarketFields::default();
        assert!(fields.mid_open.is_none());
        assert!(fields.high.is_none());
        assert!(fields.offer.is_none());
        assert!(fields.change.is_none());
        assert!(fields.market_delay.is_none());
        assert!(fields.low.is_none());
        assert!(fields.bid.is_none());
        assert!(fields.change_pct.is_none());
        assert!(fields.market_state.is_none());
        assert!(fields.update_time.is_none());
    }

    #[test]
    fn test_presentation_market_data_default() {
        let data = PresentationMarketData::default();
        assert!(data.item_name.is_empty());
        assert_eq!(data.item_pos, 0);
        assert!(!data.is_snapshot);
    }

    #[test]
    fn test_category_default() {
        let cat = Category::default();
        assert!(cat.code.is_empty());
        assert!(!cat.non_tradeable);
    }

    #[test]
    fn test_category_instrument_default() {
        let inst = CategoryInstrument::default();
        assert!(inst.epic.is_empty());
        assert!(inst.instrument_name.is_empty());
        assert_eq!(inst.market_status, CategoryMarketStatus::Offline);
        assert!(inst.expiry_timestamp.is_none());
        assert!(inst.underlying_name.is_none());
        assert!(inst.popularity.is_none());
        assert!(inst.market_type.is_none());
        assert!(inst.market_subtype.is_none());
    }

    #[test]
    fn test_category_instrument_deserialization_full_payload() {
        let json = r#"{
            "epic": "OD.D.OTCWK2AUD.28.IP",
            "instrumentName": "Weekly AUDUSD ($1) 7200 Call",
            "expiry": "17-JUL-26",
            "instrumentType": "OPT_CURRENCIES",
            "lotSize": 1.0,
            "otcTradeable": true,
            "marketStatus": "EDITS_ONLY",
            "delayTime": 0,
            "bid": 0.0,
            "offer": 4.0,
            "high": 4.0,
            "low": 0.0,
            "netChange": 6936.2,
            "percentageChange": 0.0,
            "updateTime": "06:53:51",
            "scalingFactor": 1,
            "underlyingName": "AUD/USD",
            "popularity": 91783089620,
            "marketType": "FX_PAIR",
            "marketSubtype": "Fiat",
            "expiryTimestamp": 1784300400000
        }"#;

        let inst: CategoryInstrument = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(inst.epic, "OD.D.OTCWK2AUD.28.IP");
        assert_eq!(inst.instrument_name, "Weekly AUDUSD ($1) 7200 Call");
        assert_eq!(inst.expiry, "17-JUL-26");
        assert_eq!(inst.market_status, CategoryMarketStatus::EditsOnly);
        assert_eq!(inst.expiry_timestamp, Some(1784300400000));
        assert_eq!(inst.underlying_name.as_deref(), Some("AUD/USD"));
        assert_eq!(inst.popularity, Some(91783089620));
        assert_eq!(inst.market_type.as_deref(), Some("FX_PAIR"));
        assert_eq!(inst.market_subtype.as_deref(), Some("Fiat"));
    }

    #[test]
    fn test_category_instrument_deserialization_without_optional_fields() {
        let json = r#"{
            "epic": "IX.D.FTSE.DAILY.IP",
            "instrumentName": "FTSE 100",
            "expiry": "-",
            "instrumentType": "INDICES",
            "otcTradeable": false,
            "marketStatus": "TRADEABLE"
        }"#;

        let inst: CategoryInstrument = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(inst.epic, "IX.D.FTSE.DAILY.IP");
        assert!(inst.expiry_timestamp.is_none());
        assert!(inst.underlying_name.is_none());
        assert!(inst.popularity.is_none());
        assert!(inst.market_type.is_none());
        assert!(inst.market_subtype.is_none());
    }

    #[test]
    fn test_market_state_serialization() {
        let tradeable = MarketState::Tradeable;
        let json = serde_json::to_string(&tradeable).expect("serialize failed");
        assert_eq!(json, "\"TRADEABLE\"");

        let closed = MarketState::Closed;
        let json = serde_json::to_string(&closed).expect("serialize failed");
        assert_eq!(json, "\"CLOSED\"");
    }

    #[test]
    fn test_price_point_creation() {
        let point = PricePoint {
            bid: Some(100.5),
            ask: Some(101.0),
            last_traded: Some(100.75),
        };
        assert_eq!(point.bid, Some(100.5));
        assert_eq!(point.ask, Some(101.0));
        assert_eq!(point.last_traded, Some(100.75));
    }

    #[test]
    fn test_price_allowance_creation() {
        let allowance = PriceAllowance {
            remaining_allowance: 100,
            total_allowance: 1000,
            allowance_expiry: 3600,
        };
        assert_eq!(allowance.remaining_allowance, 100);
        assert_eq!(allowance.total_allowance, 1000);
        assert_eq!(allowance.allowance_expiry, 3600);
    }

    #[test]
    fn test_expiry_details_creation() {
        let expiry = ExpiryDetails {
            last_dealing_date: "2024-12-31".to_string(),
            settlement_info: Some("Cash settlement".to_string()),
        };
        assert_eq!(expiry.last_dealing_date, "2024-12-31");
        assert_eq!(expiry.settlement_info, Some("Cash settlement".to_string()));
    }

    #[test]
    fn test_market_navigation_node_creation() {
        let node = MarketNavigationNode {
            id: "12345".to_string(),
            name: "Indices".to_string(),
        };
        assert_eq!(node.id, "12345");
        assert_eq!(node.name, "Indices");
    }

    #[test]
    fn test_market_node_creation() {
        let node = MarketNode {
            id: "node1".to_string(),
            name: "Test Node".to_string(),
            children: Vec::new(),
            markets: Vec::new(),
        };
        assert_eq!(node.id, "node1");
        assert_eq!(node.name, "Test Node");
        assert!(node.children.is_empty());
        assert!(node.markets.is_empty());
    }

    #[test]
    fn test_currency_creation() {
        let currency = Currency {
            code: "USD".to_string(),
            symbol: Some("$".to_string()),
            base_exchange_rate: Some(1.0),
            exchange_rate: Some(1.0),
            is_default: Some(true),
        };
        assert_eq!(currency.code, "USD");
        assert_eq!(currency.symbol, Some("$".to_string()));
        assert_eq!(currency.is_default, Some(true));
    }

    #[test]
    fn test_create_market_fields_with_valid_data() {
        let mut fields_map: HashMap<String, Option<String>> = HashMap::new();
        fields_map.insert("BID".to_string(), Some("100.5".to_string()));
        fields_map.insert("OFFER".to_string(), Some("101.0".to_string()));
        fields_map.insert("HIGH".to_string(), Some("102.0".to_string()));
        fields_map.insert("LOW".to_string(), Some("99.0".to_string()));
        fields_map.insert("CHANGE".to_string(), Some("1.5".to_string()));
        fields_map.insert("CHANGE_PCT".to_string(), Some("1.5".to_string()));
        fields_map.insert("MARKET_STATE".to_string(), Some("tradeable".to_string()));
        fields_map.insert("MARKET_DELAY".to_string(), Some("0".to_string()));
        fields_map.insert("UPDATE_TIME".to_string(), Some("12:30:00".to_string()));

        let result = PresentationMarketData::create_market_fields(&fields_map);
        assert!(result.is_ok());

        let fields = result.expect("should parse");
        assert_eq!(fields.bid, Some(100.5));
        assert_eq!(fields.offer, Some(101.0));
        assert_eq!(fields.high, Some(102.0));
        assert_eq!(fields.low, Some(99.0));
        assert_eq!(fields.change, Some(1.5));
        assert_eq!(fields.market_state, Some(MarketState::Tradeable));
        assert_eq!(fields.market_delay, Some(false));
        assert_eq!(fields.update_time, Some("12:30:00".to_string()));
    }

    #[test]
    fn test_create_market_fields_with_empty_map() {
        let fields_map: HashMap<String, Option<String>> = HashMap::new();
        let result = PresentationMarketData::create_market_fields(&fields_map);
        assert!(result.is_ok());

        let fields = result.expect("should parse");
        assert!(fields.bid.is_none());
        assert!(fields.offer.is_none());
    }

    #[test]
    fn test_create_market_fields_invalid_market_state() {
        let mut fields_map: HashMap<String, Option<String>> = HashMap::new();
        fields_map.insert(
            "MARKET_STATE".to_string(),
            Some("invalid_state".to_string()),
        );

        let result = PresentationMarketData::create_market_fields(&fields_map);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_market_fields_invalid_market_delay() {
        let mut fields_map: HashMap<String, Option<String>> = HashMap::new();
        fields_map.insert("MARKET_DELAY".to_string(), Some("invalid".to_string()));

        let result = PresentationMarketData::create_market_fields(&fields_map);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_market_fields_all_market_states() {
        let states = vec![
            ("closed", MarketState::Closed),
            ("offline", MarketState::Offline),
            ("tradeable", MarketState::Tradeable),
            ("edit", MarketState::Edit),
            ("auction", MarketState::Auction),
            ("auction_no_edit", MarketState::AuctionNoEdit),
            ("suspended", MarketState::Suspended),
            ("on_auction", MarketState::OnAuction),
            ("on_auction_no_edit", MarketState::OnAuctionNoEdits),
        ];

        for (state_str, expected_state) in states {
            let mut fields_map: HashMap<String, Option<String>> = HashMap::new();
            fields_map.insert("MARKET_STATE".to_string(), Some(state_str.to_string()));

            let result = PresentationMarketData::create_market_fields(&fields_map);
            assert!(result.is_ok(), "Failed for state: {}", state_str);
            let fields = result.expect("should parse");
            assert_eq!(fields.market_state, Some(expected_state));
        }
    }

    #[test]
    fn test_market_delay_values() {
        let mut fields_map: HashMap<String, Option<String>> = HashMap::new();
        fields_map.insert("MARKET_DELAY".to_string(), Some("1".to_string()));

        let result = PresentationMarketData::create_market_fields(&fields_map);
        assert!(result.is_ok());
        let fields = result.expect("should parse");
        assert_eq!(fields.market_delay, Some(true));
    }
}
