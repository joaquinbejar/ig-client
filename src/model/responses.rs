/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 19/10/25
******************************************************************************/
use crate::presentation::account::{
    Account, AccountTransaction, Activity, ActivityMetadata, Position, TransactionMetadata,
    WorkingOrder,
};
use crate::presentation::instrument::InstrumentType;
use crate::presentation::market::{
    Category, CategoryInstrument, CategoryInstrumentsMetadata, HistoricalPrice, MarketData,
    MarketDetails, MarketNavigationNode, MarketNode, PriceAllowance,
};
use crate::presentation::order::{Direction, Status};
use crate::utils::parsing::{deserialize_null_as_empty_vec, deserialize_nullable_status};
use chrono::{DateTime, Utc};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Database entry response for market instruments
#[derive(
    DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default,
)]
pub struct DBEntryResponse {
    /// The trading symbol identifier
    pub symbol: String,
    /// The Epic identifier used by the exchange
    pub epic: String,
    /// Human-readable name of the instrument
    pub name: String,
    /// Instrument type classification
    pub instrument_type: InstrumentType,
    /// The exchange where this instrument is traded
    pub exchange: String,
    /// Expiration date and time for the instrument
    pub expiry: String,
    /// Timestamp of the last update to this record
    pub last_update: DateTime<Utc>,
}

impl From<MarketNode> for DBEntryResponse {
    fn from(value: MarketNode) -> Self {
        let mut entry = DBEntryResponse::default();
        if !value.markets.is_empty() {
            let market = &value.markets[0];
            entry.symbol = market
                .epic
                .split('.')
                .nth(2)
                .unwrap_or_default()
                .to_string();
            entry.epic = market.epic.clone();
            entry.name = market.instrument_name.clone();
            entry.instrument_type = market.instrument_type;
            entry.exchange = "IG".to_string();
            entry.expiry = market.expiry.clone();
            entry.last_update = Utc::now();
        }
        entry
    }
}

impl From<MarketData> for DBEntryResponse {
    fn from(market: MarketData) -> Self {
        DBEntryResponse {
            symbol: market
                .epic
                .split('.')
                .nth(2)
                .unwrap_or_default()
                .to_string(),
            epic: market.epic.clone(),
            name: market.instrument_name.clone(),
            instrument_type: market.instrument_type,
            exchange: "IG".to_string(),
            expiry: market.expiry.clone(),
            last_update: Utc::now(),
        }
    }
}

impl From<&MarketNode> for DBEntryResponse {
    fn from(value: &MarketNode) -> Self {
        DBEntryResponse::from(value.clone())
    }
}

impl From<&MarketData> for DBEntryResponse {
    fn from(market: &MarketData) -> Self {
        DBEntryResponse::from(market.clone())
    }
}

/// Response containing multiple market details
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct MultipleMarketDetailsResponse {
    /// List of market details
    #[serde(rename = "marketDetails")]
    pub market_details: Vec<MarketDetails>,
}

impl MultipleMarketDetailsResponse {
    /// Returns the number of market details in the response
    ///
    /// # Returns
    /// Number of market details
    #[must_use]
    pub fn len(&self) -> usize {
        self.market_details.len()
    }

    /// Returns true if the response contains no market details
    ///
    /// # Returns
    /// True if empty, false otherwise
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.market_details.is_empty()
    }

    /// Returns a reference to the market details vector
    ///
    /// # Returns
    /// Reference to the vector of market details
    #[must_use]
    pub fn market_details(&self) -> &Vec<MarketDetails> {
        &self.market_details
    }

    /// Returns an iterator over the market details
    ///
    /// # Returns
    /// Iterator over market details
    pub fn iter(&self) -> impl Iterator<Item = &MarketDetails> {
        self.market_details.iter()
    }
}

/// Model for historical prices
#[derive(DebugPretty, Clone, Serialize, Deserialize)]
pub struct HistoricalPricesResponse {
    /// List of historical price points
    pub prices: Vec<HistoricalPrice>,
    /// Type of the instrument
    #[serde(rename = "instrumentType")]
    pub instrument_type: InstrumentType,
    /// API usage allowance information
    #[serde(rename = "allowance", skip_serializing_if = "Option::is_none", default)]
    pub allowance: Option<PriceAllowance>,
}

impl HistoricalPricesResponse {
    /// Returns the number of price points in the response
    ///
    /// # Returns
    /// Number of price points
    #[must_use]
    pub fn len(&self) -> usize {
        self.prices.len()
    }

    /// Returns true if the response contains no price points
    ///
    /// # Returns
    /// True if empty, false otherwise
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prices.is_empty()
    }

    /// Returns a reference to the prices vector
    ///
    /// # Returns
    /// Reference to the vector of historical prices
    #[must_use]
    pub fn prices(&self) -> &Vec<HistoricalPrice> {
        &self.prices
    }

    /// Returns an iterator over the prices
    ///
    /// # Returns
    /// Iterator over historical prices
    pub fn iter(&self) -> impl Iterator<Item = &HistoricalPrice> {
        self.prices.iter()
    }
}

/// Model for market search results
#[derive(DebugPretty, Clone, Serialize, Deserialize)]
pub struct MarketSearchResponse {
    /// List of markets matching the search criteria
    pub markets: Vec<MarketData>,
}

impl MarketSearchResponse {
    /// Returns the number of markets in the response
    ///
    /// # Returns
    /// Number of markets
    #[must_use]
    pub fn len(&self) -> usize {
        self.markets.len()
    }

    /// Returns true if the response contains no markets
    ///
    /// # Returns
    /// True if empty, false otherwise
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.markets.is_empty()
    }

    /// Returns a reference to the markets vector
    ///
    /// # Returns
    /// Reference to the vector of markets
    #[must_use]
    pub fn markets(&self) -> &Vec<MarketData> {
        &self.markets
    }

    /// Returns an iterator over the markets
    ///
    /// # Returns
    /// Iterator over markets
    pub fn iter(&self) -> impl Iterator<Item = &MarketData> {
        self.markets.iter()
    }
}

