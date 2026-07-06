use crate::presentation::instrument::InstrumentType;
use crate::presentation::market::MarketState;
use crate::presentation::order::{Direction, OrderType, Status, TimeInForce};
use crate::presentation::serialization::string_as_float_opt;
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Add;

/// Returns `true` if an instrument name denotes a call option.
///
/// IG encodes the option right in the human-readable `instrumentName` (for
/// example `"...CALL..."`). The check is a case-sensitive substring match on
/// the upper-case token IG uses. Shared by every `is_call` accessor in this
/// module so the classification stays in one place.
#[must_use]
#[inline]
fn is_call_name(name: &str) -> bool {
    name.contains("CALL")
}

/// Returns `true` if an instrument name denotes a put option.
///
/// Counterpart to [`is_call_name`]: a case-sensitive substring match on the
/// upper-case `"PUT"` token IG places in `instrumentName`. Shared by every
/// `is_put` accessor in this module.
#[must_use]
#[inline]
fn is_put_name(name: &str) -> bool {
    name.contains("PUT")
}

/// Account information
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct AccountInfo {
    /// List of accounts owned by the user
    pub accounts: Vec<Account>,
}

/// Details of a specific account
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct Account {
    /// Unique identifier for the account
    #[serde(rename = "accountId")]
    pub account_id: String,
    /// Name of the account
    #[serde(rename = "accountName")]
    pub account_name: String,
    /// Type of the account (e.g., CFD, Spread bet)
    #[serde(rename = "accountType")]
    pub account_type: String,
    /// Balance information for the account
    pub balance: AccountBalance,
    /// Base currency of the account
    pub currency: String,
    /// Current status of the account
    pub status: String,
    /// Whether this is the preferred account
    pub preferred: bool,
}

/// Account balance information
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct AccountBalance {
    /// Total balance of the account
    pub balance: f64,
    /// Deposit amount
    pub deposit: f64,
    /// Current profit or loss
    #[serde(rename = "profitLoss")]
    pub profit_loss: f64,
    /// Available funds for trading
    pub available: f64,
}

/// Metadata for activity pagination
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct ActivityMetadata {
    /// Paging information
    pub paging: Option<ActivityPaging>,
}

/// Paging information for activities
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct ActivityPaging {
    /// Number of items per page
    pub size: Option<i64>,
    /// URL for the next page of results
    pub next: Option<String>,
}

/// Type of account activity
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, DisplaySimple, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActivityType {
    /// Activity related to editing stop and limit orders
    EditStopAndLimit,
    /// Activity related to positions
    Position,
    /// System-generated activity
    System,
    /// Activity related to working orders
    WorkingOrder,
}

/// Individual activity record
#[derive(Debug, Clone, DisplaySimple, Deserialize, Serialize)]
pub struct Activity {
    /// Date and time of the activity
    pub date: String,
    /// Unique identifier for the deal
    #[serde(rename = "dealId", default)]
    pub deal_id: Option<String>,
    /// Instrument EPIC identifier
    #[serde(default)]
    pub epic: Option<String>,
    /// Time period of the activity
    #[serde(default)]
    pub period: Option<String>,
    /// Client-generated reference for the deal
    #[serde(rename = "dealReference", default)]
    pub deal_reference: Option<String>,
    /// Type of activity
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    /// Status of the activity
    #[serde(default)]
    pub status: Option<Status>,
    /// Description of the activity
    #[serde(default)]
    pub description: Option<String>,
    /// Additional details about the activity
    /// This is a string when detailed=false, and an object when detailed=true
    #[serde(default)]
    pub details: Option<ActivityDetails>,
    /// Channel the activity occurred on (e.g., "WEB" or "Mobile")
    #[serde(default)]
    pub channel: Option<String>,
    /// The currency, e.g., a pound symbol
    #[serde(default)]
    pub currency: Option<String>,
    /// Price level
    #[serde(default)]
    pub level: Option<String>,
}

