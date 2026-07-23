/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 8/3/26
******************************************************************************/

//! Indicative costs and charges service interface for IG Markets API
//!
//! This module defines the interface for retrieving indicative costs and charges
//! information for opening, closing, and editing positions. This is required
//! for regulatory compliance in certain jurisdictions.

use crate::error::AppError;
use crate::model::requests::{CloseCostsRequest, EditCostsRequest, OpenCostsRequest};
use crate::model::responses::{
    CostsHistoryResponse, DurableMediumResponse, IndicativeCostsResponse,
};
use async_trait::async_trait;

/// Service for retrieving indicative costs and charges from the IG Markets API
///
/// This service provides regulatory cost information for trading activities,
/// including costs for opening, closing, and editing positions.
#[async_trait]
pub trait CostsService: Send + Sync {
    /// Returns indicative costs and charges for opening a position
    ///
    /// # Arguments
    /// * `request` - The request containing position details
    ///
    /// # Returns
    /// * `Ok(IndicativeCostsResponse)` - The indicative costs and charges
    /// * `Err(AppError)` - If the request fails
    async fn get_indicative_costs_open(
        &self,
        request: &OpenCostsRequest,
    ) -> Result<IndicativeCostsResponse, AppError>;

    /// Returns indicative costs and charges for closing a position
    ///
    /// # Arguments
    /// * `request` - The request containing position details
    ///
    /// # Returns
    /// * `Ok(IndicativeCostsResponse)` - The indicative costs and charges
    /// * `Err(AppError)` - If the request fails
    async fn get_indicative_costs_close(
        &self,
        request: &CloseCostsRequest,
    ) -> Result<IndicativeCostsResponse, AppError>;

    /// Returns indicative costs and charges for editing a position
    ///
    /// # Arguments
    /// * `request` - The request containing position edit details
    ///
    /// # Returns
    /// * `Ok(IndicativeCostsResponse)` - The indicative costs and charges
    /// * `Err(AppError)` - If the request fails
    async fn get_indicative_costs_edit(
        &self,
        request: &EditCostsRequest,
    ) -> Result<IndicativeCostsResponse, AppError>;

    /// Returns historical costs and charges for a date range, handling
    /// pagination automatically (the full window is returned).
    ///
    /// IG parses the bounds as ISO-8601 instants; a zone-less timestamp
    /// (e.g. `"2023-01-01T00:00:00"`) is accepted and treated as UTC — the
    /// implementation appends the required `Z` designator when missing —
    /// and a date-only bound (e.g. `"2023-01-01"`) expands to midnight UTC.
    ///
    /// Entries carry no cost amounts inline: each references its disclosure
    /// document via `indicative_quote_reference`, retrievable with
    /// [`CostsService::get_durable_medium`].
    ///
    /// # Arguments
    /// * `from` - Window start, ISO-8601 (e.g., "2023-01-01T00:00:00" or "2023-01-01T00:00:00Z")
    /// * `to` - Window end, same format
    ///
    /// # Returns
    /// * `Ok(CostsHistoryResponse)` - Historical costs entries for the whole window
    /// * `Err(AppError)` - If the request fails
    async fn get_costs_history(
        &self,
        from: &str,
        to: &str,
    ) -> Result<CostsHistoryResponse, AppError>;

    /// Returns a durable medium document for a quote reference
    ///
    /// # Arguments
    /// * `quote_reference` - The indicative quote reference
    ///
    /// # Returns
    /// * `Ok(DurableMediumResponse)` - The durable medium document
    /// * `Err(AppError)` - If the request fails
    async fn get_durable_medium(
        &self,
        quote_reference: &str,
    ) -> Result<DurableMediumResponse, AppError>;
}