/// Response model for market navigation
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct MarketNavigationResponse {
    /// List of navigation nodes at the current level
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub nodes: Vec<MarketNavigationNode>,
    /// List of markets at the current level
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub markets: Vec<MarketData>,
}

/// Response containing all categories of instruments enabled for the IG account
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize, Default)]
pub struct CategoriesResponse {
    /// List of categories
    pub categories: Vec<Category>,
}

impl CategoriesResponse {
    /// Returns the number of categories in the response
    ///
    /// # Returns
    /// Number of categories
    #[must_use]
    pub fn len(&self) -> usize {
        self.categories.len()
    }

    /// Returns true if the response contains no categories
    ///
    /// # Returns
    /// True if empty, false otherwise
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }

    /// Returns a reference to the categories vector
    ///
    /// # Returns
    /// Reference to the vector of categories
    #[must_use]
    pub fn categories(&self) -> &Vec<Category> {
        &self.categories
    }

    /// Returns an iterator over the categories
    ///
    /// # Returns
    /// Iterator over categories
    pub fn iter(&self) -> impl Iterator<Item = &Category> {
        self.categories.iter()
    }
}

/// Response containing instruments for a specific category
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize, Default)]
pub struct CategoryInstrumentsResponse {
    /// List of instruments in the category
    pub instruments: Vec<CategoryInstrument>,
    /// Paging metadata
    pub metadata: Option<CategoryInstrumentsMetadata>,
}

impl CategoryInstrumentsResponse {
    /// Returns the number of instruments in the response
    ///
    /// # Returns
    /// Number of instruments
    #[must_use]
    pub fn len(&self) -> usize {
        self.instruments.len()
    }

    /// Returns true if the response contains no instruments
    ///
    /// # Returns
    /// True if empty, false otherwise
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instruments.is_empty()
    }

    /// Returns a reference to the instruments vector
    ///
    /// # Returns
    /// Reference to the vector of instruments
    #[must_use]
    pub fn instruments(&self) -> &Vec<CategoryInstrument> {
        &self.instruments
    }

    /// Returns an iterator over the instruments
    ///
    /// # Returns
    /// Iterator over instruments
    pub fn iter(&self) -> impl Iterator<Item = &CategoryInstrument> {
        self.instruments.iter()
    }
}

/// Response containing user accounts
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize, Default)]
pub struct AccountsResponse {
    /// List of accounts owned by the user
    pub accounts: Vec<Account>,
}

/// Open positions
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize, Default)]
pub struct PositionsResponse {
    /// List of open positions
    pub positions: Vec<Position>,
}

impl PositionsResponse {
    /// Compact positions by epic, combining positions with the same epic
    ///
    /// This method takes a vector of positions and returns a new vector where
    /// positions with the same epic have been combined into a single position.
    ///
    /// # Arguments
    /// * `positions` - A vector of positions to compact
    ///
    /// # Returns
    /// A vector of positions with unique epics
    #[must_use]
    pub fn compact_by_epic(positions: Vec<Position>) -> Vec<Position> {
        let mut epic_map: HashMap<String, Position> = std::collections::HashMap::new();

        for position in positions {
            let epic = position.market.epic.clone();
            epic_map
                .entry(epic)
                .and_modify(|existing| {
                    *existing = existing.clone() + position.clone();
                })
                .or_insert(position);
        }

        epic_map.into_values().collect()
    }
}

/// Working orders
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct WorkingOrdersResponse {
    /// List of pending working orders
    #[serde(rename = "workingOrders")]
    pub working_orders: Vec<WorkingOrder>,
}

/// Account activity
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct AccountActivityResponse {
    /// List of activities on the account
    pub activities: Vec<Activity>,
    /// Metadata about pagination
    pub metadata: Option<ActivityMetadata>,
}

/// Transaction history
#[derive(DebugPretty, DisplaySimple, Clone, Deserialize, Serialize)]
pub struct TransactionHistoryResponse {
    /// List of account transactions
    pub transactions: Vec<AccountTransaction>,
    /// Metadata about the transaction list
    pub metadata: TransactionMetadata,
}

/// Response to order creation
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct CreateOrderResponse {
    /// Client-generated reference for the deal
    #[serde(rename = "dealReference")]
    pub deal_reference: String,
}

/// Response to closing a position
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct ClosePositionResponse {
    /// Client-generated reference for the closing deal
    #[serde(rename = "dealReference")]
    pub deal_reference: String,
}

/// Response to updating a position
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct UpdatePositionResponse {
    /// Client-generated reference for the update deal
    #[serde(rename = "dealReference")]
    pub deal_reference: String,
}

/// Response to working order creation
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct CreateWorkingOrderResponse {
    /// Client-generated reference for the deal
    #[serde(rename = "dealReference")]
    pub deal_reference: String,
}

/// Outcome of a deal as reported by the order confirmation endpoint.
///
/// Returned by `GET /confirms/{dealReference}` in the top-level `dealStatus`
/// field, for which IG documents `ACCEPTED` and `REJECTED`. This is distinct
/// from [`AffectedDeal::status`], which reports a per-deal lifecycle status
/// (e.g. `FULLY_CLOSED`, `PARTIALLY_CLOSED`, `OPENED`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, DisplaySimple, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DealStatus {
    /// The deal was accepted by IG.
    Accepted,
    /// The deal was rejected by IG.
    Rejected,
}

/// A deal affected by an order confirmation.
///
/// IG returns one entry per deal touched by the confirmed order — for example
/// the individual deals partially closed to fill a closing order.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AffectedDeal {
    /// Identifier of the affected deal.
    #[serde(rename = "dealId")]
    pub deal_id: String,
    /// Per-deal lifecycle status — a different domain from the top-level
    /// [`DealStatus`]; IG returns values such as `FULLY_CLOSED`,
    /// `PARTIALLY_CLOSED` or `OPENED`. Kept as a `String` because the set is
    /// broader and less stable than the accept/reject outcome.
    #[serde(rename = "status")]
    pub status: String,
}