/// Detailed information about an activity
/// Only available when using the detailed=true parameter
#[derive(Debug, Clone, DisplaySimple, Deserialize, Serialize)]
pub struct ActivityDetails {
    /// Client-generated reference for the deal
    #[serde(rename = "dealReference", default)]
    pub deal_reference: Option<String>,
    /// List of actions associated with this activity
    #[serde(default)]
    pub actions: Vec<ActivityAction>,
    /// Name of the market
    #[serde(rename = "marketName", default)]
    pub market_name: Option<String>,
    /// Date until which the order is valid
    #[serde(rename = "goodTillDate", default)]
    pub good_till_date: Option<String>,
    /// Currency of the transaction
    #[serde(default)]
    pub currency: Option<String>,
    /// Size/quantity of the transaction
    #[serde(default)]
    pub size: Option<f64>,
    /// Direction of the transaction (BUY or SELL)
    #[serde(default)]
    pub direction: Option<Direction>,
    /// Price level
    #[serde(default)]
    pub level: Option<f64>,
    /// Stop level price
    #[serde(rename = "stopLevel", default)]
    pub stop_level: Option<f64>,
    /// Distance for the stop
    #[serde(rename = "stopDistance", default)]
    pub stop_distance: Option<f64>,
    /// Whether the stop is guaranteed
    #[serde(rename = "guaranteedStop", default)]
    pub guaranteed_stop: Option<bool>,
    /// Distance for the trailing stop
    #[serde(rename = "trailingStopDistance", default)]
    pub trailing_stop_distance: Option<f64>,
    /// Step size for the trailing stop
    #[serde(rename = "trailingStep", default)]
    pub trailing_step: Option<f64>,
    /// Limit level price
    #[serde(rename = "limitLevel", default)]
    pub limit_level: Option<f64>,
    /// Distance for the limit
    #[serde(rename = "limitDistance", default)]
    pub limit_distance: Option<f64>,
}

/// Types of actions that can be performed on an activity
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, DisplaySimple, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    /// A limit order was deleted
    LimitOrderDeleted,
    /// A limit order was filled
    LimitOrderFilled,
    /// A limit order was opened
    LimitOrderOpened,
    /// A limit order was rolled
    LimitOrderRolled,
    /// A position was closed
    PositionClosed,
    /// A position was deleted
    PositionDeleted,
    /// A position was opened
    PositionOpened,
    /// A position was partially closed
    PositionPartiallyClosed,
    /// A position was rolled
    PositionRolled,
    /// A stop/limit was amended
    StopLimitAmended,
    /// A stop order was amended
    StopOrderAmended,
    /// A stop order was deleted
    StopOrderDeleted,
    /// A stop order was filled
    StopOrderFilled,
    /// A stop order was opened
    StopOrderOpened,
    /// A stop order was rolled
    StopOrderRolled,
    /// Unknown action type
    Unknown,
    /// A working order was deleted
    WorkingOrderDeleted,
}

/// Action associated with an activity
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityAction {
    /// Type of action
    pub action_type: ActionType,
    /// Deal ID affected by this action
    pub affected_deal_id: Option<String>,
}

/// Individual position
#[derive(DebugPretty, Clone, DisplaySimple, Serialize, Deserialize)]
pub struct Position {
    /// Details of the position
    pub position: PositionDetails,
    /// Market information for the position
    pub market: PositionMarket,
    /// Profit and loss for the position
    pub pnl: Option<f64>,
}

impl Position {
    /// Calculates the profit and loss (PnL) for the current position
    /// of a trader.
    ///
    /// The method determines PnL based on whether it is already cached
    /// (`self.pnl`) or needs to be calculated from the position and
    /// market details.
    ///
    /// # Returns
    ///
    /// A floating-point value that represents the PnL for the position.
    /// Positive values indicate a profit, and negative values indicate a loss.
    ///
    /// # Logic
    ///
    /// - If `self.pnl` is set, the cached value is returned directly.
    /// - Otherwise it delegates to [`Position::pnl_checked`], which marks a Buy
    ///   against `market.bid` and a Sell against `market.offer`. When the
    ///   direction's market price is unavailable there is no basis for a P&L, so
    ///   this returns `0.0` (use [`Position::pnl_checked`] to distinguish the
    ///   unknown case as `None`).
    ///
    #[must_use]
    pub fn pnl(&self) -> f64 {
        self.pnl
            .unwrap_or_else(|| self.pnl_checked().unwrap_or(0.0))
    }

