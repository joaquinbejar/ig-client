use crate::presentation::serialization::{
    string_as_bool_opt, string_as_float_opt, string_as_int_opt,
};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Market dealing status flags indicating trading availability
#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum DealingFlag {
    /// Market is closed for trading
    #[default]
    Closed,
    /// Market is in call phase
    Call,
    /// Market is open for dealing
    Deal,
    /// Market is open for editing orders
    Edit,
    /// Market is open for closing positions only
    ClosingsOnly,
    /// Market is open for dealing but not editing
    DealNoEdit,
    /// Market is in auction phase
    Auction,
    /// Market is in auction phase without editing
    AuctionNoEdit,
    /// Market trading is suspended
    Suspend,
}

/// Structure for price data received from the IG Markets API
/// Contains information about market prices and related data
#[derive(DebugPretty, Clone, DisplaySimple, Serialize, Deserialize, Default)]
pub struct PriceData {
    /// Name of the item (usually the market ID)
    pub item_name: String,
    /// Position of the item in the subscription
    pub item_pos: usize,
    /// All price fields for this market
    pub fields: PriceFields,
    /// Fields that have changed in this update
    pub changed_fields: PriceFields,
    /// Whether this is a snapshot or an update
    pub is_snapshot: bool,
}

/// Price field data containing bid, offer, and market status information
///
/// Every field carries `skip_serializing_if = "Option::is_none"`, so absent
/// values are omitted from the serialized output. The struct-level
/// `#[serde(default)]` makes the reverse direction symmetric: a missing field
/// deserializes back to `None` instead of failing, so the DTO round-trips its
/// own output.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PriceFields {
    /// The opening price at the middle of the bid-ask spread
    #[serde(rename = "MID_OPEN")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mid_open: Option<f64>,

    /// The highest price reached during the trading session
    #[serde(rename = "HIGH")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,

    /// The lowest price reached during the trading session
    #[serde(rename = "LOW")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<f64>,

    /// Current bid price for the instrument
    #[serde(rename = "BID")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid: Option<f64>,

    /// Current offer (ask) price for the instrument
    #[serde(rename = "OFFER")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer: Option<f64>,

    /// Price change from previous close
    #[serde(rename = "CHANGE")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<f64>,

    /// Percentage change from previous close
    #[serde(rename = "CHANGE_PCT")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_pct: Option<f64>,

    /// Delayed-data flag (1 = delayed).
    ///
    /// This is IG's `MARKET_DELAY` wire field: a 0/1 flag, not a duration.
    /// Kept in sync with [`crate::presentation::market::MarketFields::market_delay`],
    /// which parses the same field the same way.
    #[serde(rename = "MARKET_DELAY")]
    #[serde(with = "string_as_bool_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_delay: Option<bool>,

    /// Current market state (e.g., "OPEN", "CLOSED", "SUSPENDED")
    #[serde(rename = "MARKET_STATE")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_state: Option<String>,

    /// Timestamp of the last price update
    #[serde(rename = "UPDATE_TIME")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<String>,

    /// Unique identifier for the bid quote
    #[serde(rename = "BIDQUOTEID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_quote_id: Option<String>,

    /// Unique identifier for the ask quote
    #[serde(rename = "ASKQUOTEID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_quote_id: Option<String>,

    // Bid ladder prices
    /// First level bid price in the order book
    #[serde(rename = "BIDPRICE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_price1: Option<f64>,

    /// Second level bid price in the order book
    #[serde(rename = "BIDPRICE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_price2: Option<f64>,

    /// Third level bid price in the order book
    #[serde(rename = "BIDPRICE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_price3: Option<f64>,

    /// Fourth level bid price in the order book
    #[serde(rename = "BIDPRICE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_price4: Option<f64>,

    /// Fifth level bid price in the order book
    #[serde(rename = "BIDPRICE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_price5: Option<f64>,

    // Ask ladder prices
    /// First level ask price in the order book
    #[serde(rename = "ASKPRICE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_price1: Option<f64>,

    /// Second level ask price in the order book
    #[serde(rename = "ASKPRICE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_price2: Option<f64>,

    /// Third level ask price in the order book
    #[serde(rename = "ASKPRICE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_price3: Option<f64>,

    /// Fourth level ask price in the order book
    #[serde(rename = "ASKPRICE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_price4: Option<f64>,

    /// Fifth level ask price in the order book
    #[serde(rename = "ASKPRICE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_price5: Option<f64>,

    // Bid sizes
    /// Volume available at the first level bid price
    #[serde(rename = "BIDSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_size1: Option<f64>,

    /// Volume available at the second level bid price
    #[serde(rename = "BIDSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_size2: Option<f64>,

    /// Volume available at the third level bid price
    #[serde(rename = "BIDSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_size3: Option<f64>,

    /// Volume available at the fourth level bid price
    #[serde(rename = "BIDSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_size4: Option<f64>,

    /// Volume available at the fifth level bid price
    #[serde(rename = "BIDSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_size5: Option<f64>,

    // Ask sizes
    /// Volume available at the first level ask price
    #[serde(rename = "ASKSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_size1: Option<f64>,

    /// Volume available at the second level ask price
    #[serde(rename = "ASKSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_size2: Option<f64>,

    /// Volume available at the third level ask price
    #[serde(rename = "ASKSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_size3: Option<f64>,

    /// Volume available at the fourth level ask price
    #[serde(rename = "ASKSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_size4: Option<f64>,

    /// Volume available at the fifth level ask price
    #[serde(rename = "ASKSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_size5: Option<f64>,

    /// Base currency code for the trading pair
    #[serde(rename = "CURRENCY0")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency0: Option<String>,

    /// First alternative currency code
    #[serde(rename = "CURRENCY1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency1: Option<String>,

    /// Second alternative currency code
    #[serde(rename = "CURRENCY2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency2: Option<String>,

    /// Third alternative currency code
    #[serde(rename = "CURRENCY3")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency3: Option<String>,

    /// Fourth alternative currency code
    #[serde(rename = "CURRENCY4")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency4: Option<String>,

    /// Fifth alternative currency code
    #[serde(rename = "CURRENCY5")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency5: Option<String>,

    /// Bid size for currency 1 at level 1
    #[serde(rename = "C1BIDSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_bid_size_1: Option<f64>,

    /// Bid size for currency 1 at level 2
    #[serde(rename = "C1BIDSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_bid_size_2: Option<f64>,

    /// Bid size for currency 1 at level 3
    #[serde(rename = "C1BIDSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_bid_size_3: Option<f64>,

    /// Bid size for currency 1 at level 4
    #[serde(rename = "C1BIDSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_bid_size_4: Option<f64>,

    /// Bid size for currency 1 at level 5
    #[serde(rename = "C1BIDSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_bid_size_5: Option<f64>,

    /// Bid size for currency 2 at level 1
    #[serde(rename = "C2BIDSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_bid_size_1: Option<f64>,

    /// Bid size for currency 2 at level 2
    #[serde(rename = "C2BIDSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_bid_size_2: Option<f64>,

    /// Bid size for currency 2 at level 3
    #[serde(rename = "C2BIDSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_bid_size_3: Option<f64>,

    /// Bid size for currency 2 at level 4
    #[serde(rename = "C2BIDSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_bid_size_4: Option<f64>,

    /// Bid size for currency 2 at level 5
    #[serde(rename = "C2BIDSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_bid_size_5: Option<f64>,

    /// Bid size for currency 3 at level 1
    #[serde(rename = "C3BIDSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_bid_size_1: Option<f64>,

    /// Bid size for currency 3 at level 2
    #[serde(rename = "C3BIDSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_bid_size_2: Option<f64>,

    /// Bid size for currency 3 at level 3
    #[serde(rename = "C3BIDSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_bid_size_3: Option<f64>,

    /// Bid size for currency 3 at level 4
    #[serde(rename = "C3BIDSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_bid_size_4: Option<f64>,

    /// Bid size for currency 3 at level 5
    #[serde(rename = "C3BIDSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_bid_size_5: Option<f64>,

    /// Bid size for currency 4 at level 1
    #[serde(rename = "C4BIDSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_bid_size_1: Option<f64>,

    /// Bid size for currency 4 at level 2
    #[serde(rename = "C4BIDSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_bid_size_2: Option<f64>,

    /// Bid size for currency 4 at level 3
    #[serde(rename = "C4BIDSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_bid_size_3: Option<f64>,

    /// Bid size for currency 4 at level 4
    #[serde(rename = "C4BIDSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_bid_size_4: Option<f64>,

    /// Bid size for currency 4 at level 5
    #[serde(rename = "C4BIDSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_bid_size_5: Option<f64>,

    /// Bid size for currency 5 at level 1
    #[serde(rename = "C5BIDSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_bid_size_1: Option<f64>,

    /// Bid size for currency 5 at level 2
    #[serde(rename = "C5BIDSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_bid_size_2: Option<f64>,

    /// Bid size for currency 5 at level 3
    #[serde(rename = "C5BIDSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_bid_size_3: Option<f64>,

    /// Bid size for currency 5 at level 4
    #[serde(rename = "C5BIDSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_bid_size_4: Option<f64>,

    /// Bid size for currency 5 at level 5
    #[serde(rename = "C5BIDSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_bid_size_5: Option<f64>,

    // Ask sizes for different currencies
    /// Ask size for currency 1 at level 1
    #[serde(rename = "C1ASKSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_ask_size_1: Option<f64>,

    /// Ask size for currency 1 at level 2
    #[serde(rename = "C1ASKSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_ask_size_2: Option<f64>,

    /// Ask size for currency 1 at level 3
    #[serde(rename = "C1ASKSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_ask_size_3: Option<f64>,

    /// Ask size for currency 1 at level 4
    #[serde(rename = "C1ASKSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_ask_size_4: Option<f64>,

    /// Ask size for currency 1 at level 5
    #[serde(rename = "C1ASKSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c1_ask_size_5: Option<f64>,

    /// Ask size for currency 2 at level 1
    #[serde(rename = "C2ASKSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_ask_size_1: Option<f64>,

    /// Ask size for currency 2 at level 2
    #[serde(rename = "C2ASKSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_ask_size_2: Option<f64>,

    /// Ask size for currency 2 at level 3
    #[serde(rename = "C2ASKSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_ask_size_3: Option<f64>,

    /// Ask size for currency 2 at level 4
    #[serde(rename = "C2ASKSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_ask_size_4: Option<f64>,

    /// Ask size for currency 2 at level 5
    #[serde(rename = "C2ASKSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c2_ask_size_5: Option<f64>,

    /// Ask size for currency 3 at level 1
    #[serde(rename = "C3ASKSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_ask_size_1: Option<f64>,

    /// Ask size for currency 3 at level 2
    #[serde(rename = "C3ASKSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_ask_size_2: Option<f64>,

    /// Ask size for currency 3 at level 3
    #[serde(rename = "C3ASKSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_ask_size_3: Option<f64>,

    /// Ask size for currency 3 at level 4
    #[serde(rename = "C3ASKSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_ask_size_4: Option<f64>,

    /// Ask size for currency 3 at level 5
    #[serde(rename = "C3ASKSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c3_ask_size_5: Option<f64>,

    /// Ask size for currency 4 at level 1
    #[serde(rename = "C4ASKSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_ask_size_1: Option<f64>,

    /// Ask size for currency 4 at level 2
    #[serde(rename = "C4ASKSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_ask_size_2: Option<f64>,

    /// Ask size for currency 4 at level 3
    #[serde(rename = "C4ASKSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_ask_size_3: Option<f64>,

    /// Ask size for currency 4 at level 4
    #[serde(rename = "C4ASKSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_ask_size_4: Option<f64>,

    /// Ask size for currency 4 at level 5
    #[serde(rename = "C4ASKSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c4_ask_size_5: Option<f64>,

    /// Ask size for currency 5 at level 1
    #[serde(rename = "C5ASKSIZE1")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_ask_size_1: Option<f64>,

    /// Ask size for currency 5 at level 2
    #[serde(rename = "C5ASKSIZE2")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_ask_size_2: Option<f64>,

    /// Ask size for currency 5 at level 3
    #[serde(rename = "C5ASKSIZE3")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_ask_size_3: Option<f64>,

    /// Ask size for currency 5 at level 4
    #[serde(rename = "C5ASKSIZE4")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_ask_size_4: Option<f64>,

    /// Ask size for currency 5 at level 5
    #[serde(rename = "C5ASKSIZE5")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c5_ask_size_5: Option<f64>,

    /// The timestamp of the price update, in epoch milliseconds (UTC).
    ///
    /// Typed as `i64` to preserve integer epoch-millis exactly (float parsing
    /// would risk silent rounding on large values).
    #[serde(rename = "TIMESTAMP")]
    #[serde(with = "string_as_int_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,

    /// Dealing status flag indicating trading availability/state of the market
    #[serde(rename = "DLG_FLAG")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dealing_flag: Option<DealingFlag>,

    /// Price change versus open
    #[serde(rename = "NET_CHG")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net_chg: Option<f64>,

    /// Percentage change versus open
    #[serde(rename = "NET_CHG_PCT")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net_chg_pct: Option<f64>,

    /// Delayed price flag (0 = false, 1 = true)
    #[serde(rename = "DELAY")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<f64>,
}