/// Details of a confirmed order
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize)]
pub struct OrderConfirmationResponse {
    /// Date and time of the confirmation
    pub date: String,
    /// Status of the order (accepted, rejected, etc.)
    /// This can be null in some responses (e.g., when market is closed)
    #[serde(deserialize_with = "deserialize_nullable_status")]
    pub status: Status,
    /// Reason for rejection if applicable
    pub reason: Option<String>,
    /// Unique identifier for the deal
    #[serde(rename = "dealId")]
    pub deal_id: Option<String>,
    /// Client-generated reference for the deal
    #[serde(rename = "dealReference")]
    pub deal_reference: String,
    /// Status of the deal (accepted or rejected)
    #[serde(rename = "dealStatus")]
    #[serde(default)]
    pub deal_status: Option<DealStatus>,
    /// Instrument EPIC identifier
    pub epic: Option<String>,
    /// Expiry date for the order
    #[serde(rename = "expiry")]
    pub expiry: Option<String>,
    /// Whether a guaranteed stop was used
    #[serde(rename = "guaranteedStop")]
    pub guaranteed_stop: Option<bool>,
    /// Price level of the order
    #[serde(rename = "level")]
    pub level: Option<f64>,
    /// Distance for take profit
    #[serde(rename = "limitDistance")]
    pub limit_distance: Option<f64>,
    /// Price level for take profit
    #[serde(rename = "limitLevel")]
    pub limit_level: Option<f64>,
    /// Size/quantity of the order
    pub size: Option<f64>,
    /// Distance for stop loss
    #[serde(rename = "stopDistance")]
    pub stop_distance: Option<f64>,
    /// Price level for stop loss
    #[serde(rename = "stopLevel")]
    pub stop_level: Option<f64>,
    /// Whether a trailing stop was used
    #[serde(rename = "trailingStop")]
    pub trailing_stop: Option<bool>,
    /// Direction of the order (buy or sell)
    pub direction: Option<Direction>,
    /// Deals affected by this confirmation (empty when IG omits the field)
    #[serde(rename = "affectedDeals")]
    #[serde(default)]
    pub affected_deals: Vec<AffectedDeal>,
    /// Realised profit or loss for the confirmed deal, in `profit_currency`
    #[serde(rename = "profit")]
    #[serde(default)]
    pub profit: Option<f64>,
    /// Currency in which `profit` is denominated (ISO code, for example `GBP`)
    #[serde(rename = "profitCurrency")]
    #[serde(default)]
    pub profit_currency: Option<String>,
}

impl std::fmt::Display for MultipleMarketDetailsResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use prettytable::format;
        use prettytable::{Cell, Row, Table};

        let mut table = Table::new();

        // Set table format
        table.set_format(*format::consts::FORMAT_BOX_CHARS);

        // Add header
        table.add_row(Row::new(vec![
            Cell::new("INSTRUMENT NAME"),
            Cell::new("EPIC"),
            Cell::new("BID"),
            Cell::new("OFFER"),
            Cell::new("MID"),
            Cell::new("SPREAD"),
            Cell::new("EXPIRY"),
            Cell::new("HIGH/LOW"),
        ]));

        // Sort by instrument name
        let mut sorted_details = self.market_details.clone();
        sorted_details.sort_by(|a, b| {
            a.instrument
                .name
                .to_lowercase()
                .cmp(&b.instrument.name.to_lowercase())
        });

        // Add rows
        for details in &sorted_details {
            let bid = details
                .snapshot
                .bid
                .map(|b| format!("{:.2}", b))
                .unwrap_or_else(|| "-".to_string());

            let offer = details
                .snapshot
                .offer
                .map(|o| format!("{:.2}", o))
                .unwrap_or_else(|| "-".to_string());

            let mid = match (details.snapshot.bid, details.snapshot.offer) {
                (Some(b), Some(o)) => format!("{:.2}", (b + o) / 2.0),
                _ => "-".to_string(),
            };

            let spread = match (details.snapshot.bid, details.snapshot.offer) {
                (Some(b), Some(o)) => format!("{:.2}", o - b),
                _ => "-".to_string(),
            };

            // Use expiry directly (shorter than last_dealing_date)
            let expiry = details
                .instrument
                .expiry_details
                .as_ref()
                .map(|ed| {
                    // Extract just the date part (YYYY-MM-DD)
                    ed.last_dealing_date
                        .split('T')
                        .next()
                        .unwrap_or(&ed.last_dealing_date)
                        .to_string()
                })
                .unwrap_or_else(|| {
                    details
                        .instrument
                        .expiry
                        .split('T')
                        .next()
                        .unwrap_or(&details.instrument.expiry)
                        .to_string()
                });

            let high_low = format!(
                "{}/{}",
                details
                    .snapshot
                    .high
                    .map(|h| format!("{:.2}", h))
                    .unwrap_or_else(|| "-".to_string()),
                details
                    .snapshot
                    .low
                    .map(|l| format!("{:.2}", l))
                    .unwrap_or_else(|| "-".to_string())
            );

            // Truncate long names to make room for EPIC
            let name = if details.instrument.name.len() > 30 {
                format!("{}...", &details.instrument.name[0..27])
            } else {
                details.instrument.name.clone()
            };

            // Don't truncate EPIC - show it complete
            let epic = details.instrument.epic.clone();

            table.add_row(Row::new(vec![
                Cell::new(&name),
                Cell::new(&epic),
                Cell::new(&bid),
                Cell::new(&offer),
                Cell::new(&mid),
                Cell::new(&spread),
                Cell::new(&expiry),
                Cell::new(&high_low),
            ]));
        }

        write!(f, "{}", table)
    }
}