    /// Computes P&L from current market prices, or `None` when the required
    /// price for the position's direction is unavailable.
    ///
    /// This is the single source of truth for position P&L: a Buy is marked
    /// against `market.bid`, a Sell against `market.offer`, as
    /// `(price - level) * size` (sign flipped for a Sell). When that price is
    /// missing there is no basis for a P&L, so this returns `None` rather than
    /// fabricating a value — callers that need a number use `pnl()`, which
    /// treats the unknown case as `0.0`. Unlike the pre-`pnl()` field override,
    /// this ignores any cached `self.pnl`.
    ///
    /// # Returns
    /// `Some(pnl)` when the direction's market price is present, else `None`.
    #[must_use]
    pub fn pnl_checked(&self) -> Option<f64> {
        let current_price = match self.position.direction {
            Direction::Buy => self.market.bid?,
            Direction::Sell => self.market.offer?,
        };
        let price_diff = match self.position.direction {
            Direction::Buy => current_price - self.position.level,
            Direction::Sell => self.position.level - current_price,
        };
        Some(price_diff * self.position.size)
    }

    /// Updates the cached profit and loss (PnL) for the current position from
    /// current market prices.
    ///
    /// Delegates to [`Position::pnl_checked`] (a Buy marked against `market.bid`,
    /// a Sell against `market.offer`) and stores the result in `self.pnl`. When
    /// the direction's market price is unavailable there is no basis for a P&L,
    /// so `0.0` is stored.
    ///
    /// # Fields Updated
    /// - `self.pnl`: set to `Some(pnl)`, or `Some(0.0)` when the required market
    ///   price is missing.
    ///
    pub fn update_pnl(&mut self) {
        // Store the market-derived P&L (0.0 when the direction's price is
        // unavailable), through the single `pnl_checked` implementation.
        self.pnl = Some(self.pnl_checked().unwrap_or(0.0));
    }
}

impl Position {
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
        is_call_name(&self.market.instrument_name)
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
        is_put_name(&self.market.instrument_name)
    }
}

impl Add for Position {
    type Output = Position;

    /// Adds two positions together.
    ///
    /// # Invariants
    /// Both positions must belong to the same market (same EPIC).
    /// In debug builds, adding positions from different markets will panic.
    /// In release builds, the left-hand side market is used.
    fn add(self, other: Position) -> Position {
        debug_assert_eq!(
            self.market.epic, other.market.epic,
            "cannot add positions from different markets"
        );
        Position {
            position: self.position + other.position,
            market: self.market,
            pnl: match (self.pnl, other.pnl) {
                (Some(a), Some(b)) => Some(a + b),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
        }
    }
}

/// Details of a position
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct PositionDetails {
    /// Size of one contract
    #[serde(rename = "contractSize")]
    pub contract_size: f64,
    /// Date and time when the position was created
    #[serde(rename = "createdDate")]
    pub created_date: String,
    /// UTC date and time when the position was created
    #[serde(rename = "createdDateUTC")]
    pub created_date_utc: String,
    /// Unique identifier for the deal
    #[serde(rename = "dealId")]
    pub deal_id: String,
    /// Client-generated reference for the deal
    #[serde(rename = "dealReference")]
    pub deal_reference: String,
    /// Direction of the position (buy or sell)
    pub direction: Direction,
    /// Price level for take profit
    #[serde(rename = "limitLevel")]
    pub limit_level: Option<f64>,
    /// Opening price level of the position
    pub level: f64,
    /// Size/quantity of the position
    pub size: f64,
    /// Price level for stop loss
    #[serde(rename = "stopLevel")]
    pub stop_level: Option<f64>,
    /// Step size for trailing stop
    #[serde(rename = "trailingStep")]
    pub trailing_step: Option<f64>,
    /// Distance for trailing stop
    #[serde(rename = "trailingStopDistance")]
    pub trailing_stop_distance: Option<f64>,
    /// Currency of the position
    pub currency: String,
    /// Whether the position has controlled risk
    #[serde(rename = "controlledRisk")]
    pub controlled_risk: bool,
    /// Premium paid for limited risk
    #[serde(rename = "limitedRiskPremium")]
    pub limited_risk_premium: Option<f64>,
}

impl Add for PositionDetails {
    type Output = PositionDetails;