impl PriceData {
    /// Builds a [`PriceData`] from pre-extracted streaming fields.
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
    /// A Result containing either the parsed PriceData or an error message
    pub fn from_fields(
        item_name: Option<&str>,
        item_pos: usize,
        is_snapshot: bool,
        fields: &HashMap<String, Option<String>>,
        changed_fields: &HashMap<String, Option<String>>,
    ) -> Result<Self, String> {
        let fields = Self::create_price_fields(fields)?;
        let changed_fields = Self::create_price_fields(changed_fields)?;

        Ok(PriceData {
            item_name: item_name.unwrap_or_default().to_string(),
            item_pos,
            fields,
            changed_fields,
            is_snapshot,
        })
    }

    // Helper method to create PriceFields from a HashMap
    fn create_price_fields(
        fields_map: &HashMap<String, Option<String>>,
    ) -> Result<PriceFields, String> {
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

        // Helper function to parse integer values (e.g. epoch-millis timestamps).
        let parse_int = |key: &str| -> Result<Option<i64>, String> {
            match get_field(key) {
                Some(val) if !val.is_empty() => val
                    .parse::<i64>()
                    .map(Some)
                    .map_err(|_| format!("Failed to parse {key} as integer: {val}")),
                _ => Ok(None),
            }
        };

        // Parse the MARKET_DELAY flag: IG sends "0" / "1" (delayed-data flag),
        // not a duration. An empty string is treated as None.
        let market_delay = match get_field("MARKET_DELAY").as_deref() {
            Some("0") => Some(false),
            Some("1") => Some(true),
            Some("") | None => None,
            Some(val) => return Err(format!("Invalid MARKET_DELAY value: {val}")),
        };

        // Parse dealing flag (case-insensitive to handle potential lowercase conversion).
        // An empty string (produced when the server sends '#' for null) is treated as None.
        let dealing_flag = match get_field("DLG_FLAG")
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_uppercase())
            .as_deref()
        {
            Some("CLOSED") => Some(DealingFlag::Closed),
            Some("CALL") => Some(DealingFlag::Call),
            Some("DEAL") => Some(DealingFlag::Deal),
            Some("EDIT") => Some(DealingFlag::Edit),
            Some("CLOSINGSONLY") | Some("CLOSINGONLY") => Some(DealingFlag::ClosingsOnly),
            Some("DEALNOEDIT") => Some(DealingFlag::DealNoEdit),
            Some("AUCTION") => Some(DealingFlag::Auction),
            Some("AUCTIONNOEDIT") => Some(DealingFlag::AuctionNoEdit),
            Some("SUSPEND") => Some(DealingFlag::Suspend),
            Some(unknown) => return Err(format!("Unknown dealing flag: {unknown}")),
            None => None,
        };

