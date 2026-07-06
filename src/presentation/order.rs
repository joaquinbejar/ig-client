/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 13/5/25
******************************************************************************/
use pretty_simple_display::DisplaySimple;
use serde::{Deserialize, Serialize};

/// Order direction (buy or sell)
#[repr(u8)]
#[derive(
    Debug, Clone, Copy, DisplaySimple, Serialize, Deserialize, PartialEq, Eq, Hash, Default,
)]
#[serde(rename_all = "UPPERCASE")]
pub enum Direction {
    /// Buy direction (long position)
    #[default]
    Buy,
    /// Sell direction (short position)
    Sell,
}

/// Order type
#[repr(u8)]
#[derive(
    Debug, Clone, Copy, DisplaySimple, Serialize, Deserialize, PartialEq, Eq, Hash, Default,
)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderType {
    /// Limit order - executed when price reaches specified level
    #[default]
    Limit,
    /// Market order - executed immediately at current market price
    Market,
    /// Quote order - executed at quoted price
    Quote,
    /// Stop order - becomes market order when price reaches specified level
    Stop,
    /// Stop limit order - becomes limit order when price reaches specified level
    StopLimit,
}

/// Represents the status of an order or transaction in the system.
///
/// This enum covers various states an order can be in throughout its lifecycle,
/// from creation to completion or cancellation.
#[repr(u8)]
#[derive(
    Debug, Clone, Copy, DisplaySimple, Serialize, Deserialize, PartialEq, Eq, Hash, Default,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    /// Order has been amended or modified after initial creation
    Amended,
    /// Order has been deleted from the system
    Deleted,
    /// Order has been completely closed with all positions resolved
    FullyClosed,
    /// Order has been opened and is active in the market
    Opened,
    /// Order has been partially closed with some positions still open
    PartiallyClosed,
    /// Order has been closed but may differ from FullyClosed in context
    Closed,
    /// Default state - order is open and active in the market
    #[default]
    Open,
    /// Order has been updated with new parameters
    Updated,
    /// Order has been accepted by the system or exchange
    Accepted,
    /// Order has been rejected by the system or exchange
    Rejected,
    /// Order is currently working (waiting to be filled)
    Working,
    /// Order has been filled (executed)
    Filled,
    /// Order has been cancelled
    Cancelled,
    /// Order has expired (time in force elapsed)
    Expired,
}

/// Order duration (time in force)
#[repr(u8)]
#[derive(
    Debug, Clone, Copy, DisplaySimple, Serialize, Deserialize, PartialEq, Eq, Hash, Default,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    /// Order remains valid until cancelled by the client
    #[default]
    GoodTillCancelled,
    /// Order remains valid until a specified date
    GoodTillDate,
    /// Order is executed immediately (partially or completely) or cancelled.
    ///
    /// Note: `IMMEDIATE_OR_CANCEL` is not a documented value for IG's OTC
    /// `timeInForce`; it is retained for backward compatibility. Prefer
    /// [`TimeInForce::ExecuteAndEliminate`] for partial-fill-then-cancel
    /// semantics on OTC create/close position endpoints.
    ImmediateOrCancel,
    /// Order must be filled completely immediately or cancelled.
    ///
    /// Serializes to `FILL_OR_KILL`.
    FillOrKill,
    /// Order fills as much as possible immediately, then cancels the remainder.
    ///
    /// Serializes to `EXECUTE_AND_ELIMINATE`. Accepted alongside
    /// [`TimeInForce::FillOrKill`] by IG's OTC create/close position endpoints.
    ExecuteAndEliminate,
}