    fn add(self, other: PositionDetails) -> PositionDetails {
        let (contract_size, size, direction) = if self.direction != other.direction {
            // Netting opposite directions: the surviving position takes the
            // direction of the larger size (e.g. Buy 2 + Sell 3 -> Sell 1). On a
            // tie the net is flat and the direction is immaterial; keep `self`.
            let direction = if other.size > self.size {
                other.direction
            } else {
                self.direction
            };
            (
                (self.contract_size - other.contract_size).abs(),
                (self.size - other.size).abs(),
                direction,
            )
        } else {
            (
                self.contract_size + other.contract_size,
                self.size + other.size,
                self.direction,
            )
        };

        PositionDetails {
            contract_size,
            created_date: self.created_date,
            created_date_utc: self.created_date_utc,
            deal_id: self.deal_id,
            deal_reference: self.deal_reference,
            direction,
            limit_level: other.limit_level.or(self.limit_level),
            level: (self.level + other.level) / 2.0, // Average level
            size,
            stop_level: other.stop_level.or(self.stop_level),
            trailing_step: other.trailing_step.or(self.trailing_step),
            trailing_stop_distance: other.trailing_stop_distance.or(self.trailing_stop_distance),
            currency: self.currency.clone(),
            controlled_risk: self.controlled_risk || other.controlled_risk,
            limited_risk_premium: other.limited_risk_premium.or(self.limited_risk_premium),
        }
    }
}

/// Market information for a position
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct PositionMarket {
    /// Human-readable name of the instrument
    #[serde(rename = "instrumentName")]
    pub instrument_name: String,
    /// Expiry date of the instrument
    pub expiry: String,
    /// Unique identifier for the market
    pub epic: String,
    /// Type of the instrument
    #[serde(rename = "instrumentType")]
    pub instrument_type: InstrumentType,
    /// Size of one lot
    #[serde(rename = "lotSize")]
    pub lot_size: f64,
    /// Highest price of the current trading session
    pub high: Option<f64>,
    /// Lowest price of the current trading session
    pub low: Option<f64>,
    /// Percentage change in price since previous close
    #[serde(rename = "percentageChange")]
    pub percentage_change: f64,
    /// Net change in price since previous close
    #[serde(rename = "netChange")]
    pub net_change: f64,
    /// Current bid price
    pub bid: Option<f64>,
    /// Current offer/ask price
    pub offer: Option<f64>,
    /// Time of the last price update
    #[serde(rename = "updateTime")]
    pub update_time: String,
    /// UTC time of the last price update
    #[serde(rename = "updateTimeUTC")]
    pub update_time_utc: String,
    /// Delay time in minutes for market data
    #[serde(rename = "delayTime")]
    pub delay_time: i64,
    /// Whether streaming prices are available for this market
    #[serde(rename = "streamingPricesAvailable")]
    pub streaming_prices_available: bool,
    /// Current status of the market (e.g., `TRADEABLE`, `CLOSED`)
    #[serde(rename = "marketStatus")]
    pub market_status: MarketState,
    /// Factor for scaling prices
    #[serde(rename = "scalingFactor")]
    pub scaling_factor: i64,
}

impl PositionMarket {
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
        is_call_name(&self.instrument_name)
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
        is_put_name(&self.instrument_name)
    }
}

/// Working order
#[derive(DebugPretty, Clone, DisplaySimple, Deserialize, Serialize)]
pub struct WorkingOrder {
    /// Details of the working order
    #[serde(rename = "workingOrderData")]
    pub working_order_data: WorkingOrderData,
    /// Market information for the working order
    #[serde(rename = "marketData")]
    pub market_data: AccountMarketData,
}