impl std::fmt::Display for HistoricalPricesResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use prettytable::format;
        use prettytable::{Cell, Row, Table};

        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_BOX_CHARS);

        // Add header
        table.add_row(Row::new(vec![
            Cell::new("SNAPSHOT TIME"),
            Cell::new("OPEN BID"),
            Cell::new("OPEN ASK"),
            Cell::new("HIGH BID"),
            Cell::new("HIGH ASK"),
            Cell::new("LOW BID"),
            Cell::new("LOW ASK"),
            Cell::new("CLOSE BID"),
            Cell::new("CLOSE ASK"),
            Cell::new("VOLUME"),
        ]));

        // Add rows
        for price in &self.prices {
            let open_bid = price
                .open_price
                .bid
                .map(|v| format!("{:.4}", v))
                .unwrap_or_else(|| "-".to_string());

            let open_ask = price
                .open_price
                .ask
                .map(|v| format!("{:.4}", v))
                .unwrap_or_else(|| "-".to_string());

            let high_bid = price
                .high_price
                .bid
                .map(|v| format!("{:.4}", v))
                .unwrap_or_else(|| "-".to_string());

            let high_ask = price
                .high_price
                .ask
                .map(|v| format!("{:.4}", v))
                .unwrap_or_else(|| "-".to_string());

            let low_bid = price
                .low_price
                .bid
                .map(|v| format!("{:.4}", v))
                .unwrap_or_else(|| "-".to_string());

            let low_ask = price
                .low_price
                .ask
                .map(|v| format!("{:.4}", v))
                .unwrap_or_else(|| "-".to_string());

            let close_bid = price
                .close_price
                .bid
                .map(|v| format!("{:.4}", v))
                .unwrap_or_else(|| "-".to_string());

            let close_ask = price
                .close_price
                .ask
                .map(|v| format!("{:.4}", v))
                .unwrap_or_else(|| "-".to_string());

            let volume = price
                .last_traded_volume
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string());

            table.add_row(Row::new(vec![
                Cell::new(&price.snapshot_time),
                Cell::new(&open_bid),
                Cell::new(&open_ask),
                Cell::new(&high_bid),
                Cell::new(&high_ask),
                Cell::new(&low_bid),
                Cell::new(&low_ask),
                Cell::new(&close_bid),
                Cell::new(&close_ask),
                Cell::new(&volume),
            ]));
        }

        // Add summary footer
        writeln!(f, "{}", table)?;
        writeln!(f, "\nSummary:")?;
        writeln!(f, "  Total price points: {}", self.prices.len())?;
        writeln!(f, "  Instrument type: {:?}", self.instrument_type)?;

        if let Some(allowance) = &self.allowance {
            writeln!(
                f,
                "  Remaining allowance: {}",
                allowance.remaining_allowance
            )?;
            writeln!(f, "  Total allowance: {}", allowance.total_allowance)?;
        }

        Ok(())
    }
}

impl std::fmt::Display for MarketSearchResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use prettytable::format;
        use prettytable::{Cell, Row, Table};

        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_BOX_CHARS);

        // Add header
        table.add_row(Row::new(vec![
            Cell::new("INSTRUMENT NAME"),
            Cell::new("EPIC"),
            Cell::new("BID"),
            Cell::new("OFFER"),
            Cell::new("MID"),
            Cell::new("SPREAD"),
            Cell::new("EXPIRY"),
            Cell::new("TYPE"),
        ]));

        // Sort by instrument name
        let mut sorted_markets = self.markets.clone();
        sorted_markets.sort_by(|a, b| {
            a.instrument_name
                .to_lowercase()
                .cmp(&b.instrument_name.to_lowercase())
        });

        // Add rows
        for market in &sorted_markets {
            let bid = market
                .bid
                .map(|b| format!("{:.4}", b))
                .unwrap_or_else(|| "-".to_string());

            let offer = market
                .offer
                .map(|o| format!("{:.4}", o))
                .unwrap_or_else(|| "-".to_string());

            let mid = match (market.bid, market.offer) {
                (Some(b), Some(o)) => format!("{:.4}", (b + o) / 2.0),
                _ => "-".to_string(),
            };

            let spread = match (market.bid, market.offer) {
                (Some(b), Some(o)) => format!("{:.4}", o - b),
                _ => "-".to_string(),
            };

            // Truncate long names
            let name = if market.instrument_name.len() > 30 {
                format!("{}...", &market.instrument_name[0..27])
            } else {
                market.instrument_name.clone()
            };

            // Extract date from expiry
            let expiry = market
                .expiry
                .split('T')
                .next()
                .unwrap_or(&market.expiry)
                .to_string();

            let instrument_type = format!("{:?}", market.instrument_type);

            table.add_row(Row::new(vec![
                Cell::new(&name),
                Cell::new(&market.epic),
                Cell::new(&bid),
                Cell::new(&offer),
                Cell::new(&mid),
                Cell::new(&spread),
                Cell::new(&expiry),
                Cell::new(&instrument_type),
            ]));
        }

        writeln!(f, "{}", table)?;
        writeln!(f, "\nTotal markets found: {}", self.markets.len())?;

        Ok(())
    }
}

// ============================================================================
// WATCHLIST RESPONSES
// ============================================================================

/// Response containing all watchlists for the active account
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct WatchlistsResponse {
    /// List of watchlists
    pub watchlists: Vec<Watchlist>,
}

/// A watchlist containing instruments
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct Watchlist {
    /// Watchlist identifier
    pub id: String,
    /// Watchlist name
    pub name: String,
    /// Whether the watchlist can be edited
    pub editable: bool,
    /// Whether the watchlist can be deleted
    pub deleteable: bool,
    /// Whether this is a default system watchlist
    #[serde(rename = "defaultSystemWatchlist")]
    pub default_system_watchlist: bool,
}

/// Response when creating a new watchlist
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct CreateWatchlistResponse {
    /// The ID of the created watchlist
    #[serde(rename = "watchlistId")]
    pub watchlist_id: String,
    /// Status of the operation
    pub status: String,
}

/// Response containing markets in a watchlist
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct WatchlistMarketsResponse {
    /// List of markets in the watchlist
    pub markets: Vec<MarketData>,
}

/// Generic status response for operations
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct StatusResponse {
    /// Status of the operation (e.g., "SUCCESS")
    pub status: String,
}

// ============================================================================
// CLIENT SENTIMENT RESPONSES
// ============================================================================

