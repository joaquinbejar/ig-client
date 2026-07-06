/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 25/10/25
******************************************************************************/

//! Streaming data field definitions for IG Markets API.
//!
//! This module provides enums and helper functions for working with streaming
//! subscriptions in the IG Markets API. It includes field definitions for:
//! - Market data (prices, market state)
//! - Price data (detailed bid/ask levels)
//! - Account data (P&L, margin, equity)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::{Debug, Display};

/// Streaming market fields available for market subscriptions.
///
/// These fields represent the various market data points that can be subscribed to
/// in the IG Markets streaming API for market updates.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamingMarketField {
    /// Mid open price
    MidOpen,
    /// High price
    High,
    /// Low price
    Low,
    /// Price change
    Change,
    /// Percentage change
    ChangePct,
    /// Last update time
    UpdateTime,
    /// Market delay in milliseconds
    MarketDelay,
    /// Market state (e.g., TRADEABLE, CLOSED)
    MarketState,
    /// Bid price
    Bid,
    /// Offer/Ask price
    #[default]
    Offer,
}

impl Debug for StreamingMarketField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let field_name = match self {
            StreamingMarketField::MidOpen => "MID_OPEN",
            StreamingMarketField::High => "HIGH",
            StreamingMarketField::Low => "LOW",
            StreamingMarketField::Change => "CHANGE",
            StreamingMarketField::ChangePct => "CHANGE_PCT",
            StreamingMarketField::UpdateTime => "UPDATE_TIME",
            StreamingMarketField::MarketDelay => "MARKET_DELAY",
            StreamingMarketField::MarketState => "MARKET_STATE",
            StreamingMarketField::Bid => "BID",
            StreamingMarketField::Offer => "OFFER",
        };
        write!(f, "{}", field_name)
    }
}

impl Display for StreamingMarketField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Constructs a vector of serialized streaming market field names from a given set of `StreamingMarketField`.
///
/// # Arguments
///
/// * `fields` - A reference to a `HashSet` containing `StreamingMarketField` items that need to be serialized.
///
/// # Returns
///
/// A `Vec<String>` where each `String` is the exact IG Lightstreamer wire name
/// of a `StreamingMarketField` from the input set.
pub(crate) fn get_streaming_market_fields(fields: &HashSet<StreamingMarketField>) -> Vec<String> {
    // `Display` (via the `Debug` impl above) emits the exact IG Lightstreamer
    // wire field name for every variant, so no fallible serialization is needed.
    fields.iter().map(|field| field.to_string()).collect()
}

/// Streaming price fields available for price subscriptions.
///
/// These fields represent the various price data points that can be subscribed to
/// in the IG Markets streaming API for price updates.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum StreamingPriceField {
    /// Mid open price
    #[serde(rename = "MID_OPEN")]
    MidOpen,
    /// High price
    High,
    /// Low price
    Low,
    /// Bid quote ID
    BidQuoteId,
    /// Ask quote ID
    AskQuoteId,
    /// Bid price level 1
    BidPrice1,
    /// Bid price level 2
    BidPrice2,
    /// Bid price level 3
    BidPrice3,
    /// Bid price level 4
    BidPrice4,
    /// Bid price level 5
    BidPrice5,
    /// Ask price level 1
    AskPrice1,
    /// Ask price level 2
    AskPrice2,
    /// Ask price level 3
    AskPrice3,
    /// Ask price level 4
    AskPrice4,
    /// Ask price level 5
    #[default]
    AskPrice5,
    /// Bid size level 1
    BidSize1,
    /// Bid size level 2
    BidSize2,
    /// Bid size level 3
    BidSize3,
    /// Bid size level 4
    BidSize4,
    /// Bid size level 5
    BidSize5,
    /// Ask size level 1
    AskSize1,
    /// Ask size level 2
    AskSize2,
    /// Ask size level 3
    AskSize3,
    /// Ask size level 4
    AskSize4,
    /// Ask size level 5
    AskSize5,
    /// Currency 0
    Currency0,
    /// Currency 1
    Currency1,
    /// Currency 1 bid size level 1
    C1BidSize1,
    /// Currency 1 bid size level 2
    C1BidSize2,
    /// Currency 1 bid size level 3
    C1BidSize3,
    /// Currency 1 bid size level 4
    C1BidSize4,
    /// Currency 1 bid size level 5
    C1BidSize5,
    /// Currency 1 ask size level 1
    C1AskSize1,
    /// Currency 1 ask size level 2
    C1AskSize2,
    /// Currency 1 ask size level 3
    C1AskSize3,
    /// Currency 1 ask size level 4
    C1AskSize4,
    /// Currency 1 ask size level 5
    C1AskSize5,
    /// Currency 2
    Currency2,
    /// Currency 2 bid size level 1
    C2BidSize1,
    /// Currency 2 bid size level 2
    C2BidSize2,
    /// Currency 2 bid size level 3
    C2BidSize3,
    /// Currency 2 bid size level 4
    C2BidSize4,
    /// Currency 2 bid size level 5
    C2BidSize5,
    /// Currency 2 ask size level 1
    C2AskSize1,
    /// Currency 2 ask size level 2
    C2AskSize2,
    /// Currency 2 ask size level 3
    C2AskSize3,
    /// Currency 2 ask size level 4
    C2AskSize4,
    /// Currency 2 ask size level 5
    C2AskSize5,
    /// Currency 3
    Currency3,
    /// Currency 3 bid size level 1
    C3BidSize1,
    /// Currency 3 bid size level 2
    C3BidSize2,
    /// Currency 3 bid size level 3
    C3BidSize3,
    /// Currency 3 bid size level 4
    C3BidSize4,
    /// Currency 3 bid size level 5
    C3BidSize5,
    /// Currency 3 ask size level 1
    C3AskSize1,
    /// Currency 3 ask size level 2
    C3AskSize2,
    /// Currency 3 ask size level 3
    C3AskSize3,
    /// Currency 3 ask size level 4
    C3AskSize4,
    /// Currency 3 ask size level 5
    C3AskSize5,
    /// Currency 4
    Currency4,
    /// Currency 4 bid size level 1
    C4BidSize1,
    /// Currency 4 bid size level 2
    C4BidSize2,
    /// Currency 4 bid size level 3
    C4BidSize3,
    /// Currency 4 bid size level 4
    C4BidSize4,
    /// Currency 4 bid size level 5
    C4BidSize5,
    /// Currency 4 ask size level 1
    C4AskSize1,
    /// Currency 4 ask size level 2
    C4AskSize2,
    /// Currency 4 ask size level 3
    C4AskSize3,
    /// Currency 4 ask size level 4
    C4AskSize4,
    /// Currency 4 ask size level 5
    C4AskSize5,
    /// Currency 5
    Currency5,
    /// Currency 5 bid size level 1
    C5BidSize1,
    /// Currency 5 bid size level 2
    C5BidSize2,
    /// Currency 5 bid size level 3
    C5BidSize3,
    /// Currency 5 bid size level 4
    C5BidSize4,
    /// Currency 5 bid size level 5
    C5BidSize5,
    /// Currency 5 ask size level 1
    C5AskSize1,
    /// Currency 5 ask size level 2
    C5AskSize2,
    /// Currency 5 ask size level 3
    C5AskSize3,
    /// Currency 5 ask size level 4
    C5AskSize4,
    /// Currency 5 ask size level 5
    C5AskSize5,
    /// Timestamp of the price update
    Timestamp,
    /// Dealing flag
    #[serde(rename = "DLG_FLAG")]
    DlgFlag,
    /// Price change versus open
    #[serde(rename = "NET_CHG")]
    NetChg,
    /// Percentage change versus open
    #[serde(rename = "NET_CHG_PCT")]
    NetChgPct,
    /// Delayed price flag (0 = false, 1 = true)
    Delay,
}