/// Details of a working order
#[derive(DebugPretty, Clone, DisplaySimple, Deserialize, Serialize)]
pub struct WorkingOrderData {
    /// Unique identifier for the deal
    #[serde(rename = "dealId")]
    pub deal_id: String,
    /// Direction of the order (buy or sell)
    pub direction: Direction,
    /// Instrument EPIC identifier
    pub epic: String,
    /// Size/quantity of the order
    #[serde(rename = "orderSize")]
    pub order_size: f64,
    /// Price level for the order
    #[serde(rename = "orderLevel")]
    pub order_level: f64,
    /// Time in force for the order
    #[serde(rename = "timeInForce")]
    pub time_in_force: TimeInForce,
    /// Expiry date for GTD orders
    #[serde(rename = "goodTillDate")]
    pub good_till_date: Option<String>,
    /// ISO formatted expiry date for GTD orders
    #[serde(rename = "goodTillDateISO")]
    pub good_till_date_iso: Option<String>,
    /// Date and time when the order was created
    #[serde(rename = "createdDate")]
    pub created_date: String,
    /// UTC date and time when the order was created
    #[serde(rename = "createdDateUTC")]
    pub created_date_utc: String,
    /// Whether the order has a guaranteed stop
    #[serde(rename = "guaranteedStop")]
    pub guaranteed_stop: bool,
    /// Type of the order
    #[serde(rename = "orderType")]
    pub order_type: OrderType,
    /// Distance for stop loss
    #[serde(rename = "stopDistance")]
    pub stop_distance: Option<f64>,
    /// Distance for take profit
    #[serde(rename = "limitDistance")]
    pub limit_distance: Option<f64>,
    /// Currency code for the order
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    /// Whether direct market access is enabled
    pub dma: bool,
    /// Premium for limited risk
    #[serde(rename = "limitedRiskPremium")]
    pub limited_risk_premium: Option<f64>,
    /// Price level for take profit
    #[serde(rename = "limitLevel", default)]
    pub limit_level: Option<f64>,
    /// Price level for stop loss
    #[serde(rename = "stopLevel", default)]
    pub stop_level: Option<f64>,
    /// Client-generated reference for the deal
    #[serde(rename = "dealReference", default)]
    pub deal_reference: Option<String>,
}

/// Market data for a working order
#[derive(DebugPretty, Clone, DisplaySimple, Deserialize, Serialize)]
pub struct AccountMarketData {
    /// Human-readable name of the instrument
    #[serde(rename = "instrumentName")]
    pub instrument_name: String,
    /// Exchange identifier
    #[serde(rename = "exchangeId")]
    pub exchange_id: String,
    /// Expiry date of the instrument
    pub expiry: String,
    /// Current status of the market
    #[serde(rename = "marketStatus")]
    pub market_status: MarketState,
    /// Unique identifier for the market
    pub epic: String,
    /// Type of the instrument
    #[serde(rename = "instrumentType")]
    pub instrument_type: InstrumentType,
    /// Size of one lot
    #[serde(rename = "lotSize")]
    pub lot_size: f64,
    /// Highest price of the current trading session
    pub high: Option<f64>,
    /// Lowest price of the current trading session
    pub low: Option<f64>,
    /// Percentage change in price since previous close
    #[serde(rename = "percentageChange")]
    pub percentage_change: f64,
    /// Net change in price since previous close
    #[serde(rename = "netChange")]
    pub net_change: f64,
    /// Current bid price
    pub bid: Option<f64>,
    /// Current offer/ask price
    pub offer: Option<f64>,
    /// Time of the last price update
    #[serde(rename = "updateTime")]
    pub update_time: String,
    /// UTC time of the last price update
    #[serde(rename = "updateTimeUTC")]
    pub update_time_utc: String,
    /// Delay time in minutes for market data
    #[serde(rename = "delayTime")]
    pub delay_time: i64,
    /// Whether streaming prices are available for this market
    #[serde(rename = "streamingPricesAvailable")]
    pub streaming_prices_available: bool,
    /// Factor for scaling prices
    #[serde(rename = "scalingFactor")]
    pub scaling_factor: i64,
}

impl AccountMarketData {
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
        is_call_name(&self.instrument_name)
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
        is_put_name(&self.instrument_name)
    }
}

/// Transaction metadata
#[derive(DebugPretty, Clone, DisplaySimple, Deserialize, Serialize)]
pub struct TransactionMetadata {
    /// Pagination information
    #[serde(rename = "pageData")]
    pub page_data: PageData,
    /// Total number of transactions
    pub size: i64,
}

/// Pagination information
#[derive(DebugPretty, Clone, DisplaySimple, Deserialize, Serialize)]
pub struct PageData {
    /// Current page number
    #[serde(rename = "pageNumber")]
    pub page_number: i64,
    /// Number of items per page
    #[serde(rename = "pageSize")]
    pub page_size: i64,
    /// Total number of pages
    #[serde(rename = "totalPages")]
    pub total_pages: i64,
}

