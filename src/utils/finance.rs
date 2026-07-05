// src/utils/finance.rs
//
// Financial calculation utilities for the IG client

use crate::presentation::account::Position;

/// Calculate the Profit and Loss (P&L) for a position based on current market prices
///
/// Thin wrapper over [`Position::pnl_checked`], the single source of truth for
/// position P&L, kept for backwards compatibility.
///
/// # Arguments
///
/// * `position` - The position to calculate P&L for
///
/// # Returns
///
/// * `Option<f64>` - The calculated P&L if market prices are available, None otherwise
///
#[must_use]
pub fn calculate_pnl(position: &Position) -> Option<f64> {
    position.pnl_checked()
}

/// Calculate the percentage return for a position
///
/// # Arguments
///
/// * `position` - The position to calculate percentage return for
///
/// # Returns
///
/// * `Option<f64>` - The calculated percentage return if market prices are available, None otherwise
#[must_use]
pub fn calculate_percentage_return(position: &Position) -> Option<f64> {
    let pnl = calculate_pnl(position)?;
    let initial_value = position.position.level * position.position.size;

    // Avoid division by zero
    if initial_value == 0.0 {
        return None;
    }

    Some((pnl / initial_value) * 100.0)
}