impl Debug for StreamingPriceField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let field_name = match self {
            StreamingPriceField::MidOpen => "MID_OPEN",
            StreamingPriceField::High => "HIGH",
            StreamingPriceField::Low => "LOW",
            StreamingPriceField::BidQuoteId => "BIDQUOTEID",
            StreamingPriceField::AskQuoteId => "ASKQUOTEID",
            StreamingPriceField::BidPrice1 => "BIDPRICE1",
            StreamingPriceField::BidPrice2 => "BIDPRICE2",
            StreamingPriceField::BidPrice3 => "BIDPRICE3",
            StreamingPriceField::BidPrice4 => "BIDPRICE4",
            StreamingPriceField::BidPrice5 => "BIDPRICE5",
            StreamingPriceField::AskPrice1 => "ASKPRICE1",
            StreamingPriceField::AskPrice2 => "ASKPRICE2",
            StreamingPriceField::AskPrice3 => "ASKPRICE3",
            StreamingPriceField::AskPrice4 => "ASKPRICE4",
            StreamingPriceField::AskPrice5 => "ASKPRICE5",
            StreamingPriceField::BidSize1 => "BIDSIZE1",
            StreamingPriceField::BidSize2 => "BIDSIZE2",
            StreamingPriceField::BidSize3 => "BIDSIZE3",
            StreamingPriceField::BidSize4 => "BIDSIZE4",
            StreamingPriceField::BidSize5 => "BIDSIZE5",
            StreamingPriceField::AskSize1 => "ASKSIZE1",
            StreamingPriceField::AskSize2 => "ASKSIZE2",
            StreamingPriceField::AskSize3 => "ASKSIZE3",
            StreamingPriceField::AskSize4 => "ASKSIZE4",
            StreamingPriceField::AskSize5 => "ASKSIZE5",
            StreamingPriceField::Currency0 => "CURRENCY0",
            StreamingPriceField::Currency1 => "CURRENCY1",
            StreamingPriceField::C1BidSize1 => "C1BIDSIZE1",
            StreamingPriceField::C1BidSize2 => "C1BIDSIZE2",
            StreamingPriceField::C1BidSize3 => "C1BIDSIZE3",
            StreamingPriceField::C1BidSize4 => "C1BIDSIZE4",
            StreamingPriceField::C1BidSize5 => "C1BIDSIZE5",
            StreamingPriceField::C1AskSize1 => "C1ASKSIZE1",
            StreamingPriceField::C1AskSize2 => "C1ASKSIZE2",
            StreamingPriceField::C1AskSize3 => "C1ASKSIZE3",
            StreamingPriceField::C1AskSize4 => "C1ASKSIZE4",
            StreamingPriceField::C1AskSize5 => "C1ASKSIZE5",
            StreamingPriceField::Currency2 => "CURRENCY2",
            StreamingPriceField::C2BidSize1 => "C2BIDSIZE1",
            StreamingPriceField::C2BidSize2 => "C2BIDSIZE2",
            StreamingPriceField::C2BidSize3 => "C2BIDSIZE3",
            StreamingPriceField::C2BidSize4 => "C2BIDSIZE4",
            StreamingPriceField::C2BidSize5 => "C2BIDSIZE5",
            StreamingPriceField::C2AskSize1 => "C2ASKSIZE1",
            StreamingPriceField::C2AskSize2 => "C2ASKSIZE2",
            StreamingPriceField::C2AskSize3 => "C2ASKSIZE3",
            StreamingPriceField::C2AskSize4 => "C2ASKSIZE4",
            StreamingPriceField::C2AskSize5 => "C2ASKSIZE5",
            StreamingPriceField::Currency3 => "CURRENCY3",
            StreamingPriceField::C3BidSize1 => "C3BIDSIZE1",
            StreamingPriceField::C3BidSize2 => "C3BIDSIZE2",
            StreamingPriceField::C3BidSize3 => "C3BIDSIZE3",
            StreamingPriceField::C3BidSize4 => "C3BIDSIZE4",
            StreamingPriceField::C3BidSize5 => "C3BIDSIZE5",
            StreamingPriceField::C3AskSize1 => "C3ASKSIZE1",
            StreamingPriceField::C3AskSize2 => "C3ASKSIZE2",
            StreamingPriceField::C3AskSize3 => "C3ASKSIZE3",
            StreamingPriceField::C3AskSize4 => "C3ASKSIZE4",
            StreamingPriceField::C3AskSize5 => "C3ASKSIZE5",
            StreamingPriceField::Currency4 => "CURRENCY4",
            StreamingPriceField::C4BidSize1 => "C4BIDSIZE1",
            StreamingPriceField::C4BidSize2 => "C4BIDSIZE2",
            StreamingPriceField::C4BidSize3 => "C4BIDSIZE3",
            StreamingPriceField::C4BidSize4 => "C4BIDSIZE4",
            StreamingPriceField::C4BidSize5 => "C4BIDSIZE5",
            StreamingPriceField::C4AskSize1 => "C4ASKSIZE1",
            StreamingPriceField::C4AskSize2 => "C4ASKSIZE2",
            StreamingPriceField::C4AskSize3 => "C4ASKSIZE3",
            StreamingPriceField::C4AskSize4 => "C4ASKSIZE4",
            StreamingPriceField::C4AskSize5 => "C4ASKSIZE5",
            StreamingPriceField::Currency5 => "CURRENCY5",
            StreamingPriceField::C5BidSize1 => "C5BIDSIZE1",
            StreamingPriceField::C5BidSize2 => "C5BIDSIZE2",
            StreamingPriceField::C5BidSize3 => "C5BIDSIZE3",
            StreamingPriceField::C5BidSize4 => "C5BIDSIZE4",
            StreamingPriceField::C5BidSize5 => "C5BIDSIZE5",
            StreamingPriceField::C5AskSize1 => "C5ASKSIZE1",
            StreamingPriceField::C5AskSize2 => "C5ASKSIZE2",
            StreamingPriceField::C5AskSize3 => "C5ASKSIZE3",
            StreamingPriceField::C5AskSize4 => "C5ASKSIZE4",
            StreamingPriceField::C5AskSize5 => "C5ASKSIZE5",
            StreamingPriceField::Timestamp => "TIMESTAMP",
            StreamingPriceField::DlgFlag => "DLG_FLAG",
            StreamingPriceField::NetChg => "NET_CHG",
            StreamingPriceField::NetChgPct => "NET_CHG_PCT",
            StreamingPriceField::Delay => "DELAY",
        };
        write!(f, "{}", field_name)
    }
}