/// Response containing client sentiment for multiple markets
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct ClientSentimentResponse {
    /// List of client sentiments
    #[serde(rename = "clientSentiments")]
    pub client_sentiments: Vec<MarketSentiment>,
}

/// Client sentiment data for a single market
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct MarketSentiment {
    /// Market identifier
    #[serde(rename = "marketId")]
    pub market_id: String,
    /// Percentage of clients with long positions
    #[serde(rename = "longPositionPercentage")]
    pub long_position_percentage: f64,
    /// Percentage of clients with short positions
    #[serde(rename = "shortPositionPercentage")]
    pub short_position_percentage: f64,
}

// ============================================================================
// INDICATIVE COSTS RESPONSES
// ============================================================================

/// Response containing indicative costs and charges
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct IndicativeCostsResponse {
    /// Reference for the indicative quote
    #[serde(rename = "indicativeQuoteReference")]
    pub indicative_quote_reference: String,
    /// Costs and charges breakdown
    #[serde(rename = "costsAndCharges")]
    pub costs_and_charges: CostsAndCharges,
}

/// Breakdown of costs and charges
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct CostsAndCharges {
    /// Total cost percentage
    #[serde(rename = "totalCostPercentage")]
    pub total_cost_percentage: Option<f64>,
    /// Total cost amount
    #[serde(rename = "totalCostAmount")]
    pub total_cost_amount: Option<f64>,
    /// Currency
    pub currency: Option<String>,
    /// One-off costs
    #[serde(rename = "oneOffCosts")]
    pub one_off_costs: Option<CostBreakdown>,
    /// Ongoing costs
    #[serde(rename = "ongoingCosts")]
    pub ongoing_costs: Option<CostBreakdown>,
    /// Transaction costs
    #[serde(rename = "transactionCosts")]
    pub transaction_costs: Option<CostBreakdown>,
    /// Incidental costs
    #[serde(rename = "incidentalCosts")]
    pub incidental_costs: Option<CostBreakdown>,
}

/// Breakdown of a specific cost category
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct CostBreakdown {
    /// Percentage value
    pub percentage: Option<f64>,
    /// Monetary amount
    pub amount: Option<f64>,
}

/// Response containing historical costs
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct CostsHistoryResponse {
    /// List of historical costs
    pub costs: Vec<HistoricalCost>,
}

/// Historical cost entry
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct HistoricalCost {
    /// Date of the cost
    pub date: String,
    /// Deal reference
    #[serde(rename = "dealReference")]
    pub deal_reference: Option<String>,
    /// Epic of the instrument
    pub epic: Option<String>,
    /// Total cost amount
    #[serde(rename = "totalCost")]
    pub total_cost: Option<f64>,
    /// Currency
    pub currency: Option<String>,
}

/// Response containing a durable medium document
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct DurableMediumResponse {
    /// The durable medium document content (typically HTML or PDF)
    pub document: String,
}

// ============================================================================
// ACCOUNT PREFERENCES RESPONSES
// ============================================================================

/// Response containing account preferences
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct AccountPreferencesResponse {
    /// Whether trailing stops are enabled
    #[serde(rename = "trailingStopsEnabled")]
    pub trailing_stops_enabled: bool,
}

// ============================================================================
// OPERATIONS/APPLICATION RESPONSES
// ============================================================================

/// Response containing application details (a single application)
#[derive(DebugPretty, Clone, Serialize, Deserialize, Default)]
pub struct ApplicationDetailsResponse {
    /// API key
    #[serde(rename = "apiKey")]
    pub api_key: String,
    /// Application name
    pub name: Option<String>,
    /// Application status
    pub status: String,
    /// Overall allowance for the account
    #[serde(rename = "allowanceAccountOverall")]
    pub allowance_account_overall: Option<i64>,
    /// Trading allowance for the account
    #[serde(rename = "allowanceAccountTrading")]
    pub allowance_account_trading: Option<i64>,
    /// Concurrent connections allowance
    #[serde(rename = "concurrentSubscriptionsLimit")]
    pub concurrent_subscriptions_limit: Option<i64>,
    /// Creation date
    #[serde(rename = "createdDate")]
    pub created_date: Option<String>,
}

// ============================================================================
// SINGLE POSITION RESPONSE
// ============================================================================

/// Response containing a single position
#[derive(DebugPretty, Clone, Serialize, Deserialize)]
pub struct SinglePositionResponse {
    /// Position details
    pub position: Position,
    /// Market data for the position
    pub market: MarketData,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::auth::{SessionResponse, V3Response};
    use crate::presentation::account::ActivityType;