/// Individual transaction
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct AccountTransaction {
    /// Date and time of the transaction
    pub date: String,
    /// UTC date and time of the transaction
    #[serde(rename = "dateUtc")]
    pub date_utc: String,
    /// Represents the date and time in UTC when an event or entity was opened or initiated.
    #[serde(rename = "openDateUtc")]
    pub open_date_utc: String,
    /// Name of the instrument
    #[serde(rename = "instrumentName")]
    pub instrument_name: String,
    /// Time period of the transaction
    pub period: String,
    /// Profit or loss amount
    #[serde(rename = "profitAndLoss")]
    pub profit_and_loss: String,
    /// Type of transaction
    #[serde(rename = "transactionType")]
    pub transaction_type: String,
    /// Reference identifier for the transaction
    pub reference: String,
    /// Opening price level
    #[serde(rename = "openLevel")]
    pub open_level: String,
    /// Closing price level
    #[serde(rename = "closeLevel")]
    pub close_level: String,
    /// Size/quantity of the transaction
    pub size: String,
    /// Currency of the transaction
    pub currency: String,
    /// Whether this is a cash transaction
    #[serde(rename = "cashTransaction")]
    pub cash_transaction: bool,
}

impl AccountTransaction {
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
        is_call_name(&self.instrument_name)
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
        is_put_name(&self.instrument_name)
    }
}

/// Representation of account data received from the IG Markets streaming API
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
pub struct AccountData {
    /// Name of the item this data belongs to
    pub item_name: String,
    /// Position of the item in the subscription
    pub item_pos: usize,
    /// All account fields
    pub fields: AccountFields,
    /// Fields that have changed in this update
    pub changed_fields: AccountFields,
    /// Whether this is a snapshot or an update
    pub is_snapshot: bool,
}

/// Fields containing account financial information
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default)]
pub struct AccountFields {
    #[serde(rename = "PNL")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pnl: Option<f64>,

    #[serde(rename = "DEPOSIT")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    deposit: Option<f64>,

    #[serde(rename = "AVAILABLE_CASH")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    available_cash: Option<f64>,

    #[serde(rename = "PNL_LR")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pnl_lr: Option<f64>,

    #[serde(rename = "PNL_NLR")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pnl_nlr: Option<f64>,

    #[serde(rename = "FUNDS")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    funds: Option<f64>,

    #[serde(rename = "MARGIN")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    margin: Option<f64>,

    #[serde(rename = "MARGIN_LR")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    margin_lr: Option<f64>,

    #[serde(rename = "MARGIN_NLR")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    margin_nlr: Option<f64>,

    #[serde(rename = "AVAILABLE_TO_DEAL")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    available_to_deal: Option<f64>,

    #[serde(rename = "EQUITY")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    equity: Option<f64>,

    #[serde(rename = "EQUITY_USED")]
    #[serde(with = "string_as_float_opt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    equity_used: Option<f64>,
}

impl AccountData {
    /// Builds an [`AccountData`] from pre-extracted streaming fields.
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
    /// * `Result<Self, String>` - The converted AccountData or an error message
    pub fn from_fields(
        item_name: Option<&str>,
        item_pos: usize,
        is_snapshot: bool,
        fields: &HashMap<String, Option<String>>,
        changed_fields: &HashMap<String, Option<String>>,
    ) -> Result<Self, String> {
        let fields = Self::create_account_fields(fields)?;
        let changed_fields = Self::create_account_fields(changed_fields)?;

        Ok(AccountData {
            item_name: item_name.unwrap_or_default().to_string(),
            item_pos,
            fields,
            changed_fields,
            is_snapshot,
        })
    }