impl Display for StreamingPriceField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Constructs a vector of serialized streaming price field names from a given set of `StreamingPriceField`.
///
/// # Arguments
///
/// * `fields` - A reference to a `HashSet` containing `StreamingPriceField` items that need to be serialized.
///
/// # Returns
///
/// A `Vec<String>` where each `String` is the exact IG Lightstreamer wire name
/// of a `StreamingPriceField` from the input set.
pub(crate) fn get_streaming_price_fields(fields: &HashSet<StreamingPriceField>) -> Vec<String> {
    // `Display` (via the `Debug` impl above) is the single source of truth for
    // the exact IG Lightstreamer wire field name of every variant, so no
    // separate mapping table or fallible serialization is needed.
    fields.iter().map(|field| field.to_string()).collect()
}

/// Streaming account data fields available for account subscriptions.
///
/// These fields represent the various account data points that can be subscribed to
/// in the IG Markets streaming API for account updates.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamingAccountDataField {
    /// Profit and loss
    #[default]
    Pnl,
    /// Deposit amount
    Deposit,
    /// Available cash
    AvailableCash,
    /// Profit and loss for long positions with guaranteed stops
    PnlLr,
    /// Profit and loss for long positions without guaranteed stops
    PnlNlr,
    /// Total funds
    Funds,
    /// Total margin
    Margin,
    /// Margin for positions with guaranteed stops
    MarginLr,
    /// Margin for positions without guaranteed stops
    MarginNlr,
    /// Available amount to deal
    AvailableToDeal,
    /// Total equity
    Equity,
    /// Equity used
    EquityUsed,
}

impl Debug for StreamingAccountDataField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let field_name = match self {
            StreamingAccountDataField::Pnl => "PNL",
            StreamingAccountDataField::Deposit => "DEPOSIT",
            StreamingAccountDataField::AvailableCash => "AVAILABLE_CASH",
            StreamingAccountDataField::PnlLr => "PNL_LR",
            StreamingAccountDataField::PnlNlr => "PNL_NLR",
            StreamingAccountDataField::Funds => "FUNDS",
            StreamingAccountDataField::Margin => "MARGIN",
            StreamingAccountDataField::MarginLr => "MARGIN_LR",
            StreamingAccountDataField::MarginNlr => "MARGIN_NLR",
            StreamingAccountDataField::AvailableToDeal => "AVAILABLE_TO_DEAL",
            StreamingAccountDataField::Equity => "EQUITY",
            StreamingAccountDataField::EquityUsed => "EQUITY_USED",
        };
        write!(f, "{}", field_name)
    }
}