    /// Round-trips a DTO through `serialize -> deserialize` and returns the
    /// re-parsed value. The DTOs under test do not all derive `PartialEq`, so
    /// callers assert the load-bearing fields on the result instead of comparing
    /// whole structs. This pins each custom `serialize_with` helper to its
    /// `deserialize_with` counterpart.
    fn roundtrip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let json = serde_json::to_string(value).expect("serialize failed");
        serde_json::from_str(&json).expect("re-deserialize failed")
    }

    #[test]
    fn test_accounts_response_deserialize_and_roundtrip() {
        let json = r#"{
            "accounts": [
                {
                    "accountId": "ABC12",
                    "accountName": "Demo CFD",
                    "accountType": "CFD",
                    "balance": {
                        "balance": 10000.0,
                        "deposit": 2000.0,
                        "profitLoss": 150.5,
                        "available": 8000.0
                    },
                    "currency": "EUR",
                    "status": "ENABLED",
                    "preferred": true
                }
            ]
        }"#;

        let resp: AccountsResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.accounts.len(), 1);
        let acc = &resp.accounts[0];
        assert_eq!(acc.account_id, "ABC12");
        assert_eq!(acc.account_type, "CFD");
        assert!((acc.balance.available - 8000.0).abs() < 1e-9);
        assert!(acc.preferred);

        let re = roundtrip(&resp);
        assert_eq!(re.accounts[0].account_id, "ABC12");
        assert_eq!(re.accounts[0].currency, "EUR");
    }

    #[test]
    fn test_positions_response_deserialize_and_roundtrip() {
        let json = r#"{
            "positions": [
                {
                    "position": {
                        "contractSize": 1.0,
                        "createdDate": "2025/07/01 10:00:00:000",
                        "createdDateUTC": "2025-07-01T08:00:00",
                        "dealId": "DIFAKE111",
                        "dealReference": "REFFAKE111",
                        "direction": "BUY",
                        "limitLevel": null,
                        "level": 100.5,
                        "size": 2.0,
                        "stopLevel": null,
                        "trailingStep": null,
                        "trailingStopDistance": null,
                        "currency": "GBP",
                        "controlledRisk": false,
                        "limitedRiskPremium": null
                    },
                    "market": {
                        "instrumentName": "FTSE 100",
                        "expiry": "-",
                        "epic": "IX.D.FTSE.DAILY.IP",
                        "instrumentType": "INDICES",
                        "lotSize": 1.0,
                        "high": 7600.0,
                        "low": 7500.0,
                        "percentageChange": 0.5,
                        "netChange": 30.0,
                        "bid": 7550.0,
                        "offer": 7551.0,
                        "updateTime": "10:00:00",
                        "updateTimeUTC": "08:00:00",
                        "delayTime": 0,
                        "streamingPricesAvailable": true,
                        "marketStatus": "TRADEABLE",
                        "scalingFactor": 1
                    },
                    "pnl": null
                }
            ]
        }"#;

        let resp: PositionsResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.positions.len(), 1);
        let pos = &resp.positions[0];
        assert_eq!(pos.position.deal_id, "DIFAKE111");
        assert_eq!(pos.position.direction, Direction::Buy);
        assert!((pos.position.size - 2.0).abs() < 1e-9);
        assert_eq!(pos.market.epic, "IX.D.FTSE.DAILY.IP");
        assert_eq!(pos.market.bid, Some(7550.0));
        assert!(pos.pnl.is_none());

        let re = roundtrip(&resp);
        assert_eq!(re.positions[0].position.deal_id, "DIFAKE111");
        assert_eq!(re.positions[0].market.epic, "IX.D.FTSE.DAILY.IP");
    }

    #[test]
    fn test_working_orders_response_deserialize_and_roundtrip() {
        // Inline, sanitized replacement for the former file-dependent test that
        // required an uncommitted `Data/working_orders.json`. Runs fully offline.
        let json = r#"{
            "workingOrders": [
                {
                    "workingOrderData": {
                        "dealId": "DIFAKEWO1",
                        "direction": "SELL",
                        "epic": "CS.D.EURUSD.MINI.IP",
                        "orderSize": 1.5,
                        "orderLevel": 1.2345,
                        "timeInForce": "GOOD_TILL_CANCELLED",
                        "goodTillDate": null,
                        "goodTillDateISO": null,
                        "createdDate": "2025/07/01 09:30:00:000",
                        "createdDateUTC": "2025-07-01T07:30:00",
                        "guaranteedStop": false,
                        "orderType": "LIMIT",
                        "stopDistance": null,
                        "limitDistance": null,
                        "currencyCode": "USD",
                        "dma": false,
                        "limitedRiskPremium": null,
                        "limitLevel": null,
                        "stopLevel": null,
                        "dealReference": "REFFAKEWO1"
                    },
                    "marketData": {
                        "instrumentName": "EUR/USD Mini",
                        "exchangeId": "FX",
                        "expiry": "-",
                        "marketStatus": "TRADEABLE",
                        "epic": "CS.D.EURUSD.MINI.IP",
                        "instrumentType": "CURRENCIES",
                        "lotSize": 1.0,
                        "high": 1.24,
                        "low": 1.23,
                        "percentageChange": 0.1,
                        "netChange": 0.001,
                        "bid": 1.2344,
                        "offer": 1.2346,
                        "updateTime": "09:30:00",
                        "updateTimeUTC": "07:30:00",
                        "delayTime": 0,
                        "streamingPricesAvailable": true,
                        "scalingFactor": 1
                    }
                }
            ]
        }"#;

        let resp: WorkingOrdersResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.working_orders.len(), 1);
        let order = &resp.working_orders[0];
        assert_eq!(order.working_order_data.epic, "CS.D.EURUSD.MINI.IP");
        assert_eq!(order.working_order_data.direction, Direction::Sell);
        assert!((order.working_order_data.order_size - 1.5).abs() < 1e-9);
        assert!((order.working_order_data.order_level - 1.2345).abs() < 1e-9);
        assert_eq!(
            order.market_data.instrument_type,
            InstrumentType::Currencies
        );

        let re = roundtrip(&resp);
        assert_eq!(re.working_orders[0].working_order_data.deal_id, "DIFAKEWO1");
        assert_eq!(re.working_orders[0].market_data.epic, "CS.D.EURUSD.MINI.IP");
    }

    #[test]
    fn test_account_activity_response_deserialize_and_roundtrip() {
        let json = r#"{
            "activities": [
                {
                    "date": "2025-07-01T09:00:00",
                    "dealId": "DIFAKEACT1",
                    "epic": "IX.D.FTSE.DAILY.IP",
                    "period": "DAY",
                    "dealReference": "REFFAKEACT1",
                    "type": "POSITION",
                    "status": "ACCEPTED",
                    "description": "Position opened",
                    "channel": "WEB",
                    "currency": "GBP",
                    "level": "7550.0"
                }
            ],
            "metadata": { "paging": { "size": 50, "next": null } }
        }"#;

        let resp: AccountActivityResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.activities.len(), 1);
        let act = &resp.activities[0];
        assert_eq!(act.deal_id.as_deref(), Some("DIFAKEACT1"));
        assert_eq!(act.activity_type, ActivityType::Position);
        assert_eq!(act.status, Some(Status::Accepted));
        assert!(resp.metadata.is_some());

        let re = roundtrip(&resp);
        assert_eq!(re.activities[0].activity_type, ActivityType::Position);
        assert_eq!(
            re.activities[0].deal_reference.as_deref(),
            Some("REFFAKEACT1")
        );
    }

    #[test]
    fn test_transaction_history_response_deserialize_and_roundtrip() {
        let json = r#"{
            "transactions": [
                {
                    "date": "01/07/25",
                    "dateUtc": "2025-07-01T08:00:00",
                    "openDateUtc": "2025-06-30T08:00:00",
                    "instrumentName": "FTSE 100",
                    "period": "DAY",
                    "profitAndLoss": "E150.50",
                    "transactionType": "DEAL",
                    "reference": "REFFAKETX1",
                    "openLevel": "7500.0",
                    "closeLevel": "7550.0",
                    "size": "2",
                    "currency": "GBP",
                    "cashTransaction": false
                }
            ],
            "metadata": {
                "pageData": { "pageNumber": 1, "pageSize": 20, "totalPages": 1 },
                "size": 1
            }
        }"#;

        let resp: TransactionHistoryResponse =
            serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.transactions.len(), 1);
        assert_eq!(resp.transactions[0].reference, "REFFAKETX1");
        assert_eq!(resp.transactions[0].profit_and_loss, "E150.50");
        assert_eq!(resp.metadata.size, 1);
        assert_eq!(resp.metadata.page_data.page_number, 1);

        let re = roundtrip(&resp);
        assert_eq!(re.transactions[0].reference, "REFFAKETX1");
        assert_eq!(re.metadata.page_data.total_pages, 1);
    }

    #[test]
    fn test_watchlists_response_deserialize_and_roundtrip() {
        let json = r#"{
            "watchlists": [
                {
                    "id": "WL1",
                    "name": "My Watchlist",
                    "editable": true,
                    "deleteable": true,
                    "defaultSystemWatchlist": false
                }
            ]
        }"#;

        let resp: WatchlistsResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.watchlists.len(), 1);
        let wl = &resp.watchlists[0];
        assert_eq!(wl.id, "WL1");
        assert_eq!(wl.name, "My Watchlist");
        assert!(wl.editable);
        assert!(!wl.default_system_watchlist);

        let re = roundtrip(&resp);
        assert_eq!(re.watchlists[0].id, "WL1");
        assert!(re.watchlists[0].deleteable);
    }

    #[test]
    fn test_client_sentiment_response_deserialize_and_roundtrip() {
        let json = r#"{
            "clientSentiments": [
                {
                    "marketId": "EURUSD",
                    "longPositionPercentage": 62.5,
                    "shortPositionPercentage": 37.5
                }
            ]
        }"#;

        let resp: ClientSentimentResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.client_sentiments.len(), 1);
        let s = &resp.client_sentiments[0];
        assert_eq!(s.market_id, "EURUSD");
        assert!((s.long_position_percentage - 62.5).abs() < 1e-9);
        assert!((s.short_position_percentage - 37.5).abs() < 1e-9);

        let re = roundtrip(&resp);
        assert_eq!(re.client_sentiments[0].market_id, "EURUSD");
    }

    #[test]
    fn test_indicative_costs_response_deserialize_and_roundtrip() {
        let json = r#"{
            "indicativeQuoteReference": "QREF-FAKE-1",
            "costsAndCharges": {
                "totalCostPercentage": 0.12,
                "totalCostAmount": 3.45,
                "currency": "GBP",
                "oneOffCosts": { "percentage": 0.05, "amount": 1.0 },
                "ongoingCosts": { "percentage": 0.02, "amount": 0.5 },
                "transactionCosts": { "percentage": 0.03, "amount": 1.2 },
                "incidentalCosts": { "percentage": 0.02, "amount": 0.75 }
            }
        }"#;

        let resp: IndicativeCostsResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.indicative_quote_reference, "QREF-FAKE-1");
        assert_eq!(resp.costs_and_charges.total_cost_amount, Some(3.45));
        assert_eq!(resp.costs_and_charges.currency.as_deref(), Some("GBP"));
        let one_off = resp
            .costs_and_charges
            .one_off_costs
            .as_ref()
            .expect("oneOffCosts present");
        assert_eq!(one_off.amount, Some(1.0));

        let re = roundtrip(&resp);
        assert_eq!(re.indicative_quote_reference, "QREF-FAKE-1");
        assert_eq!(re.costs_and_charges.total_cost_percentage, Some(0.12));
    }

    #[test]
    fn test_costs_and_charges_deserialize_and_roundtrip() {
        let json = r#"{
            "totalCostPercentage": 0.20,
            "totalCostAmount": 5.0,
            "currency": "USD",
            "transactionCosts": { "percentage": 0.10, "amount": 2.5 }
        }"#;

        let costs: CostsAndCharges = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(costs.total_cost_amount, Some(5.0));
        assert_eq!(costs.currency.as_deref(), Some("USD"));
        // Absent cost categories default to None (not `deny_unknown_fields`).
        assert!(costs.one_off_costs.is_none());
        assert!(costs.ongoing_costs.is_none());
        let txn = costs
            .transaction_costs
            .as_ref()
            .expect("transactionCosts present");
        assert_eq!(txn.percentage, Some(0.10));

        let re = roundtrip(&costs);
        assert_eq!(re.total_cost_amount, Some(5.0));
    }

    #[test]
    fn test_costs_history_response_deserialize_and_roundtrip() {
        let json = r#"{
            "costs": [
                {
                    "date": "2025-07-01",
                    "dealReference": "REFFAKEC1",
                    "epic": "IX.D.FTSE.DAILY.IP",
                    "totalCost": 5.5,
                    "currency": "GBP"
                }
            ]
        }"#;

        let resp: CostsHistoryResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.costs.len(), 1);
        let cost = &resp.costs[0];
        assert_eq!(cost.date, "2025-07-01");
        assert_eq!(cost.deal_reference.as_deref(), Some("REFFAKEC1"));
        assert_eq!(cost.total_cost, Some(5.5));

        let re = roundtrip(&resp);
        assert_eq!(re.costs[0].epic.as_deref(), Some("IX.D.FTSE.DAILY.IP"));
    }

    #[test]
    fn test_durable_medium_response_deserialize_and_roundtrip() {
        let json = r#"{ "document": "<html>Terms and Conditions</html>" }"#;

        let resp: DurableMediumResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.document, "<html>Terms and Conditions</html>");

        let re = roundtrip(&resp);
        assert_eq!(re.document, "<html>Terms and Conditions</html>");
    }

    #[test]
    fn test_account_preferences_response_deserialize_and_roundtrip() {
        let json = r#"{ "trailingStopsEnabled": true }"#;

        let resp: AccountPreferencesResponse =
            serde_json::from_str(json).expect("deserialize failed");
        assert!(resp.trailing_stops_enabled);

        let re = roundtrip(&resp);
        assert!(re.trailing_stops_enabled);
    }

    #[test]
    fn test_application_details_response_deserialize_and_roundtrip() {
        // `allowanceAccountOverall` is deliberately > i32::MAX (2_147_483_647)
        // to prove the field widening to i64 — an i32 field would overflow here.
        let json = r#"{
            "apiKey": "FAKE-API-KEY",
            "name": "My App",
            "status": "ENABLED",
            "allowanceAccountOverall": 3000000000,
            "allowanceAccountTrading": 1000,
            "concurrentSubscriptionsLimit": 40,
            "createdDate": "2025-01-01"
        }"#;

        let resp: ApplicationDetailsResponse =
            serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.api_key, "FAKE-API-KEY");
        assert_eq!(resp.name.as_deref(), Some("My App"));
        assert_eq!(resp.status, "ENABLED");
        assert!(resp.allowance_account_overall > Some(i64::from(i32::MAX)));
        assert_eq!(resp.allowance_account_overall, Some(3_000_000_000));
        assert_eq!(resp.concurrent_subscriptions_limit, Some(40));

        let re = roundtrip(&resp);
        assert_eq!(re.api_key, "FAKE-API-KEY");
        assert_eq!(re.allowance_account_overall, Some(3_000_000_000));
        assert_eq!(re.allowance_account_trading, Some(1000));
    }

    #[test]
    fn test_categories_response_deserialize_and_roundtrip() {
        let json = r#"{
            "categories": [
                { "code": "INDICES", "nonTradeable": false },
                { "code": "CRYPTOCURRENCY", "nonTradeable": true }
            ]
        }"#;

        let resp: CategoriesResponse = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.len(), 2);
        assert_eq!(resp.categories[0].code, "INDICES");
        assert!(!resp.categories[0].non_tradeable);
        assert!(resp.categories[1].non_tradeable);

        let re = roundtrip(&resp);
        assert_eq!(re.categories[1].code, "CRYPTOCURRENCY");
    }

    #[test]
    fn test_historical_prices_response_deserialize_and_roundtrip() {
        let json = r#"{
            "prices": [
                {
                    "snapshotTime": "2025:07:01-09:00:00",
                    "openPrice": { "bid": 1.2340, "ask": 1.2342, "lastTraded": null },
                    "highPrice": { "bid": 1.2350, "ask": 1.2352, "lastTraded": null },
                    "lowPrice": { "bid": 1.2330, "ask": 1.2332, "lastTraded": null },
                    "closePrice": { "bid": 1.2345, "ask": 1.2347, "lastTraded": null },
                    "lastTradedVolume": 1234
                }
            ],
            "instrumentType": "CURRENCIES",
            "allowance": {
                "remainingAllowance": 9950,
                "totalAllowance": 10000,
                "allowanceExpiry": 604800
            }
        }"#;

        let resp: HistoricalPricesResponse =
            serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.len(), 1);
        assert_eq!(resp.prices[0].snapshot_time, "2025:07:01-09:00:00");
        assert_eq!(resp.prices[0].close_price.bid, Some(1.2345));
        assert_eq!(resp.prices[0].last_traded_volume, Some(1234));
        assert_eq!(resp.instrument_type, InstrumentType::Currencies);
        let allowance = resp.allowance.as_ref().expect("allowance present");
        assert_eq!(allowance.remaining_allowance, 9950);
        assert_eq!(allowance.total_allowance, 10000);

        let re = roundtrip(&resp);
        assert_eq!(re.prices[0].open_price.ask, Some(1.2342));
        assert_eq!(
            re.allowance.as_ref().map(|a| a.allowance_expiry),
            Some(604800)
        );
    }

    #[test]
    fn test_v3_login_response_deserialize_and_roundtrip() {
        // Raw v3 (OAuth) login payload. `oauthToken.created_at` is derived at
        // parse time (serde `skip`), so it is intentionally absent from the wire
        // payload. No real tokens: obvious `FAKE-*` placeholders only.
        let json = r#"{
            "clientId": "FAKE-CLIENT-101",
            "accountId": "ABC12",
            "timezoneOffset": 1,
            "lightstreamerEndpoint": "https://demo-apd.marketdatasystems.com",
            "oauthToken": {
                "access_token": "FAKE-ACCESS-TOKEN",
                "refresh_token": "FAKE-REFRESH-TOKEN",
                "scope": "profile",
                "token_type": "Bearer",
                "expires_in": "60"
            }
        }"#;

        let resp: V3Response = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(resp.client_id, "FAKE-CLIENT-101");
        assert_eq!(resp.account_id, "ABC12");
        assert_eq!(resp.oauth_token.access_token, "FAKE-ACCESS-TOKEN");
        assert_eq!(resp.oauth_token.refresh_token, "FAKE-REFRESH-TOKEN");
        assert_eq!(resp.oauth_token.token_type, "Bearer");
        assert_eq!(resp.oauth_token.expires_in, "60");

        // The untagged `SessionResponse` must land on the V3 variant.
        let session_resp: SessionResponse =
            serde_json::from_str(json).expect("session deserialize failed");
        assert!(session_resp.is_v3());
        assert!(session_resp.get_session().is_oauth());

        let re = roundtrip(&resp);
        assert_eq!(re.oauth_token.access_token, "FAKE-ACCESS-TOKEN");
        assert_eq!(re.account_id, "ABC12");
    }
}