    /// Helper method to create AccountFields from a HashMap of field values
    ///
    /// # Arguments
    /// * `fields_map` - HashMap containing field names and their string values
    ///
    /// # Returns
    /// * `Result<AccountFields, String>` - The parsed AccountFields or an error message
    fn create_account_fields(
        fields_map: &HashMap<String, Option<String>>,
    ) -> Result<AccountFields, String> {
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

        Ok(AccountFields {
            pnl: parse_float("PNL")?,
            deposit: parse_float("DEPOSIT")?,
            available_cash: parse_float("AVAILABLE_CASH")?,
            pnl_lr: parse_float("PNL_LR")?,
            pnl_nlr: parse_float("PNL_NLR")?,
            funds: parse_float("FUNDS")?,
            margin: parse_float("MARGIN")?,
            margin_lr: parse_float("MARGIN_LR")?,
            margin_nlr: parse_float("MARGIN_NLR")?,
            available_to_deal: parse_float("AVAILABLE_TO_DEAL")?,
            equity: parse_float("EQUITY")?,
            equity_used: parse_float("EQUITY_USED")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::order::Direction;
    use crate::utils::finance::calculate_pnl;

    fn sample_position_details(direction: Direction, level: f64, size: f64) -> PositionDetails {
        PositionDetails {
            contract_size: 1.0,
            created_date: "2025/10/30 18:13:53:000".to_string(),
            created_date_utc: "2025-10-30T17:13:53".to_string(),
            deal_id: "DIAAAAVJNQPWZAG".to_string(),
            deal_reference: "RZ0RQ1K8V1S1JN2".to_string(),
            direction,
            limit_level: None,
            level,
            size,
            stop_level: None,
            trailing_step: None,
            trailing_stop_distance: None,
            currency: "USD".to_string(),
            controlled_risk: false,
            limited_risk_premium: None,
        }
    }

    fn sample_market(bid: Option<f64>, offer: Option<f64>) -> PositionMarket {
        PositionMarket {
            instrument_name: "US 500 6910 PUT ($1)".to_string(),
            expiry: "DEC-25".to_string(),
            epic: "OP.D.OTCSPX3.6910P.IP".to_string(),
            instrument_type: InstrumentType::Unknown,
            lot_size: 1.0,
            high: Some(153.43),
            low: Some(147.42),
            percentage_change: 0.61,
            net_change: 6895.38,
            bid,
            offer,
            update_time: "05:55:59".to_string(),
            update_time_utc: "05:55:59".to_string(),
            delay_time: 0,
            streaming_prices_available: true,
            market_status: MarketState::Tradeable,
            scaling_factor: 1,
        }
    }

    #[test]
    fn pnl_sell_uses_offer_and_matches_sample_data() {
        // Given the provided sample data (SELL):
        // size = 1.0, level = 155.14, offer = 152.82
        // value = 155.14, current_value = 152.82 => pnl = 155.14 - 152.82 = 2.32
        let details = sample_position_details(Direction::Sell, 155.14, 1.0);
        let market = sample_market(Some(151.32), Some(152.82));
        let position = Position {
            position: details,
            market,
            pnl: None,
        };

        let pnl = position.pnl();
        assert!((pnl - 2.32).abs() < 1e-9, "expected 2.32, got {}", pnl);
    }

    #[test]
    fn pnl_buy_uses_bid_and_computes_difference() {
        // For BUY: pnl = current_value - value
        // Using size = 1.0, level = 155.14, bid = 151.32 => pnl = 151.32 - 155.14 = -3.82
        let details = sample_position_details(Direction::Buy, 155.14, 1.0);
        let market = sample_market(Some(151.32), Some(152.82));
        let position = Position {
            position: details,
            market,
            pnl: None,
        };

        let pnl = position.pnl();
        assert!((pnl + 3.82).abs() < 1e-9, "expected -3.82, got {}", pnl);
    }

    #[test]
    fn pnl_field_overrides_calculation_when_present() {
        let details = sample_position_details(Direction::Sell, 155.14, 1.0);
        let market = sample_market(Some(151.32), Some(152.82));
        // Set explicit pnl different from calculated (which would be 2.32)
        let position = Position {
            position: details,
            market,
            pnl: Some(10.0),
        };
        assert_eq!(position.pnl(), 10.0);
    }

    #[test]
    fn pnl_sell_is_zero_when_offer_missing() {
        // When offer is missing for SELL, unwrap_or(value) makes current_value == value => pnl = 0
        let details = sample_position_details(Direction::Sell, 155.14, 1.0);
        let market = sample_market(Some(151.32), None);
        let position = Position {
            position: details,
            market,
            pnl: None,
        };
        assert!((position.pnl() - 0.0).abs() < 1e-12);
    }

    #[test]
    fn pnl_buy_is_zero_when_bid_missing() {
        // When bid is missing for BUY, unwrap_or(value) makes current_value == value => pnl = 0
        let details = sample_position_details(Direction::Buy, 155.14, 1.0);
        let market = sample_market(None, Some(152.82));
        let position = Position {
            position: details,
            market,
            pnl: None,
        };
        assert!((position.pnl() - 0.0).abs() < 1e-12);
    }

    #[test]
    fn pnl_missing_price_with_size_two_is_zero_not_inflated() {
        // Regression: the old fallback substituted the notional (size*level) as
        // the price, giving size^2*level - size*level. With size != 1 that is
        // non-zero garbage; the correct answer with no market price is 0.
        for direction in [Direction::Buy, Direction::Sell] {
            let details = sample_position_details(direction, 155.14, 2.0);
            let market = sample_market(None, None);
            let position = Position {
                position: details,
                market,
                pnl: None,
            };
            assert!(position.pnl_checked().is_none());
            assert!(
                position.pnl().abs() < 1e-12,
                "size-2 missing-price pnl must be 0, got {}",
                position.pnl()
            );
        }
    }

    #[test]
    fn pnl_size_two_with_price_scales_with_size() {
        // Buy 2 @ 100, bid 105 => (105-100)*2 = 10.
        let details = sample_position_details(Direction::Buy, 100.0, 2.0);
        let market = sample_market(Some(105.0), None);
        let position = Position {
            position: details,
            market,
            pnl: None,
        };
        assert!((position.pnl() - 10.0).abs() < 1e-12);
        assert!((calculate_pnl(&position).expect("has bid") - 10.0).abs() < 1e-12);
    }

    #[test]
    fn netting_opposite_directions_takes_larger_side() {
        // Buy 2 + Sell 3 => Sell 1 (net short).
        let buy = sample_position_details(Direction::Buy, 100.0, 2.0);
        let sell = sample_position_details(Direction::Sell, 102.0, 3.0);
        let net = buy.clone() + sell.clone();
        assert_eq!(net.direction, Direction::Sell);
        assert!((net.size - 1.0).abs() < 1e-12);

        // Symmetric: Sell 3 + Buy 2 => Sell 1 as well.
        let net2 = sell + buy;
        assert_eq!(net2.direction, Direction::Sell);
        assert!((net2.size - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_transaction_metadata_deserialize_and_roundtrip() {
        // Sanitized `metadata` object from `history/transactions`. `size` and
        // all `pageData` fields are i64.
        let json = r#"{
            "pageData": { "pageNumber": 1, "pageSize": 20, "totalPages": 3 },
            "size": 42
        }"#;

        let meta: TransactionMetadata = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(meta.size, 42);
        assert_eq!(meta.page_data.page_number, 1);
        assert_eq!(meta.page_data.page_size, 20);
        assert_eq!(meta.page_data.total_pages, 3);

        let serialized = serde_json::to_string(&meta).expect("serialize failed");
        let re: TransactionMetadata =
            serde_json::from_str(&serialized).expect("re-deserialize failed");
        assert_eq!(re.size, 42);
        assert_eq!(re.page_data.total_pages, 3);
        assert!(serialized.contains("\"pageNumber\":1"));
        assert!(serialized.contains("\"pageSize\":20"));
        assert!(serialized.contains("\"totalPages\":3"));
    }

    #[test]
    fn test_activity_paging_deserialize_and_roundtrip() {
        // Sanitized `paging` object from `history/activity`. `size` is i64.
        let json = r#"{ "size": 10, "next": "/history/activity?from=X&to=Y" }"#;

        let paging: ActivityPaging = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(paging.size, Some(10));
        assert_eq!(
            paging.next.as_deref(),
            Some("/history/activity?from=X&to=Y")
        );

        let serialized = serde_json::to_string(&paging).expect("serialize failed");
        let re: ActivityPaging = serde_json::from_str(&serialized).expect("re-deserialize failed");
        assert_eq!(re.size, Some(10));
        assert_eq!(re.next.as_deref(), Some("/history/activity?from=X&to=Y"));
    }
}