impl Display for StreamingAccountDataField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Constructs a vector of serialized streaming account data field names from a given set of `StreamingAccountDataField`.
///
/// # Arguments
///
/// * `fields` - A reference to a `HashSet` containing `StreamingAccountDataField` items that need to be serialized.
///
/// # Returns
///
/// A `Vec<String>` where each `String` is the exact IG Lightstreamer wire name
/// of a `StreamingAccountDataField` from the input set.
pub(crate) fn get_streaming_account_data_fields(
    fields: &HashSet<StreamingAccountDataField>,
) -> Vec<String> {
    // `Display` (via the `Debug` impl above) emits the exact IG Lightstreamer
    // wire field name for every variant, so no fallible serialization is needed.
    fields.iter().map(|field| field.to_string()).collect()
}

/// Streaming chart fields available for chart subscriptions (tick and candle).
///
/// These fields represent both tick-level and aggregated (candle) chart data
/// provided by the IG Markets Lightstreamer streaming API.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamingChartField {
    // Common fields (available in both tick and candle data)
    /// Last traded volume for the period (tick or candle)
    #[default]
    Ltv,
    /// Incremental trading volume since last update
    Ttv,
    /// Update time as milliseconds from the Epoch
    Utm,
    /// Mid-market price at the start of the day
    DayOpenMid,
    /// Change from day's opening mid price to current mid price
    DayNetChgMid,
    /// Daily percentage change in mid price
    DayPercChgMid,
    /// Highest mid price for the day
    DayHigh,
    /// Lowest mid price for the day
    DayLow,

    // Tick-only fields (DISTINCT mode, CHART:{epic}:TICK)
    /// Current bid price
    Bid,
    /// Current offer/ask price
    Ofr,
    /// Last traded price
    Ltp,

    // Candle-only fields (MERGE mode, CHART:{epic}:{scale})
    /// Candle open price (offer)
    OfrOpen,
    /// Candle high price (offer)
    OfrHigh,
    /// Candle low price (offer)
    OfrLow,
    /// Candle closing price (offer)
    OfrClose,
    /// Candle open price (bid)
    BidOpen,
    /// Candle high price (bid)
    BidHigh,
    /// Candle low price (bid)
    BidLow,
    /// Candle closing price (bid)
    BidClose,
    /// Candle open price (last traded price)
    LtpOpen,
    /// Candle high price (last traded price)
    LtpHigh,
    /// Candle low price (last traded price)
    LtpLow,
    /// Candle closing price (last traded price)
    LtpClose,
    /// Indicator that candle ended (1 when candle ends, 0 otherwise)
    ConsEnd,
    /// Number of ticks consolidated in the candle
    ConsTickCount,
}

impl std::fmt::Debug for StreamingChartField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            StreamingChartField::Ltv => "LTV",
            StreamingChartField::Ttv => "TTV",
            StreamingChartField::Utm => "UTM",
            StreamingChartField::DayOpenMid => "DAY_OPEN_MID",
            StreamingChartField::DayNetChgMid => "DAY_NET_CHG_MID",
            StreamingChartField::DayPercChgMid => "DAY_PERC_CHG_MID",
            StreamingChartField::DayHigh => "DAY_HIGH",
            StreamingChartField::DayLow => "DAY_LOW",

            StreamingChartField::Bid => "BID",
            StreamingChartField::Ofr => "OFR",
            StreamingChartField::Ltp => "LTP",

            StreamingChartField::OfrOpen => "OFR_OPEN",
            StreamingChartField::OfrHigh => "OFR_HIGH",
            StreamingChartField::OfrLow => "OFR_LOW",
            StreamingChartField::OfrClose => "OFR_CLOSE",
            StreamingChartField::BidOpen => "BID_OPEN",
            StreamingChartField::BidHigh => "BID_HIGH",
            StreamingChartField::BidLow => "BID_LOW",
            StreamingChartField::BidClose => "BID_CLOSE",
            StreamingChartField::LtpOpen => "LTP_OPEN",
            StreamingChartField::LtpHigh => "LTP_HIGH",
            StreamingChartField::LtpLow => "LTP_LOW",
            StreamingChartField::LtpClose => "LTP_CLOSE",
            StreamingChartField::ConsEnd => "CONS_END",
            StreamingChartField::ConsTickCount => "CONS_TICK_COUNT",
        };
        write!(f, "{}", name)
    }
}