        Ok(PriceFields {
            mid_open: parse_float("MID_OPEN")?,
            high: parse_float("HIGH")?,
            low: parse_float("LOW")?,
            bid: parse_float("BID")?,
            offer: parse_float("OFFER")?,
            change: parse_float("CHANGE")?,
            change_pct: parse_float("CHANGE_PCT")?,
            market_delay,
            market_state: get_field("MARKET_STATE"),
            update_time: get_field("UPDATE_TIME"),

            bid_quote_id: get_field("BIDQUOTEID"),
            ask_quote_id: get_field("ASKQUOTEID"),

            // Bid ladder prices
            bid_price1: parse_float("BIDPRICE1")?,
            bid_price2: parse_float("BIDPRICE2")?,
            bid_price3: parse_float("BIDPRICE3")?,
            bid_price4: parse_float("BIDPRICE4")?,
            bid_price5: parse_float("BIDPRICE5")?,

            // Ask ladder prices
            ask_price1: parse_float("ASKPRICE1")?,
            ask_price2: parse_float("ASKPRICE2")?,
            ask_price3: parse_float("ASKPRICE3")?,
            ask_price4: parse_float("ASKPRICE4")?,
            ask_price5: parse_float("ASKPRICE5")?,

            // Bid sizes
            bid_size1: parse_float("BIDSIZE1")?,
            bid_size2: parse_float("BIDSIZE2")?,
            bid_size3: parse_float("BIDSIZE3")?,
            bid_size4: parse_float("BIDSIZE4")?,
            bid_size5: parse_float("BIDSIZE5")?,

            // Ask sizes
            ask_size1: parse_float("ASKSIZE1")?,
            ask_size2: parse_float("ASKSIZE2")?,
            ask_size3: parse_float("ASKSIZE3")?,
            ask_size4: parse_float("ASKSIZE4")?,
            ask_size5: parse_float("ASKSIZE5")?,

            // Currencies
            currency0: get_field("CURRENCY0"),
            currency1: get_field("CURRENCY1"),
            currency2: get_field("CURRENCY2"),
            currency3: get_field("CURRENCY3"),
            currency4: get_field("CURRENCY4"),
            currency5: get_field("CURRENCY5"),

            // Bid size thresholds (expanded 1..5 for C1..C5)
            c1_bid_size_1: parse_float("C1BIDSIZE1")?,
            c1_bid_size_2: parse_float("C1BIDSIZE2")?,
            c1_bid_size_3: parse_float("C1BIDSIZE3")?,
            c1_bid_size_4: parse_float("C1BIDSIZE4")?,
            c1_bid_size_5: parse_float("C1BIDSIZE5")?,

            c2_bid_size_1: parse_float("C2BIDSIZE1")?,
            c2_bid_size_2: parse_float("C2BIDSIZE2")?,
            c2_bid_size_3: parse_float("C2BIDSIZE3")?,
            c2_bid_size_4: parse_float("C2BIDSIZE4")?,
            c2_bid_size_5: parse_float("C2BIDSIZE5")?,

            c3_bid_size_1: parse_float("C3BIDSIZE1")?,
            c3_bid_size_2: parse_float("C3BIDSIZE2")?,
            c3_bid_size_3: parse_float("C3BIDSIZE3")?,
            c3_bid_size_4: parse_float("C3BIDSIZE4")?,
            c3_bid_size_5: parse_float("C3BIDSIZE5")?,

            c4_bid_size_1: parse_float("C4BIDSIZE1")?,
            c4_bid_size_2: parse_float("C4BIDSIZE2")?,
            c4_bid_size_3: parse_float("C4BIDSIZE3")?,
            c4_bid_size_4: parse_float("C4BIDSIZE4")?,
            c4_bid_size_5: parse_float("C4BIDSIZE5")?,

            c5_bid_size_1: parse_float("C5BIDSIZE1")?,
            c5_bid_size_2: parse_float("C5BIDSIZE2")?,
            c5_bid_size_3: parse_float("C5BIDSIZE3")?,
            c5_bid_size_4: parse_float("C5BIDSIZE4")?,
            c5_bid_size_5: parse_float("C5BIDSIZE5")?,

            // Ask size thresholds (expanded 1..5 for C1..C5)
            c1_ask_size_1: parse_float("C1ASKSIZE1")?,
            c1_ask_size_2: parse_float("C1ASKSIZE2")?,
            c1_ask_size_3: parse_float("C1ASKSIZE3")?,
            c1_ask_size_4: parse_float("C1ASKSIZE4")?,
            c1_ask_size_5: parse_float("C1ASKSIZE5")?,

            c2_ask_size_1: parse_float("C2ASKSIZE1")?,
            c2_ask_size_2: parse_float("C2ASKSIZE2")?,
            c2_ask_size_3: parse_float("C2ASKSIZE3")?,
            c2_ask_size_4: parse_float("C2ASKSIZE4")?,
            c2_ask_size_5: parse_float("C2ASKSIZE5")?,

            c3_ask_size_1: parse_float("C3ASKSIZE1")?,
            c3_ask_size_2: parse_float("C3ASKSIZE2")?,
            c3_ask_size_3: parse_float("C3ASKSIZE3")?,
            c3_ask_size_4: parse_float("C3ASKSIZE4")?,
            c3_ask_size_5: parse_float("C3ASKSIZE5")?,

            c4_ask_size_1: parse_float("C4ASKSIZE1")?,
            c4_ask_size_2: parse_float("C4ASKSIZE2")?,
            c4_ask_size_3: parse_float("C4ASKSIZE3")?,
            c4_ask_size_4: parse_float("C4ASKSIZE4")?,
            c4_ask_size_5: parse_float("C4ASKSIZE5")?,

            c5_ask_size_1: parse_float("C5ASKSIZE1")?,
            c5_ask_size_2: parse_float("C5ASKSIZE2")?,
            c5_ask_size_3: parse_float("C5ASKSIZE3")?,
            c5_ask_size_4: parse_float("C5ASKSIZE4")?,
            c5_ask_size_5: parse_float("C5ASKSIZE5")?,

            timestamp: parse_int("TIMESTAMP")?,
            dealing_flag,
            net_chg: parse_float("NET_CHG")?,
            net_chg_pct: parse_float("NET_CHG_PCT")?,
            delay: parse_float("DELAY")?,
        })
    }
}
