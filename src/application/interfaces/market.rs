use crate::error::AppError;
use crate::model::requests::RecentPricesRequest;
use crate::model::responses::{
    CategoriesResponse, CategoryInstrumentsResponse, DBEntryResponse, HistoricalPricesResponse,
    MarketNavigationResponse, MarketSearchResponse, MultipleMarketDetailsResponse,
};
use crate::presentation::market::{MarketData, MarketDetails};
use async_trait::async_trait;

/// Interface for the market service
#[async_trait]
pub trait MarketService: Send + Sync {
    /// Searches markets by search term
    async fn search_markets(&self, search_term: &str) -> Result<MarketSearchResponse, AppError>;

    /// Gets details of a specific market by its EPIC
    async fn get_market_details(&self, epic: &str) -> Result<MarketDetails, AppError>;

    /// Gets details of multiple markets by their EPICs in a single request
    ///
    /// This method accepts a vector of EPICs and returns a vector of market details.
    /// The EPICs are sent as a comma-separated list in a single API request.
    ///
    /// # Arguments
    /// * `session` - The active IG session
    /// * `epics` - A slice of EPICs to get details for
    ///
    /// # Returns
    /// A vector of market details in the same order as the input EPICs
    async fn get_multiple_market_details(
        &self,
        epics: &[String],
    ) -> Result<MultipleMarketDetailsResponse, AppError>;

    /// Gets historical prices for a market
    async fn get_historical_prices(
        &self,
        epic: &str,
        resolution: &str,
        from: &str,
        to: &str,
    ) -> Result<HistoricalPricesResponse, AppError>;

    /// Gets historical prices for a market using path parameters (API v2)
    ///
    /// # Arguments
    /// * `epic` - Instrument epic
    /// * `resolution` - Price resolution (SECOND, MINUTE, MINUTE_2, MINUTE_3, MINUTE_5, MINUTE_10, MINUTE_15, MINUTE_30, HOUR, HOUR_2, HOUR_3, HOUR_4, DAY, WEEK, MONTH)
    /// * `start_date` - Start date (yyyy-MM-dd HH:mm:ss)
    /// * `end_date` - End date (yyyy-MM-dd HH:mm:ss). Must be later than the start date
    async fn get_historical_prices_by_date_range(
        &self,
        epic: &str,
        resolution: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<HistoricalPricesResponse, AppError>;

    /// Gets recent historical prices with custom parameters
    ///
    /// # Arguments
    /// * `params` - Request parameters including epic, resolution, and date range
    ///
    /// # Returns
    /// Historical price data for the specified parameters
    async fn get_recent_prices(
        &self,
        params: &RecentPricesRequest<'_>,
    ) -> Result<HistoricalPricesResponse, AppError>;

    /// Gets historical prices by number of data points (API v1)
    ///
    /// # Arguments
    /// * `epic` - Instrument epic
    /// * `resolution` - Price resolution
    /// * `num_points` - Number of data points required
    async fn get_historical_prices_by_count_v1(
        &self,

        epic: &str,
        resolution: &str,
        num_points: u32,
    ) -> Result<HistoricalPricesResponse, AppError>;

    /// Gets historical prices by number of data points (API v2)
    ///
    /// # Arguments
    /// * `epic` - Instrument epic
    /// * `resolution` - Price resolution
    /// * `num_points` - Number of data points required
    async fn get_historical_prices_by_count_v2(
        &self,
        epic: &str,
        resolution: &str,
        num_points: u32,
    ) -> Result<HistoricalPricesResponse, AppError>;

    /// Gets the top-level market navigation nodes
    ///
    /// This method returns the root nodes of the market hierarchy, which can be used
    /// to navigate through the available markets.
    async fn get_market_navigation(&self) -> Result<MarketNavigationResponse, AppError>;

    /// Gets the market navigation node with the specified ID
    ///
    /// This method returns the child nodes and markets under the specified node ID.
    ///
    /// # Arguments
    /// * `node_id` - The ID of the navigation node to retrieve
    async fn get_market_navigation_node(
        &self,
        node_id: &str,
    ) -> Result<MarketNavigationResponse, AppError>;

    /// Navigates through all levels of the market hierarchy and collects all MarketData
    ///
    /// This method performs a comprehensive traversal of the IG Markets hierarchy,
    /// starting from the root navigation and going through multiple levels to collect
    /// all available market instruments.
    ///
    /// # Arguments
    /// * `session` - The authenticated IG session
    /// * `max_levels` - Maximum depth to traverse (default: 5 levels)
    ///
    /// # Returns
    /// * `Result<Vec<MarketData>, AppError>` - Vector containing all found market instruments
    async fn get_all_markets(&self) -> Result<Vec<MarketData>, AppError>;

    /// Gets all markets converted to database entries format
    ///
    /// This method retrieves all available markets and converts them to a standardized
    /// database entry format for storage or further processing.
    ///
    /// # Arguments
    /// * `session` - The authenticated IG session
    ///
    /// # Returns
    /// * `Result<Vec<DBEntry>, AppError>` - Vector of database entries representing all markets
    async fn get_vec_db_entries(&self) -> Result<Vec<DBEntryResponse>, AppError>;

    /// Gets all categories of instruments enabled for the IG account
    ///
    /// This method returns a list of all categories of instruments that are
    /// available for trading on the account.
    ///
    /// # Returns
    /// * `Result<CategoriesResponse, AppError>` - List of available categories
    async fn get_categories(&self) -> Result<CategoriesResponse, AppError>;

    /// Gets all instruments for a specific category
    ///
    /// This method returns all instruments belonging to the specified category,
    /// with optional pagination support.
    ///
    /// # Arguments
    /// * `category_id` - The identifier of the category
    /// * `page_number` - Optional page number (default: 0)
    /// * `page_size` - Optional page size (default: 150, max: 1000)
    ///
    /// # Returns
    /// * `Result<CategoryInstrumentsResponse, AppError>` - List of instruments in the category
    async fn get_category_instruments(
        &self,
        category_id: &str,
        page_number: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<CategoryInstrumentsResponse, AppError>;
}