impl std::fmt::Display for StreamingChartField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Constructs a vector of serialized streaming chart field names from a given set of `StreamingChartField`.
///
/// # Arguments
///
/// * `fields` - A reference to a `HashSet` containing `StreamingChartField` items.
///
/// # Returns
///
/// A `Vec<String>` of exact IG Lightstreamer wire field names for
/// subscriptions.
pub(crate) fn get_streaming_chart_fields(fields: &HashSet<StreamingChartField>) -> Vec<String> {
    // `Display` (via the `Debug` impl above) emits the exact IG Lightstreamer
    // wire field name for every variant, so no fallible serialization is needed.
    fields.iter().map(|field| field.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_market_field_default() {
        let field = StreamingMarketField::default();
        assert_eq!(field, StreamingMarketField::Offer);
    }

    #[test]
    fn test_streaming_market_field_debug() {
        assert_eq!(format!("{:?}", StreamingMarketField::Bid), "BID");
        assert_eq!(format!("{:?}", StreamingMarketField::Offer), "OFFER");
        assert_eq!(format!("{:?}", StreamingMarketField::High), "HIGH");
        assert_eq!(format!("{:?}", StreamingMarketField::Low), "LOW");
        assert_eq!(format!("{:?}", StreamingMarketField::Change), "CHANGE");
        assert_eq!(
            format!("{:?}", StreamingMarketField::ChangePct),
            "CHANGE_PCT"
        );
        assert_eq!(
            format!("{:?}", StreamingMarketField::UpdateTime),
            "UPDATE_TIME"
        );
        assert_eq!(
            format!("{:?}", StreamingMarketField::MarketDelay),
            "MARKET_DELAY"
        );
        assert_eq!(
            format!("{:?}", StreamingMarketField::MarketState),
            "MARKET_STATE"
        );
        assert_eq!(format!("{:?}", StreamingMarketField::MidOpen), "MID_OPEN");
    }

    #[test]
    fn test_streaming_market_field_display() {
        assert_eq!(format!("{}", StreamingMarketField::Bid), "BID");
        assert_eq!(format!("{}", StreamingMarketField::Offer), "OFFER");
    }

    #[test]
    fn test_get_streaming_market_fields_empty() {
        let fields: HashSet<StreamingMarketField> = HashSet::new();
        let result = get_streaming_market_fields(&fields);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_streaming_market_fields_single() {
        let mut fields = HashSet::new();
        fields.insert(StreamingMarketField::Bid);
        let result = get_streaming_market_fields(&fields);
        assert_eq!(result.len(), 1);
        assert!(result.contains(&"BID".to_string()));
    }

    #[test]
    fn test_get_streaming_market_fields_multiple() {
        let mut fields = HashSet::new();
        fields.insert(StreamingMarketField::Bid);
        fields.insert(StreamingMarketField::Offer);
        fields.insert(StreamingMarketField::High);
        let result = get_streaming_market_fields(&fields);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"BID".to_string()));
        assert!(result.contains(&"OFFER".to_string()));
        assert!(result.contains(&"HIGH".to_string()));
    }

    #[test]
    fn test_get_streaming_account_data_fields_wire_names() {
        // Pins the exact IG Lightstreamer wire names produced by the getter
        // after replacing the serde_json round-trip with Display, so a future
        // divergence between Display and the wire contract fails loudly.
        let mut fields = HashSet::new();
        fields.insert(StreamingAccountDataField::Pnl);
        fields.insert(StreamingAccountDataField::AvailableCash);
        fields.insert(StreamingAccountDataField::PnlLr);
        let result = get_streaming_account_data_fields(&fields);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"PNL".to_string()));
        assert!(result.contains(&"AVAILABLE_CASH".to_string()));
        assert!(result.contains(&"PNL_LR".to_string()));
    }

    #[test]
    fn test_streaming_price_field_default() {
        let field = StreamingPriceField::default();
        assert_eq!(field, StreamingPriceField::AskPrice5);
    }

    #[test]
    fn test_streaming_price_field_debug() {
        assert_eq!(format!("{:?}", StreamingPriceField::High), "HIGH");
        assert_eq!(format!("{:?}", StreamingPriceField::Low), "LOW");
        assert_eq!(format!("{:?}", StreamingPriceField::MidOpen), "MID_OPEN");
        assert_eq!(format!("{:?}", StreamingPriceField::BidPrice1), "BIDPRICE1");
        assert_eq!(format!("{:?}", StreamingPriceField::AskPrice1), "ASKPRICE1");
    }

    #[test]
    fn test_streaming_price_field_display() {
        assert_eq!(format!("{}", StreamingPriceField::High), "HIGH");
        assert_eq!(format!("{}", StreamingPriceField::BidPrice1), "BIDPRICE1");
    }

    #[test]
    fn test_streaming_account_field_default() {
        let field = StreamingAccountDataField::default();
        assert_eq!(field, StreamingAccountDataField::Pnl);
    }

    #[test]
    fn test_streaming_account_field_debug() {
        assert_eq!(format!("{:?}", StreamingAccountDataField::Pnl), "PNL");
        assert_eq!(
            format!("{:?}", StreamingAccountDataField::Deposit),
            "DEPOSIT"
        );
        assert_eq!(format!("{:?}", StreamingAccountDataField::Margin), "MARGIN");
        assert_eq!(format!("{:?}", StreamingAccountDataField::Equity), "EQUITY");
    }

    #[test]
    fn test_streaming_account_field_display() {
        assert_eq!(format!("{}", StreamingAccountDataField::Pnl), "PNL");
        assert_eq!(format!("{}", StreamingAccountDataField::Equity), "EQUITY");
    }

    #[test]
    fn test_get_streaming_account_fields_empty() {
        let fields: HashSet<StreamingAccountDataField> = HashSet::new();
        let result = get_streaming_account_data_fields(&fields);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_streaming_account_fields_multiple() {
        let mut fields = HashSet::new();
        fields.insert(StreamingAccountDataField::Pnl);
        fields.insert(StreamingAccountDataField::Equity);
        let result = get_streaming_account_data_fields(&fields);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"PNL".to_string()));
        assert!(result.contains(&"EQUITY".to_string()));
    }

    #[test]
    fn test_streaming_chart_field_default() {
        let field = StreamingChartField::default();
        assert_eq!(field, StreamingChartField::Ltv);
    }

    #[test]
    fn test_streaming_chart_field_debug() {
        assert_eq!(format!("{:?}", StreamingChartField::Bid), "BID");
        assert_eq!(format!("{:?}", StreamingChartField::Ofr), "OFR");
        assert_eq!(format!("{:?}", StreamingChartField::Ltp), "LTP");
        assert_eq!(format!("{:?}", StreamingChartField::Ltv), "LTV");
        assert_eq!(format!("{:?}", StreamingChartField::Utm), "UTM");
        assert_eq!(format!("{:?}", StreamingChartField::DayHigh), "DAY_HIGH");
        assert_eq!(format!("{:?}", StreamingChartField::DayLow), "DAY_LOW");
    }

    #[test]
    fn test_streaming_chart_field_display() {
        assert_eq!(format!("{}", StreamingChartField::Bid), "BID");
        assert_eq!(format!("{}", StreamingChartField::Ofr), "OFR");
    }

    #[test]
    fn test_get_streaming_chart_fields_empty() {
        let fields: HashSet<StreamingChartField> = HashSet::new();
        let result = get_streaming_chart_fields(&fields);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_streaming_chart_fields_multiple() {
        let mut fields = HashSet::new();
        fields.insert(StreamingChartField::Bid);
        fields.insert(StreamingChartField::Ofr);
        fields.insert(StreamingChartField::Ltp);
        let result = get_streaming_chart_fields(&fields);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"BID".to_string()));
        assert!(result.contains(&"OFR".to_string()));
        assert!(result.contains(&"LTP".to_string()));
    }

    #[test]
    fn test_streaming_market_field_serialization() {
        let field = StreamingMarketField::Bid;
        let json = serde_json::to_string(&field).expect("serialize failed");
        assert_eq!(json, "\"BID\"");

        let deserialized: StreamingMarketField =
            serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized, StreamingMarketField::Bid);
    }

    #[test]
    fn test_streaming_account_field_serialization() {
        let field = StreamingAccountDataField::Pnl;
        let json = serde_json::to_string(&field).expect("serialize failed");
        assert_eq!(json, "\"PNL\"");

        let deserialized: StreamingAccountDataField =
            serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized, StreamingAccountDataField::Pnl);
    }

    #[test]
    fn test_streaming_chart_field_serialization() {
        let field = StreamingChartField::Bid;
        let json = serde_json::to_string(&field).expect("serialize failed");
        assert_eq!(json, "\"BID\"");

        let deserialized: StreamingChartField =
            serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized, StreamingChartField::Bid);
    }

    #[test]
    fn test_streaming_market_field_equality() {
        let field1 = StreamingMarketField::Bid;
        let field2 = StreamingMarketField::Bid;
        let field3 = StreamingMarketField::Offer;

        assert_eq!(field1, field2);
        assert_ne!(field1, field3);
    }

    #[test]
    fn test_streaming_market_field_hash() {
        let mut set = HashSet::new();
        set.insert(StreamingMarketField::Bid);
        set.insert(StreamingMarketField::Bid); // Duplicate

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_streaming_market_field_clone() {
        let field = StreamingMarketField::High;
        let cloned = field.clone();
        assert_eq!(field, cloned);
    }

    // Exhaustive wire-name pinning for every enum variant. Each variant's
    // JSON serialization, `Debug`, and `Display` must all agree on the exact
    // IG Lightstreamer field name, so any future rename fails loudly. This is
    // the coverage previously carried by `tests/unit/model/test_streaming.rs`.
    fn assert_wire_name<T>(field: &T, expected: &str)
    where
        T: Serialize + Debug + Display,
    {
        let serialized = serde_json::to_string(field).expect("serialize failed");
        assert_eq!(serialized, format!("\"{expected}\""));
        assert_eq!(format!("{field:?}"), expected);
        assert_eq!(format!("{field}"), expected);
    }

    #[test]
    fn test_streaming_market_field_all_variants_wire_names() {
        for (field, expected) in [
            (StreamingMarketField::MidOpen, "MID_OPEN"),
            (StreamingMarketField::High, "HIGH"),
            (StreamingMarketField::Low, "LOW"),
            (StreamingMarketField::Change, "CHANGE"),
            (StreamingMarketField::ChangePct, "CHANGE_PCT"),
            (StreamingMarketField::UpdateTime, "UPDATE_TIME"),
            (StreamingMarketField::MarketDelay, "MARKET_DELAY"),
            (StreamingMarketField::MarketState, "MARKET_STATE"),
            (StreamingMarketField::Bid, "BID"),
            (StreamingMarketField::Offer, "OFFER"),
        ] {
            assert_wire_name(&field, expected);
        }
    }

    #[test]
    fn test_streaming_account_field_all_variants_wire_names() {
        for (field, expected) in [
            (StreamingAccountDataField::Pnl, "PNL"),
            (StreamingAccountDataField::Deposit, "DEPOSIT"),
            (StreamingAccountDataField::AvailableCash, "AVAILABLE_CASH"),
            (StreamingAccountDataField::PnlLr, "PNL_LR"),
            (StreamingAccountDataField::PnlNlr, "PNL_NLR"),
            (StreamingAccountDataField::Funds, "FUNDS"),
            (StreamingAccountDataField::Margin, "MARGIN"),
            (StreamingAccountDataField::MarginLr, "MARGIN_LR"),
            (StreamingAccountDataField::MarginNlr, "MARGIN_NLR"),
            (
                StreamingAccountDataField::AvailableToDeal,
                "AVAILABLE_TO_DEAL",
            ),
            (StreamingAccountDataField::Equity, "EQUITY"),
            (StreamingAccountDataField::EquityUsed, "EQUITY_USED"),
        ] {
            assert_wire_name(&field, expected);
        }
    }

    #[test]
    fn test_streaming_price_field_special_variants_wire_names() {
        // Pins the variants whose wire names are non-obvious: explicit renames
        // (MID_OPEN, DLG_FLAG) and the UPPERCASE-run collapses (BIDQUOTEID,
        // BIDPRICE1, C1BIDSIZE1, ...).
        for (field, expected) in [
            (StreamingPriceField::MidOpen, "MID_OPEN"),
            (StreamingPriceField::High, "HIGH"),
            (StreamingPriceField::Low, "LOW"),
            (StreamingPriceField::BidQuoteId, "BIDQUOTEID"),
            (StreamingPriceField::AskQuoteId, "ASKQUOTEID"),
            (StreamingPriceField::BidPrice1, "BIDPRICE1"),
            (StreamingPriceField::AskPrice5, "ASKPRICE5"),
            (StreamingPriceField::Currency0, "CURRENCY0"),
            (StreamingPriceField::C1BidSize1, "C1BIDSIZE1"),
            (StreamingPriceField::C5AskSize5, "C5ASKSIZE5"),
            (StreamingPriceField::Timestamp, "TIMESTAMP"),
            (StreamingPriceField::DlgFlag, "DLG_FLAG"),
        ] {
            assert_wire_name(&field, expected);
        }
    }

    #[test]
    fn test_streaming_chart_field_all_variants_wire_names() {
        for (field, expected) in [
            (StreamingChartField::Ltv, "LTV"),
            (StreamingChartField::Ttv, "TTV"),
            (StreamingChartField::Utm, "UTM"),
            (StreamingChartField::DayOpenMid, "DAY_OPEN_MID"),
            (StreamingChartField::DayNetChgMid, "DAY_NET_CHG_MID"),
            (StreamingChartField::DayPercChgMid, "DAY_PERC_CHG_MID"),
            (StreamingChartField::DayHigh, "DAY_HIGH"),
            (StreamingChartField::DayLow, "DAY_LOW"),
            (StreamingChartField::Bid, "BID"),
            (StreamingChartField::Ofr, "OFR"),
            (StreamingChartField::Ltp, "LTP"),
            (StreamingChartField::OfrOpen, "OFR_OPEN"),
            (StreamingChartField::OfrHigh, "OFR_HIGH"),
            (StreamingChartField::OfrLow, "OFR_LOW"),
            (StreamingChartField::OfrClose, "OFR_CLOSE"),
            (StreamingChartField::BidOpen, "BID_OPEN"),
            (StreamingChartField::BidHigh, "BID_HIGH"),
            (StreamingChartField::BidLow, "BID_LOW"),
            (StreamingChartField::BidClose, "BID_CLOSE"),
            (StreamingChartField::LtpOpen, "LTP_OPEN"),
            (StreamingChartField::LtpHigh, "LTP_HIGH"),
            (StreamingChartField::LtpLow, "LTP_LOW"),
            (StreamingChartField::LtpClose, "LTP_CLOSE"),
            (StreamingChartField::ConsEnd, "CONS_END"),
            (StreamingChartField::ConsTickCount, "CONS_TICK_COUNT"),
        ] {
            assert_wire_name(&field, expected);
        }
    }

    /// Every variant of every streaming field enum, so drift between the serde
    /// renames and the `Display`/`Debug` wire mapping can be checked
    /// exhaustively (see [`test_all_streaming_fields_serde_matches_display`]).
    const ALL_MARKET_FIELDS: &[StreamingMarketField] = &[
        StreamingMarketField::MidOpen,
        StreamingMarketField::High,
        StreamingMarketField::Low,
        StreamingMarketField::Change,
        StreamingMarketField::ChangePct,
        StreamingMarketField::UpdateTime,
        StreamingMarketField::MarketDelay,
        StreamingMarketField::MarketState,
        StreamingMarketField::Bid,
        StreamingMarketField::Offer,
    ];

    const ALL_ACCOUNT_FIELDS: &[StreamingAccountDataField] = &[
        StreamingAccountDataField::Pnl,
        StreamingAccountDataField::Deposit,
        StreamingAccountDataField::AvailableCash,
        StreamingAccountDataField::PnlLr,
        StreamingAccountDataField::PnlNlr,
        StreamingAccountDataField::Funds,
        StreamingAccountDataField::Margin,
        StreamingAccountDataField::MarginLr,
        StreamingAccountDataField::MarginNlr,
        StreamingAccountDataField::AvailableToDeal,
        StreamingAccountDataField::Equity,
        StreamingAccountDataField::EquityUsed,
    ];

    const ALL_CHART_FIELDS: &[StreamingChartField] = &[
        StreamingChartField::Ltv,
        StreamingChartField::Ttv,
        StreamingChartField::Utm,
        StreamingChartField::DayOpenMid,
        StreamingChartField::DayNetChgMid,
        StreamingChartField::DayPercChgMid,
        StreamingChartField::DayHigh,
        StreamingChartField::DayLow,
        StreamingChartField::Bid,
        StreamingChartField::Ofr,
        StreamingChartField::Ltp,
        StreamingChartField::OfrOpen,
        StreamingChartField::OfrHigh,
        StreamingChartField::OfrLow,
        StreamingChartField::OfrClose,
        StreamingChartField::BidOpen,
        StreamingChartField::BidHigh,
        StreamingChartField::BidLow,
        StreamingChartField::BidClose,
        StreamingChartField::LtpOpen,
        StreamingChartField::LtpHigh,
        StreamingChartField::LtpLow,
        StreamingChartField::LtpClose,
        StreamingChartField::ConsEnd,
        StreamingChartField::ConsTickCount,
    ];

    const ALL_PRICE_FIELDS: &[StreamingPriceField] = &[
        StreamingPriceField::MidOpen,
        StreamingPriceField::High,
        StreamingPriceField::Low,
        StreamingPriceField::BidQuoteId,
        StreamingPriceField::AskQuoteId,
        StreamingPriceField::BidPrice1,
        StreamingPriceField::BidPrice2,
        StreamingPriceField::BidPrice3,
        StreamingPriceField::BidPrice4,
        StreamingPriceField::BidPrice5,
        StreamingPriceField::AskPrice1,
        StreamingPriceField::AskPrice2,
        StreamingPriceField::AskPrice3,
        StreamingPriceField::AskPrice4,
        StreamingPriceField::AskPrice5,
        StreamingPriceField::BidSize1,
        StreamingPriceField::BidSize2,
        StreamingPriceField::BidSize3,
        StreamingPriceField::BidSize4,
        StreamingPriceField::BidSize5,
        StreamingPriceField::AskSize1,
        StreamingPriceField::AskSize2,
        StreamingPriceField::AskSize3,
        StreamingPriceField::AskSize4,
        StreamingPriceField::AskSize5,
        StreamingPriceField::Currency0,
        StreamingPriceField::Currency1,
        StreamingPriceField::C1BidSize1,
        StreamingPriceField::C1BidSize2,
        StreamingPriceField::C1BidSize3,
        StreamingPriceField::C1BidSize4,
        StreamingPriceField::C1BidSize5,
        StreamingPriceField::C1AskSize1,
        StreamingPriceField::C1AskSize2,
        StreamingPriceField::C1AskSize3,
        StreamingPriceField::C1AskSize4,
        StreamingPriceField::C1AskSize5,
        StreamingPriceField::Currency2,
        StreamingPriceField::C2BidSize1,
        StreamingPriceField::C2BidSize2,
        StreamingPriceField::C2BidSize3,
        StreamingPriceField::C2BidSize4,
        StreamingPriceField::C2BidSize5,
        StreamingPriceField::C2AskSize1,
        StreamingPriceField::C2AskSize2,
        StreamingPriceField::C2AskSize3,
        StreamingPriceField::C2AskSize4,
        StreamingPriceField::C2AskSize5,
        StreamingPriceField::Currency3,
        StreamingPriceField::C3BidSize1,
        StreamingPriceField::C3BidSize2,
        StreamingPriceField::C3BidSize3,
        StreamingPriceField::C3BidSize4,
        StreamingPriceField::C3BidSize5,
        StreamingPriceField::C3AskSize1,
        StreamingPriceField::C3AskSize2,
        StreamingPriceField::C3AskSize3,
        StreamingPriceField::C3AskSize4,
        StreamingPriceField::C3AskSize5,
        StreamingPriceField::Currency4,
        StreamingPriceField::C4BidSize1,
        StreamingPriceField::C4BidSize2,
        StreamingPriceField::C4BidSize3,
        StreamingPriceField::C4BidSize4,
        StreamingPriceField::C4BidSize5,
        StreamingPriceField::C4AskSize1,
        StreamingPriceField::C4AskSize2,
        StreamingPriceField::C4AskSize3,
        StreamingPriceField::C4AskSize4,
        StreamingPriceField::C4AskSize5,
        StreamingPriceField::Currency5,
        StreamingPriceField::C5BidSize1,
        StreamingPriceField::C5BidSize2,
        StreamingPriceField::C5BidSize3,
        StreamingPriceField::C5BidSize4,
        StreamingPriceField::C5BidSize5,
        StreamingPriceField::C5AskSize1,
        StreamingPriceField::C5AskSize2,
        StreamingPriceField::C5AskSize3,
        StreamingPriceField::C5AskSize4,
        StreamingPriceField::C5AskSize5,
        StreamingPriceField::Timestamp,
        StreamingPriceField::DlgFlag,
        StreamingPriceField::NetChg,
        StreamingPriceField::NetChgPct,
        StreamingPriceField::Delay,
    ];

    /// Asserts that the serde wire name (the JSON string a variant serializes
    /// to) matches the variant's `Display` output. This is the single guard
    /// that keeps the serde renames and the `Display`/`Debug` mapping — the two
    /// independent sources for the IG Lightstreamer wire name — from drifting.
    fn assert_serde_matches_display<T>(field: &T)
    where
        T: Serialize + Display,
    {
        let value = serde_json::to_value(field).expect("serialize to value failed");
        let wire = value
            .as_str()
            .expect("a streaming field enum variant must serialize to a JSON string");
        assert_eq!(
            wire,
            field.to_string(),
            "serde wire name and Display disagree for a streaming field variant"
        );
    }

    #[test]
    fn test_all_streaming_fields_serde_matches_display() {
        for field in ALL_MARKET_FIELDS {
            assert_serde_matches_display(field);
        }
        for field in ALL_ACCOUNT_FIELDS {
            assert_serde_matches_display(field);
        }
        for field in ALL_CHART_FIELDS {
            assert_serde_matches_display(field);
        }
        for field in ALL_PRICE_FIELDS {
            assert_serde_matches_display(field);
        }
    }

    #[test]
    fn test_get_streaming_price_fields_uses_display_wire_names() {
        // The price getter now derives wire names from `Display`; pin a couple of
        // non-obvious ones so a regression to a divergent mapping fails loudly.
        let mut fields = HashSet::new();
        fields.insert(StreamingPriceField::MidOpen);
        fields.insert(StreamingPriceField::C1BidSize1);
        fields.insert(StreamingPriceField::DlgFlag);
        let result = get_streaming_price_fields(&fields);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"MID_OPEN".to_string()));
        assert!(result.contains(&"C1BIDSIZE1".to_string()));
        assert!(result.contains(&"DLG_FLAG".to_string()));
    }

    #[test]
    fn test_streaming_price_field_in_hashset() {
        let mut set = HashSet::new();
        set.insert(StreamingPriceField::MidOpen);
        set.insert(StreamingPriceField::High);
        set.insert(StreamingPriceField::MidOpen); // duplicate
        assert_eq!(set.len(), 2);
        assert!(set.contains(&StreamingPriceField::MidOpen));
        assert!(!set.contains(&StreamingPriceField::Low));
    }

    #[test]
    fn test_streaming_chart_field_in_hashset() {
        let mut set = HashSet::new();
        set.insert(StreamingChartField::Bid);
        set.insert(StreamingChartField::Ofr);
        set.insert(StreamingChartField::Bid); // duplicate
        assert_eq!(set.len(), 2);
        assert!(set.contains(&StreamingChartField::Bid));
        assert!(!set.contains(&StreamingChartField::Ltp));
    }

    #[test]
    fn test_streaming_account_field_in_hashset() {
        let mut set = HashSet::new();
        set.insert(StreamingAccountDataField::Pnl);
        set.insert(StreamingAccountDataField::Deposit);
        set.insert(StreamingAccountDataField::Pnl); // duplicate
        assert_eq!(set.len(), 2);
        assert!(set.contains(&StreamingAccountDataField::Pnl));
        assert!(!set.contains(&StreamingAccountDataField::AvailableCash));
    }
}
