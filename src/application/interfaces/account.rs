use crate::error::AppError;
use crate::model::responses::{
    AccountActivityResponse, AccountPreferencesResponse, AccountsResponse, PositionsResponse,
    TransactionHistoryResponse, WorkingOrdersResponse,
};
use async_trait::async_trait;

/// Interface for the account service
#[async_trait]
pub trait AccountService: Send + Sync {
    /// Gets information about all user accounts
    async fn get_accounts(&self) -> Result<AccountsResponse, AppError>;

    /// Gets open positions
    async fn get_positions(&self) -> Result<PositionsResponse, AppError>;

    /// Gets open positions base in filter
    async fn get_positions_w_filter(&self, filter: &str) -> Result<PositionsResponse, AppError>;

    /// Gets working orders
    async fn get_working_orders(&self) -> Result<WorkingOrdersResponse, AppError>;

    /// Gets account activity
    ///
    /// # Arguments
    /// * `session` - The current session
    /// * `from` - Start date in ISO format (e.g. "2023-01-01T00:00:00Z")
    /// * `to` - End date in ISO format (e.g. "2023-02-01T00:00:00Z")
    ///
    /// # Returns
    /// * Account activity for the specified period
    async fn get_activity(&self, from: &str, to: &str)
    -> Result<AccountActivityResponse, AppError>;

    /// Gets detailed account activity
    ///
    /// This method includes additional details for each activity item by using
    /// the detailed=true parameter in the API request.
    ///
    /// # Arguments
    /// * `session` - The current session
    /// * `from` - Start date in ISO format (e.g. "2023-01-01T00:00:00Z")
    /// * `to` - End date in ISO format (e.g. "2023-02-01T00:00:00Z")
    ///
    /// # Returns
    /// * Detailed account activity for the specified period
    async fn get_activity_with_details(
        &self,

        from: &str,
        to: &str,
    ) -> Result<AccountActivityResponse, AppError>;

    /// Gets transaction history for a given period, handling pagination automatically.
    async fn get_transactions(
        &self,
        from: &str,
        to: &str,
    ) -> Result<TransactionHistoryResponse, AppError>;

    /// Gets account preferences
    ///
    /// # Returns
    /// * `Ok(AccountPreferencesResponse)` - The account preferences
    /// * `Err(AppError)` - If the request fails
    async fn get_preferences(&self) -> Result<AccountPreferencesResponse, AppError>;

    /// Updates account preferences
    ///
    /// # Arguments
    /// * `trailing_stops_enabled` - Whether trailing stops should be enabled
    ///
    /// # Returns
    /// * `Ok(())` - If the update was successful
    /// * `Err(AppError)` - If the request fails
    async fn update_preferences(&self, trailing_stops_enabled: bool) -> Result<(), AppError>;

    /// Gets account activity for a specified period
    ///
    /// # Arguments
    /// * `period` - Period in milliseconds (e.g., 600000 for 10 minutes)
    ///
    /// # Returns
    /// * `Ok(AccountActivityResponse)` - Account activity for the period
    /// * `Err(AppError)` - If the request fails
    async fn get_activity_by_period(
        &self,
        period_ms: u64,
    ) -> Result<AccountActivityResponse, AppError>;
}
