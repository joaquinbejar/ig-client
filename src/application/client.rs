/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 19/10/25
******************************************************************************/
use crate::application::auth::WebsocketInfo;
use crate::application::config::Config;
use crate::application::interfaces::account::AccountService;
use crate::application::interfaces::costs::CostsService;
use crate::application::interfaces::market::MarketService;
use crate::application::interfaces::operations::OperationsService;
use crate::application::interfaces::order::OrderService;
use crate::application::interfaces::sentiment::SentimentService;
use crate::application::interfaces::watchlist::WatchlistService;
use crate::error::AppError;
use crate::model::http::HttpClient;
use crate::model::requests::RecentPricesRequest;
use crate::model::requests::{
    AddToWatchlistRequest, CloseCostsRequest, CreateWatchlistRequest, EditCostsRequest,
    OpenCostsRequest, UpdateWorkingOrderRequest,
};
use crate::model::requests::{
    ClosePositionRequest, CreateOrderRequest, CreateWorkingOrderRequest, UpdatePositionRequest,
};
use crate::model::responses::{
    AccountPreferencesResponse, ApplicationDetailsResponse, CategoriesResponse,
    CategoryInstrumentsResponse, ClientSentimentResponse, CostsHistoryResponse,
    CreateWatchlistResponse, DBEntryResponse, DurableMediumResponse, HistoricalPricesResponse,
    IndicativeCostsResponse, MarketNavigationResponse, MarketSearchResponse, MarketSentiment,
    MultipleMarketDetailsResponse, SinglePositionResponse, StatusResponse,
    WatchlistMarketsResponse, WatchlistsResponse,
};
use crate::model::responses::{
    ClosePositionResponse, CreateOrderResponse, CreateWorkingOrderResponse, UpdatePositionResponse,
};
use crate::model::retry::backoff_delay;
use crate::model::streaming::{
    StreamingAccountDataField, StreamingChartField, StreamingMarketField, StreamingPriceField,
    get_streaming_account_data_fields, get_streaming_chart_fields, get_streaming_market_fields,
    get_streaming_price_fields,
};
use crate::prelude::{
    AccountActivityResponse, AccountFields, AccountsResponse, ChartData, ChartScale,
    OrderConfirmationResponse, PositionsResponse, TradeFields, TransactionHistoryResponse,
    WorkingOrdersResponse,
};
use crate::presentation::market::{MarketData, MarketDetails};
use crate::presentation::price::PriceData;
use async_trait::async_trait;
use lightstreamer_rs::client::{LightstreamerClient, LogType, Transport};
use lightstreamer_rs::subscription::{
    ChannelSubscriptionListener, Snapshot, Subscription, SubscriptionMode,
};
use lightstreamer_rs::utils::setup_signal_hook;
use reqwest::StatusCode;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify, RwLock, mpsc};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

const MAX_CONNECTION_ATTEMPTS: u64 = 3;

/// Returns `true` if an error from the order-confirmation endpoint is transient
/// and worth polling again.
///
/// The IG `GET /confirms/{dealReference}` endpoint returns `404 Not Found` until
/// the deal has been processed, so a not-found result means "not yet available"
/// rather than a permanent failure. Transient cases retried by
/// [`Client::get_order_confirmation_w_retry`]:
///
/// - [`AppError::RateLimitExceeded`] — rate limited.
/// - [`AppError::Network`] — connection / transport error.
/// - [`AppError::NotFound`] / [`AppError::Unexpected`] with a `404` or `5xx`
///   status — confirmation not yet available or a server error.
///
/// Everything else (auth failures, invalid input, deserialization, 4xx client
/// errors) is permanent and returned to the caller immediately.
#[must_use]
fn is_transient_confirmation_error(err: &AppError) -> bool {
    match err {
        AppError::RateLimitExceeded | AppError::Network(_) | AppError::NotFound => true,
        AppError::Unexpected(status) => {
            *status == StatusCode::NOT_FOUND || status.is_server_error()
        }
        _ => false,
    }
}

/// Main client for interacting with IG Markets API
///
/// This client provides a unified interface for all IG Markets API operations,
/// including market data, account management, and order execution.
pub struct Client {
    http_client: Arc<HttpClient>,
}

impl Client {
    /// Creates a new client instance
    ///
    /// # Returns
    /// A new Client with default configuration
    ///
    /// # Panics
    /// Panics if the underlying [`HttpClient`] cannot be constructed at
    /// startup — typically a missing TLS backend, but also invalid proxy or
    /// certificate configuration. This is an unrecoverable environment
    /// invariant at construction time. For graceful handling use
    /// [`Client::try_new`], which returns a typed [`AppError`] instead of
    /// panicking.
    pub fn new() -> Self {
        Self::try_new().expect("failed to create HTTP client")
    }

    /// Creates a new client instance without performing initial authentication,
    /// returning an error if the underlying HTTP client cannot be constructed.
    ///
    /// This is the fallible counterpart to [`Client::new`]: it builds the
    /// underlying [`HttpClient`] via [`HttpClient::new_lazy`] and surfaces a
    /// client-construction failure as a typed [`AppError`] instead of panicking.
    ///
    /// # Returns
    /// * `Ok(Client)` - A client ready to use with the default configuration.
    /// * `Err(AppError)` - If the underlying HTTP client cannot be constructed.
    ///
    /// # Errors
    /// Returns [`AppError::Network`] if the underlying `reqwest` client cannot
    /// be built (e.g. the system TLS backend fails to initialize).
    pub fn try_new() -> Result<Self, AppError> {
        let http_client = Arc::new(HttpClient::new_lazy(Config::default())?);
        Ok(Self { http_client })
    }

    /// Gets WebSocket connection information for Lightstreamer, reusing the
    /// cached session.
    ///
    /// Delegates to [`HttpClient::ws_info`], which returns the cached session
    /// when it is valid and only logs in when needed.
    ///
    /// # Returns
    /// * `Ok(WebsocketInfo)` - Server endpoint, authentication tokens, and
    ///   account ID for the current session.
    /// * `Err(AppError)` - If session retrieval (login / refresh) fails.
    ///
    /// # Errors
    /// Returns [`AppError`] when the session cannot be retrieved.
    pub async fn ws_info(&self) -> Result<WebsocketInfo, AppError> {
        self.http_client.ws_info().await
    }

    /// Gets WebSocket connection information for Lightstreamer
    ///
    /// # Returns
    /// * `WebsocketInfo` containing server endpoint, authentication tokens, and account ID
    #[deprecated(
        note = "use ws_info() which reuses the cached session and returns a typed error instead of a default-on-error WebsocketInfo"
    )]
    pub async fn get_ws_info(&self) -> WebsocketInfo {
        self.ws_info().await.unwrap_or_default()
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MarketService for Client {
    async fn search_markets(&self, search_term: &str) -> Result<MarketSearchResponse, AppError> {
        let path = format!("markets?searchTerm={}", search_term);
        info!("Searching markets with term: {}", search_term);
        let result: MarketSearchResponse = self.http_client.get(&path, Some(1)).await?;
        debug!("{} markets found", result.markets.len());
        Ok(result)
    }

    async fn get_market_details(&self, epic: &str) -> Result<MarketDetails, AppError> {
        let path = format!("markets/{epic}");
        info!("Getting market details: {}", epic);
        let market_value: Value = self.http_client.get(&path, Some(3)).await?;
        let market_details: MarketDetails = serde_json::from_value(market_value)?;
        debug!("Market details obtained for: {}", epic);
        Ok(market_details)
    }

    async fn get_multiple_market_details(
        &self,
        epics: &[String],
    ) -> Result<MultipleMarketDetailsResponse, AppError> {
        if epics.is_empty() {
            return Ok(MultipleMarketDetailsResponse::default());
        } else if epics.len() > 50 {
            return Err(AppError::InvalidInput(
                "The maximum number of EPICs is 50".to_string(),
            ));
        }

        let epics_str = epics.join(",");
        let path = format!("markets?epics={}", epics_str);
        debug!(
            "Getting market details for {} EPICs in a batch",
            epics.len()
        );

        let response: MultipleMarketDetailsResponse = self.http_client.get(&path, Some(2)).await?;

        Ok(response)
    }

    async fn get_historical_prices(
        &self,
        epic: &str,
        resolution: &str,
        from: &str,
        to: &str,
    ) -> Result<HistoricalPricesResponse, AppError> {
        let path = format!(
            "prices/{}?resolution={}&from={}&to={}",
            epic, resolution, from, to
        );
        info!("Getting historical prices for: {}", epic);
        let result: HistoricalPricesResponse = self.http_client.get(&path, Some(3)).await?;
        debug!("Historical prices obtained for: {}", epic);
        Ok(result)
    }

    async fn get_historical_prices_by_date_range(
        &self,
        epic: &str,
        resolution: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<HistoricalPricesResponse, AppError> {
        let path = format!("prices/{}/{}/{}/{}", epic, resolution, start_date, end_date);
        info!(
            "Getting historical prices for epic: {}, resolution: {}, from: {} to: {}",
            epic, resolution, start_date, end_date
        );
        let result: HistoricalPricesResponse = self.http_client.get(&path, Some(2)).await?;
        debug!(
            "Historical prices obtained for epic: {}, {} data points",
            epic,
            result.prices.len()
        );
        Ok(result)
    }

    async fn get_recent_prices(
        &self,
        params: &RecentPricesRequest<'_>,
    ) -> Result<HistoricalPricesResponse, AppError> {
        let mut query_params = Vec::new();

        if let Some(res) = params.resolution {
            query_params.push(format!("resolution={}", res));
        }
        if let Some(f) = params.from {
            query_params.push(format!("from={}", f));
        }
        if let Some(t) = params.to {
            query_params.push(format!("to={}", t));
        }
        if let Some(max) = params.max_points {
            query_params.push(format!("max={}", max));
        }
        if let Some(size) = params.page_size {
            query_params.push(format!("pageSize={}", size));
        }
        if let Some(num) = params.page_number {
            query_params.push(format!("pageNumber={}", num));
        }

        let query_string = if query_params.is_empty() {
            String::new()
        } else {
            format!("?{}", query_params.join("&"))
        };

        let path = format!("prices/{}{}", params.epic, query_string);
        info!("Getting recent prices for epic: {}", params.epic);
        let result: HistoricalPricesResponse = self.http_client.get(&path, Some(3)).await?;
        debug!(
            "Recent prices obtained for epic: {}, {} data points",
            params.epic,
            result.prices.len()
        );
        Ok(result)
    }

    async fn get_historical_prices_by_count_v1(
        &self,
        epic: &str,
        resolution: &str,
        num_points: i32,
    ) -> Result<HistoricalPricesResponse, AppError> {
        let path = format!("prices/{}/{}/{}", epic, resolution, num_points);
        info!(
            "Getting historical prices (v1) for epic: {}, resolution: {}, points: {}",
            epic, resolution, num_points
        );
        let result: HistoricalPricesResponse = self.http_client.get(&path, Some(1)).await?;
        debug!(
            "Historical prices (v1) obtained for epic: {}, {} data points",
            epic,
            result.prices.len()
        );
        Ok(result)
    }

    async fn get_historical_prices_by_count_v2(
        &self,
        epic: &str,
        resolution: &str,
        num_points: i32,
    ) -> Result<HistoricalPricesResponse, AppError> {
        let path = format!("prices/{}/{}/{}", epic, resolution, num_points);
        info!(
            "Getting historical prices (v2) for epic: {}, resolution: {}, points: {}",
            epic, resolution, num_points
        );
        let result: HistoricalPricesResponse = self.http_client.get(&path, Some(2)).await?;
        debug!(
            "Historical prices (v2) obtained for epic: {}, {} data points",
            epic,
            result.prices.len()
        );
        Ok(result)
    }

    async fn get_market_navigation(&self) -> Result<MarketNavigationResponse, AppError> {
        let path = "marketnavigation";
        info!("Getting top-level market navigation nodes");
        let result: MarketNavigationResponse = self.http_client.get(path, Some(1)).await?;
        debug!("{} navigation nodes found", result.nodes.len());
        debug!("{} markets found at root level", result.markets.len());
        Ok(result)
    }

    async fn get_market_navigation_node(
        &self,
        node_id: &str,
    ) -> Result<MarketNavigationResponse, AppError> {
        let path = format!("marketnavigation/{}", node_id);
        info!("Getting market navigation node: {}", node_id);
        let result: MarketNavigationResponse = self.http_client.get(&path, Some(1)).await?;
        debug!("{} child nodes found", result.nodes.len());
        debug!("{} markets found in node {}", result.markets.len(), node_id);
        Ok(result)
    }

    async fn get_all_markets(&self) -> Result<Vec<MarketData>, AppError> {
        let max_depth = 6;
        info!(
            "Starting comprehensive market hierarchy traversal (max {} levels)",
            max_depth
        );

        let root_response = self.get_market_navigation().await?;
        info!(
            "Root navigation: {} nodes, {} markets at top level",
            root_response.nodes.len(),
            root_response.markets.len()
        );

        let mut all_markets = root_response.markets.clone();
        let mut nodes_to_process = root_response.nodes.clone();
        let mut processed_levels = 0;

        while !nodes_to_process.is_empty() && processed_levels < max_depth {
            let mut next_level_nodes = Vec::new();
            let mut level_market_count = 0;

            info!(
                "Processing level {} with {} nodes",
                processed_levels,
                nodes_to_process.len()
            );

            for node in &nodes_to_process {
                match self.get_market_navigation_node(&node.id).await {
                    Ok(node_response) => {
                        let node_markets = node_response.markets.len();
                        let node_children = node_response.nodes.len();

                        if node_markets > 0 || node_children > 0 {
                            debug!(
                                "Node '{}' (level {}): {} markets, {} child nodes",
                                node.name, processed_levels, node_markets, node_children
                            );
                        }

                        all_markets.extend(node_response.markets);
                        level_market_count += node_markets;
                        next_level_nodes.extend(node_response.nodes);
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to get markets for node '{}' at level {}: {:?}",
                            node.name,
                            processed_levels,
                            e
                        );
                    }
                }
            }

            info!(
                "Level {} completed: {} markets found, {} nodes for next level",
                processed_levels,
                level_market_count,
                next_level_nodes.len()
            );

            nodes_to_process = next_level_nodes;
            processed_levels += 1;
        }

        info!(
            "Market hierarchy traversal completed: {} total markets found across {} levels",
            all_markets.len(),
            processed_levels
        );

        Ok(all_markets)
    }

    async fn get_vec_db_entries(&self) -> Result<Vec<DBEntryResponse>, AppError> {
        info!("Getting all markets from hierarchy for DB entries");

        let all_markets = self.get_all_markets().await?;
        info!("Collected {} markets from hierarchy", all_markets.len());

        let mut vec_db_entries: Vec<DBEntryResponse> = all_markets
            .iter()
            .map(DBEntryResponse::from)
            .filter(|entry| !entry.epic.is_empty())
            .collect();

        info!("Created {} DB entries from markets", vec_db_entries.len());

        // Collect unique symbols
        let unique_symbols: std::collections::HashSet<String> = vec_db_entries
            .iter()
            .map(|entry| entry.symbol.clone())
            .filter(|symbol| !symbol.is_empty())
            .collect();

        info!(
            "Found {} unique symbols to fetch expiry dates for",
            unique_symbols.len()
        );

        let mut symbol_expiry_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for symbol in unique_symbols {
            if let Some(entry) = vec_db_entries
                .iter()
                .find(|e| e.symbol == symbol && !e.epic.is_empty())
            {
                match self.get_market_details(&entry.epic).await {
                    Ok(market_details) => {
                        let expiry_date = market_details
                            .instrument
                            .expiry_details
                            .as_ref()
                            .map(|details| details.last_dealing_date.clone())
                            .unwrap_or_else(|| market_details.instrument.expiry.clone());

                        symbol_expiry_map.insert(symbol.clone(), expiry_date);
                        if let Some(expiry) = symbol_expiry_map.get(&symbol) {
                            info!("Fetched expiry date for symbol {}: {}", symbol, expiry);
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to get market details for epic {} (symbol {}): {:?}",
                            entry.epic,
                            symbol,
                            e
                        );
                        symbol_expiry_map.insert(symbol.clone(), entry.expiry.clone());
                    }
                }
            }
        }

        for entry in &mut vec_db_entries {
            if let Some(expiry_date) = symbol_expiry_map.get(&entry.symbol) {
                entry.expiry = expiry_date.clone();
            }
        }

        info!("Updated expiry dates for {} entries", vec_db_entries.len());
        Ok(vec_db_entries)
    }

    async fn get_categories(&self) -> Result<CategoriesResponse, AppError> {
        info!("Getting all categories of instruments");
        let result: CategoriesResponse = self.http_client.get("categories", Some(1)).await?;
        debug!("{} categories found", result.categories.len());
        Ok(result)
    }

    async fn get_category_instruments(
        &self,
        category_id: &str,
        page_number: Option<i32>,
        page_size: Option<i32>,
    ) -> Result<CategoryInstrumentsResponse, AppError> {
        let mut path = format!("categories/{}/instruments", category_id);

        let mut query_params = Vec::new();
        if let Some(page) = page_number {
            query_params.push(format!("pageNumber={}", page));
        }
        if let Some(size) = page_size {
            if size > 1000 {
                return Err(AppError::InvalidInput(
                    "pageSize cannot exceed 1000".to_string(),
                ));
            }
            query_params.push(format!("pageSize={}", size));
        }

        if !query_params.is_empty() {
            path = format!("{}?{}", path, query_params.join("&"));
        }

        info!(
            "Getting instruments for category: {} (page: {:?}, size: {:?})",
            category_id, page_number, page_size
        );
        let result: CategoryInstrumentsResponse = self.http_client.get(&path, Some(1)).await?;
        debug!(
            "{} instruments found in category {}",
            result.instruments.len(),
            category_id
        );
        Ok(result)
    }
}

#[async_trait]
impl AccountService for Client {
    async fn get_accounts(&self) -> Result<AccountsResponse, AppError> {
        info!("Getting account information");
        let result: AccountsResponse = self.http_client.get("accounts", Some(1)).await?;
        debug!(
            "Account information obtained: {} accounts",
            result.accounts.len()
        );
        Ok(result)
    }

    async fn get_positions(&self) -> Result<PositionsResponse, AppError> {
        debug!("Getting open positions");
        let result: PositionsResponse = self.http_client.get("positions", Some(2)).await?;
        debug!("Positions obtained: {} positions", result.positions.len());
        Ok(result)
    }

    async fn get_positions_w_filter(&self, filter: &str) -> Result<PositionsResponse, AppError> {
        debug!("Getting open positions with filter: {}", filter);
        let mut positions = self.get_positions().await?;

        positions
            .positions
            .retain(|position| position.market.epic.contains(filter));

        debug!(
            "Positions obtained after filtering: {} positions",
            positions.positions.len()
        );
        Ok(positions)
    }

    async fn get_working_orders(&self) -> Result<WorkingOrdersResponse, AppError> {
        info!("Getting working orders");
        let result: WorkingOrdersResponse = self.http_client.get("workingorders", Some(2)).await?;
        debug!(
            "Working orders obtained: {} orders",
            result.working_orders.len()
        );
        Ok(result)
    }

    async fn get_activity(
        &self,
        from: &str,
        to: &str,
    ) -> Result<AccountActivityResponse, AppError> {
        let path = format!("history/activity?from={}&to={}&pageSize=500", from, to);
        info!("Getting account activity");
        let result: AccountActivityResponse = self.http_client.get(&path, Some(3)).await?;
        debug!(
            "Account activity obtained: {} activities",
            result.activities.len()
        );
        Ok(result)
    }

    async fn get_activity_with_details(
        &self,
        from: &str,
        to: &str,
    ) -> Result<AccountActivityResponse, AppError> {
        let path = format!(
            "history/activity?from={}&to={}&detailed=true&pageSize=500",
            from, to
        );
        info!("Getting detailed account activity");
        let result: AccountActivityResponse = self.http_client.get(&path, Some(3)).await?;
        debug!(
            "Detailed account activity obtained: {} activities",
            result.activities.len()
        );
        Ok(result)
    }

    async fn get_transactions(
        &self,
        from: &str,
        to: &str,
    ) -> Result<TransactionHistoryResponse, AppError> {
        const PAGE_SIZE: u32 = 200;
        let mut all_transactions = Vec::new();
        let mut current_page = 1;
        #[allow(unused_assignments)]
        let mut last_metadata = None;

        loop {
            let path = format!(
                "history/transactions?from={}&to={}&pageSize={}&pageNumber={}",
                from, to, PAGE_SIZE, current_page
            );
            info!("Getting transaction history page {}", current_page);

            let result: TransactionHistoryResponse = self.http_client.get(&path, Some(2)).await?;

            let total_pages = result.metadata.page_data.total_pages as u32;
            last_metadata = Some(result.metadata);
            all_transactions.extend(result.transactions);

            if current_page >= total_pages {
                break;
            }
            current_page += 1;
        }

        debug!(
            "Total transaction history obtained: {} transactions",
            all_transactions.len()
        );

        Ok(TransactionHistoryResponse {
            transactions: all_transactions,
            metadata: last_metadata
                .ok_or_else(|| AppError::InvalidInput("Could not retrieve metadata".to_string()))?,
        })
    }

    async fn get_preferences(&self) -> Result<AccountPreferencesResponse, AppError> {
        info!("Getting account preferences");
        let result: AccountPreferencesResponse = self
            .http_client
            .get("accounts/preferences", Some(1))
            .await?;
        debug!(
            "Account preferences obtained: trailing_stops_enabled={}",
            result.trailing_stops_enabled
        );
        Ok(result)
    }

    async fn update_preferences(&self, trailing_stops_enabled: bool) -> Result<(), AppError> {
        info!(
            "Updating account preferences: trailing_stops_enabled={}",
            trailing_stops_enabled
        );
        let request = serde_json::json!({
            "trailingStopsEnabled": trailing_stops_enabled
        });
        let _: serde_json::Value = self
            .http_client
            .put("accounts/preferences", &request, Some(1))
            .await?;
        debug!("Account preferences updated");
        Ok(())
    }

    async fn get_activity_by_period(
        &self,
        period_ms: u64,
    ) -> Result<AccountActivityResponse, AppError> {
        let path = format!("history/activity/{}", period_ms);
        info!("Getting account activity for period: {} ms", period_ms);
        let result: AccountActivityResponse = self.http_client.get(&path, Some(1)).await?;
        debug!(
            "Account activity obtained: {} activities",
            result.activities.len()
        );
        Ok(result)
    }
}

#[async_trait]
impl OrderService for Client {
    async fn create_order(
        &self,
        order: &CreateOrderRequest,
    ) -> Result<CreateOrderResponse, AppError> {
        info!("Creating order for: {}", order.epic);
        let result: CreateOrderResponse = self
            .http_client
            .post("positions/otc", order, Some(2))
            .await?;
        debug!("Order created with reference: {}", result.deal_reference);
        Ok(result)
    }

    async fn get_order_confirmation(
        &self,
        deal_reference: &str,
    ) -> Result<OrderConfirmationResponse, AppError> {
        let path = format!("confirms/{}", deal_reference);
        info!("Getting confirmation for order: {}", deal_reference);
        let result: OrderConfirmationResponse = self.http_client.get(&path, Some(1)).await?;
        debug!("Confirmation obtained for order: {}", deal_reference);
        Ok(result)
    }

    async fn get_order_confirmation_w_retry(
        &self,
        deal_reference: &str,
        retries: u64,
        delay_ms: u64,
    ) -> Result<OrderConfirmationResponse, AppError> {
        // `delay_ms` is the backoff base; the actual per-attempt wait grows
        // exponentially (with jitter) via the shared `RetryConfig` policy.
        let base = Duration::from_millis(delay_ms);
        let mut attempt: u32 = 0;
        loop {
            match self.get_order_confirmation(deal_reference).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    // Only poll again on transient errors; permanent failures
                    // (auth, invalid input, deserialization) return immediately.
                    if !is_transient_confirmation_error(&e) {
                        return Err(e);
                    }
                    if u64::from(attempt) >= retries {
                        return Err(e);
                    }
                    let delay = backoff_delay(base, attempt);
                    let delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
                    let next_attempt = attempt.checked_add(1).ok_or_else(|| {
                        AppError::Generic("retry attempt counter overflow".to_string())
                    })?;
                    warn!(
                        deal_reference = %deal_reference,
                        attempt = next_attempt,
                        max_retries = retries,
                        delay_ms,
                        "retrying order confirmation after transient error"
                    );
                    sleep(delay).await;
                    attempt = next_attempt;
                }
            }
        }
    }

    async fn update_position(
        &self,
        deal_id: &str,
        update: &UpdatePositionRequest,
    ) -> Result<UpdatePositionResponse, AppError> {
        let path = format!("positions/otc/{}", deal_id);
        info!("Updating position: {}", deal_id);
        let result: UpdatePositionResponse = self.http_client.put(&path, update, Some(2)).await?;
        debug!(
            "Position updated: {} with deal reference: {}",
            deal_id, result.deal_reference
        );
        Ok(result)
    }

    async fn update_level_in_position(
        &self,
        deal_id: &str,
        limit_level: Option<f64>,
    ) -> Result<UpdatePositionResponse, AppError> {
        let path = format!("positions/otc/{}", deal_id);
        info!("Updating position: {}", deal_id);
        let limit_level = limit_level.unwrap_or(0.0);

        let update: UpdatePositionRequest = UpdatePositionRequest {
            guaranteed_stop: None,
            limit_level: Some(limit_level),
            stop_level: None,
            trailing_stop: None,
            trailing_stop_distance: None,
            trailing_stop_increment: None,
        };
        let result: UpdatePositionResponse = self.http_client.put(&path, update, Some(2)).await?;
        debug!(
            "Position updated: {} with deal reference: {}",
            deal_id, result.deal_reference
        );
        Ok(result)
    }

    async fn close_position(
        &self,
        close_request: &ClosePositionRequest,
    ) -> Result<ClosePositionResponse, AppError> {
        info!("Closing position");

        // IG API requires POST with _method: DELETE header for closing positions
        // This is a workaround for HTTP client limitations with DELETE + body
        let result: ClosePositionResponse = self
            .http_client
            .post_with_delete_method("positions/otc", close_request, Some(1))
            .await?;

        debug!("Position closed with reference: {}", result.deal_reference);
        Ok(result)
    }

    async fn create_working_order(
        &self,
        order: &CreateWorkingOrderRequest,
    ) -> Result<CreateWorkingOrderResponse, AppError> {
        info!("Creating working order for: {}", order.epic);
        let result: CreateWorkingOrderResponse = self
            .http_client
            .post("workingorders/otc", order, Some(2))
            .await?;
        debug!(
            "Working order created with reference: {}",
            result.deal_reference
        );
        Ok(result)
    }

    async fn delete_working_order(&self, deal_id: &str) -> Result<(), AppError> {
        let path = format!("workingorders/otc/{}", deal_id);
        let result: CreateWorkingOrderResponse =
            self.http_client.delete(path.as_str(), Some(2)).await?;
        debug!(
            "Working order created with reference: {}",
            result.deal_reference
        );
        Ok(())
    }

    async fn get_position(&self, deal_id: &str) -> Result<SinglePositionResponse, AppError> {
        let path = format!("positions/{}", deal_id);
        info!("Getting position: {}", deal_id);
        let result: SinglePositionResponse = self.http_client.get(&path, Some(2)).await?;
        debug!("Position obtained for deal: {}", deal_id);
        Ok(result)
    }

    async fn update_working_order(
        &self,
        deal_id: &str,
        update: &UpdateWorkingOrderRequest,
    ) -> Result<CreateWorkingOrderResponse, AppError> {
        let path = format!("workingorders/otc/{}", deal_id);
        info!("Updating working order: {}", deal_id);
        let result: CreateWorkingOrderResponse =
            self.http_client.put(&path, update, Some(2)).await?;
        debug!(
            "Working order updated: {} with reference: {}",
            deal_id, result.deal_reference
        );
        Ok(result)
    }
}

// ============================================================================
// WATCHLIST SERVICE IMPLEMENTATION
// ============================================================================

#[async_trait]
impl WatchlistService for Client {
    async fn get_watchlists(&self) -> Result<WatchlistsResponse, AppError> {
        info!("Getting all watchlists");
        let result: WatchlistsResponse = self.http_client.get("watchlists", Some(1)).await?;
        debug!(
            "Watchlists obtained: {} watchlists",
            result.watchlists.len()
        );
        Ok(result)
    }

    async fn create_watchlist(
        &self,
        name: &str,
        epics: Option<&[String]>,
    ) -> Result<CreateWatchlistResponse, AppError> {
        info!("Creating watchlist: {}", name);
        let request = CreateWatchlistRequest {
            name: name.to_string(),
            epics: epics.map(|e| e.to_vec()),
        };
        let result: CreateWatchlistResponse = self
            .http_client
            .post("watchlists", &request, Some(1))
            .await?;
        debug!(
            "Watchlist created: {} with ID: {}",
            name, result.watchlist_id
        );
        Ok(result)
    }

    async fn get_watchlist(
        &self,
        watchlist_id: &str,
    ) -> Result<WatchlistMarketsResponse, AppError> {
        let path = format!("watchlists/{}", watchlist_id);
        info!("Getting watchlist: {}", watchlist_id);
        let result: WatchlistMarketsResponse = self.http_client.get(&path, Some(1)).await?;
        debug!(
            "Watchlist obtained: {} with {} markets",
            watchlist_id,
            result.markets.len()
        );
        Ok(result)
    }

    async fn delete_watchlist(&self, watchlist_id: &str) -> Result<StatusResponse, AppError> {
        let path = format!("watchlists/{}", watchlist_id);
        info!("Deleting watchlist: {}", watchlist_id);
        let result: StatusResponse = self.http_client.delete(&path, Some(1)).await?;
        debug!("Watchlist deleted: {}", watchlist_id);
        Ok(result)
    }

    async fn add_to_watchlist(
        &self,
        watchlist_id: &str,
        epic: &str,
    ) -> Result<StatusResponse, AppError> {
        let path = format!("watchlists/{}", watchlist_id);
        info!("Adding {} to watchlist: {}", epic, watchlist_id);
        let request = AddToWatchlistRequest {
            epic: epic.to_string(),
        };
        let result: StatusResponse = self.http_client.put(&path, &request, Some(1)).await?;
        debug!("Added {} to watchlist: {}", epic, watchlist_id);
        Ok(result)
    }

    async fn remove_from_watchlist(
        &self,
        watchlist_id: &str,
        epic: &str,
    ) -> Result<StatusResponse, AppError> {
        let path = format!("watchlists/{}/{}", watchlist_id, epic);
        info!("Removing {} from watchlist: {}", epic, watchlist_id);
        let result: StatusResponse = self.http_client.delete(&path, Some(1)).await?;
        debug!("Removed {} from watchlist: {}", epic, watchlist_id);
        Ok(result)
    }
}

// ============================================================================
// SENTIMENT SERVICE IMPLEMENTATION
// ============================================================================

#[async_trait]
impl SentimentService for Client {
    async fn get_client_sentiment(
        &self,
        market_ids: &[String],
    ) -> Result<ClientSentimentResponse, AppError> {
        let market_ids_str = market_ids.join(",");
        let path = format!("clientsentiment?marketIds={}", market_ids_str);
        info!("Getting client sentiment for {} markets", market_ids.len());
        let result: ClientSentimentResponse = self.http_client.get(&path, Some(1)).await?;
        debug!(
            "Client sentiment obtained for {} markets",
            result.client_sentiments.len()
        );
        Ok(result)
    }

    async fn get_client_sentiment_by_market(
        &self,
        market_id: &str,
    ) -> Result<MarketSentiment, AppError> {
        let path = format!("clientsentiment/{}", market_id);
        info!("Getting client sentiment for market: {}", market_id);
        let result: MarketSentiment = self.http_client.get(&path, Some(1)).await?;
        debug!(
            "Client sentiment for {}: {}% long, {}% short",
            market_id, result.long_position_percentage, result.short_position_percentage
        );
        Ok(result)
    }

    async fn get_related_sentiment(
        &self,
        market_id: &str,
    ) -> Result<ClientSentimentResponse, AppError> {
        let path = format!("clientsentiment/related/{}", market_id);
        info!("Getting related sentiment for market: {}", market_id);
        let result: ClientSentimentResponse = self.http_client.get(&path, Some(1)).await?;
        debug!(
            "Related sentiment obtained: {} markets",
            result.client_sentiments.len()
        );
        Ok(result)
    }
}

// ============================================================================
// COSTS SERVICE IMPLEMENTATION
// ============================================================================

#[async_trait]
impl CostsService for Client {
    async fn get_indicative_costs_open(
        &self,
        request: &OpenCostsRequest,
    ) -> Result<IndicativeCostsResponse, AppError> {
        info!(
            "Getting indicative costs for opening position on: {}",
            request.epic
        );
        let result: IndicativeCostsResponse = self
            .http_client
            .post("indicativecostsandcharges/open", request, Some(1))
            .await?;
        debug!(
            "Indicative costs obtained, reference: {}",
            result.indicative_quote_reference
        );
        Ok(result)
    }

    async fn get_indicative_costs_close(
        &self,
        request: &CloseCostsRequest,
    ) -> Result<IndicativeCostsResponse, AppError> {
        info!(
            "Getting indicative costs for closing position: {}",
            request.deal_id
        );
        let result: IndicativeCostsResponse = self
            .http_client
            .post("indicativecostsandcharges/close", request, Some(1))
            .await?;
        debug!(
            "Indicative costs obtained, reference: {}",
            result.indicative_quote_reference
        );
        Ok(result)
    }

    async fn get_indicative_costs_edit(
        &self,
        request: &EditCostsRequest,
    ) -> Result<IndicativeCostsResponse, AppError> {
        info!(
            "Getting indicative costs for editing position: {}",
            request.deal_id
        );
        let result: IndicativeCostsResponse = self
            .http_client
            .post("indicativecostsandcharges/edit", request, Some(1))
            .await?;
        debug!(
            "Indicative costs obtained, reference: {}",
            result.indicative_quote_reference
        );
        Ok(result)
    }

    async fn get_costs_history(
        &self,
        from: &str,
        to: &str,
    ) -> Result<CostsHistoryResponse, AppError> {
        let path = format!("indicativecostsandcharges/history/from/{}/to/{}", from, to);
        info!("Getting costs history from {} to {}", from, to);
        let result: CostsHistoryResponse = self.http_client.get(&path, Some(1)).await?;
        debug!("Costs history obtained: {} entries", result.costs.len());
        Ok(result)
    }

    async fn get_durable_medium(
        &self,
        quote_reference: &str,
    ) -> Result<DurableMediumResponse, AppError> {
        let path = format!(
            "indicativecostsandcharges/durablemedium/{}",
            quote_reference
        );
        info!("Getting durable medium for reference: {}", quote_reference);
        let result: DurableMediumResponse = self.http_client.get(&path, Some(1)).await?;
        debug!("Durable medium obtained for reference: {}", quote_reference);
        Ok(result)
    }
}

// ============================================================================
// OPERATIONS SERVICE IMPLEMENTATION
// ============================================================================

#[async_trait]
impl OperationsService for Client {
    async fn get_client_apps(&self) -> Result<ApplicationDetailsResponse, AppError> {
        info!("Getting client applications");
        let result: ApplicationDetailsResponse = self
            .http_client
            .get("operations/application", Some(1))
            .await?;
        // Never log `api_key`: it is a live credential.
        debug!(
            name = ?result.name,
            status = %result.status,
            "Client application obtained"
        );
        Ok(result)
    }

    async fn disable_client_app(&self) -> Result<StatusResponse, AppError> {
        info!("Disabling current client application");
        let result: StatusResponse = self
            .http_client
            .put(
                "operations/application/disable",
                &serde_json::json!({}),
                Some(1),
            )
            .await?;
        debug!("Client application disabled");
        Ok(result)
    }
}

/// Streaming client for IG Markets real-time data.
///
/// This client manages two Lightstreamer connections for different data types:
/// - **Market streamer**: Handles market data (prices, market state), trade updates (CONFIRMS, OPU, WOU),
///   and account updates (positions, orders, balance). Uses the default adapter.
/// - **Price streamer**: Handles detailed price data (bid/ask levels, sizes, multiple currencies).
///   Uses the "Pricing" adapter.
///
/// Each connection type can be managed independently and runs in parallel.
pub struct StreamerClient {
    account_id: String,
    market_streamer_client: Option<Arc<Mutex<LightstreamerClient>>>,
    price_streamer_client: Option<Arc<Mutex<LightstreamerClient>>>,
    // Flags indicating whether there is at least one active subscription for each client
    has_market_stream_subs: bool,
    has_price_stream_subs: bool,
}

impl StreamerClient {
    /// Creates a new streaming client instance.
    ///
    /// This initializes both streaming clients (market and price) but does not
    /// establish connections yet. Connections are established when `connect()` is called.
    ///
    /// # Returns
    ///
    /// Returns a new `StreamerClient` instance or an error if initialization fails.
    pub async fn new() -> Result<Self, AppError> {
        let http_client_raw = Arc::new(RwLock::new(Client::new()));
        let http_client = http_client_raw.read().await;
        let ws_info = http_client.ws_info().await?;
        let password = ws_info.get_ws_password();

        // Market data client (no adapter specified - uses default)
        let market_streamer_client = Arc::new(Mutex::new(LightstreamerClient::new(
            Some(ws_info.server.as_str()),
            None,
            Some(&ws_info.account_id),
            Some(&password),
        )?));

        let price_streamer_client = Arc::new(Mutex::new(LightstreamerClient::new(
            Some(ws_info.server.as_str()),
            None,
            Some(&ws_info.account_id),
            Some(&password),
        )?));

        // Force WebSocket streaming transport on both clients to satisfy IG requirements
        // and configure logging to use tracing levels for proper log propagation
        {
            let mut client = market_streamer_client.lock().await;
            client
                .connection_options
                .set_forced_transport(Some(Transport::WsStreaming));
            client.set_logging_type(LogType::TracingLogs);
        }
        {
            let mut client = price_streamer_client.lock().await;
            client
                .connection_options
                .set_forced_transport(Some(Transport::WsStreaming));
            client.set_logging_type(LogType::TracingLogs);
        }

        Ok(Self {
            account_id: ws_info.account_id.clone(),
            market_streamer_client: Some(market_streamer_client),
            price_streamer_client: Some(price_streamer_client),
            has_market_stream_subs: false,
            has_price_stream_subs: false,
        })
    }

    /// Creates a default streaming client instance.
    pub async fn default() -> Result<Self, AppError> {
        Self::new().await
    }

    /// Subscribes to market data updates for the specified instruments.
    ///
    /// This method creates a subscription to receive real-time market data updates
    /// for the given EPICs and returns a channel receiver for consuming the updates.
    ///
    /// # Arguments
    ///
    /// * `epics` - List of instrument EPICs to subscribe to
    /// * `fields` - Set of market data fields to receive (e.g., BID, OFFER, etc.)
    ///
    /// # Returns
    ///
    /// Returns a receiver channel for `PriceData` updates, or an error if
    /// the subscription setup failed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut receiver = client.market_subscribe(
    ///     vec!["IX.D.DAX.DAILY.IP".to_string()],
    ///     fields
    /// ).await?;
    ///
    /// tokio::spawn(async move {
    ///     while let Some(price_data) = receiver.recv().await {
    ///         println!("Price update: {:?}", price_data);
    ///     }
    /// });
    /// ```
    pub async fn market_subscribe(
        &mut self,
        epics: Vec<String>,
        fields: HashSet<StreamingMarketField>,
    ) -> Result<mpsc::UnboundedReceiver<PriceData>, AppError> {
        // Mark that we have at least one subscription on the market streamer
        self.has_market_stream_subs = true;

        let fields = get_streaming_market_fields(&fields);
        let market_epics: Vec<String> = epics
            .iter()
            .map(|epic| "MARKET:".to_string() + epic)
            .collect();
        let mut subscription =
            Subscription::new(SubscriptionMode::Merge, Some(market_epics), Some(fields))?;

        subscription.set_data_adapter(None)?;
        subscription.set_requested_snapshot(Some(Snapshot::Yes))?;

        // Create channel listener that converts ItemUpdate to PriceData
        let (listener, item_receiver) = ChannelSubscriptionListener::create_channel();
        subscription.add_listener(Box::new(listener));

        // Configure client and add subscription
        let client = self.market_streamer_client.as_ref().ok_or_else(|| {
            AppError::WebSocketError("market streamer client not initialized".to_string())
        })?;

        {
            let mut client = client.lock().await;
            client
                .connection_options
                .set_forced_transport(Some(Transport::WsStreaming));
            LightstreamerClient::subscribe(client.subscription_sender.clone(), subscription)
                .await?;
        }

        // Create a channel for PriceData and spawn a task to convert ItemUpdate to PriceData
        let (price_tx, price_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut receiver = item_receiver;
            while let Some(item_update) = receiver.recv().await {
                let price_data = PriceData::from(&item_update);
                if price_tx.send(price_data).is_err() {
                    tracing::debug!("Price channel receiver dropped");
                    break;
                }
            }
        });

        info!(
            "Market subscription created for {} instruments",
            epics.len()
        );
        Ok(price_rx)
    }

    /// Subscribes to trade updates for the account.
    ///
    /// This method creates a subscription to receive real-time trade confirmations,
    /// order updates (OPU), and working order updates (WOU) for the account,
    /// and returns a channel receiver for consuming the updates.
    ///
    /// # Returns
    ///
    /// Returns a receiver channel for `TradeFields` updates, or an error if
    /// the subscription setup failed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut receiver = client.trade_subscribe().await?;
    ///
    /// tokio::spawn(async move {
    ///     while let Some(trade_fields) = receiver.recv().await {
    ///         println!("Trade update: {:?}", trade_fields);
    ///     }
    /// });
    /// ```
    pub async fn trade_subscribe(
        &mut self,
    ) -> Result<mpsc::UnboundedReceiver<TradeFields>, AppError> {
        // Mark that we have at least one subscription on the market streamer
        self.has_market_stream_subs = true;

        let account_id = self.account_id.clone();
        let fields = Some(vec![
            "CONFIRMS".to_string(),
            "OPU".to_string(),
            "WOU".to_string(),
        ]);
        let trade_items = vec![format!("TRADE:{account_id}")];

        let mut subscription =
            Subscription::new(SubscriptionMode::Distinct, Some(trade_items), fields)?;

        subscription.set_data_adapter(None)?;
        subscription.set_requested_snapshot(Some(Snapshot::Yes))?;

        // Create channel listener
        let (listener, item_receiver) = ChannelSubscriptionListener::create_channel();
        subscription.add_listener(Box::new(listener));

        // Configure client and add subscription (reusing market_streamer_client)
        let client = self.market_streamer_client.as_ref().ok_or_else(|| {
            AppError::WebSocketError("market streamer client not initialized".to_string())
        })?;

        {
            let mut client = client.lock().await;
            client
                .connection_options
                .set_forced_transport(Some(Transport::WsStreaming));
            LightstreamerClient::subscribe(client.subscription_sender.clone(), subscription)
                .await?;
        }

        // Create a channel for TradeFields and spawn a task to convert ItemUpdate to TradeFields
        let (trade_tx, trade_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut receiver = item_receiver;
            while let Some(item_update) = receiver.recv().await {
                let trade_data = crate::presentation::trade::TradeData::from(&item_update);
                if trade_tx.send(trade_data.fields).is_err() {
                    tracing::debug!("Trade channel receiver dropped");
                    break;
                }
            }
        });

        info!("Trade subscription created for account: {}", account_id);
        Ok(trade_rx)
    }

    /// Subscribes to account data updates.
    ///
    /// This method creates a subscription to receive real-time account updates including
    /// profit/loss, margin, equity, available funds, and other account metrics,
    /// and returns a channel receiver for consuming the updates.
    ///
    /// # Arguments
    ///
    /// * `fields` - Set of account data fields to receive (e.g., PNL, MARGIN, EQUITY, etc.)
    ///
    /// # Returns
    ///
    /// Returns a receiver channel for `AccountFields` updates, or an error if
    /// the subscription setup failed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut receiver = client.account_subscribe(fields).await?;
    ///
    /// tokio::spawn(async move {
    ///     while let Some(account_fields) = receiver.recv().await {
    ///         println!("Account update: {:?}", account_fields);
    ///     }
    /// });
    /// ```
    pub async fn account_subscribe(
        &mut self,
        fields: HashSet<StreamingAccountDataField>,
    ) -> Result<mpsc::UnboundedReceiver<AccountFields>, AppError> {
        // Mark that we have at least one subscription on the market streamer
        self.has_market_stream_subs = true;

        let fields = get_streaming_account_data_fields(&fields);
        let account_id = self.account_id.clone();
        let account_items = vec![format!("ACCOUNT:{account_id}")];

        let mut subscription =
            Subscription::new(SubscriptionMode::Merge, Some(account_items), Some(fields))?;

        subscription.set_data_adapter(None)?;
        subscription.set_requested_snapshot(Some(Snapshot::Yes))?;

        // Create channel listener
        let (listener, item_receiver) = ChannelSubscriptionListener::create_channel();
        subscription.add_listener(Box::new(listener));

        // Configure client and add subscription (reusing market_streamer_client)
        let client = self.market_streamer_client.as_ref().ok_or_else(|| {
            AppError::WebSocketError("market streamer client not initialized".to_string())
        })?;

        {
            let mut client = client.lock().await;
            client
                .connection_options
                .set_forced_transport(Some(Transport::WsStreaming));
            LightstreamerClient::subscribe(client.subscription_sender.clone(), subscription)
                .await?;
        }

        // Create a channel for AccountFields and spawn a task to convert ItemUpdate to AccountFields
        let (account_tx, account_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut receiver = item_receiver;
            while let Some(item_update) = receiver.recv().await {
                let account_data = crate::presentation::account::AccountData::from(&item_update);
                if account_tx.send(account_data.fields).is_err() {
                    tracing::debug!("Account channel receiver dropped");
                    break;
                }
            }
        });

        info!("Account subscription created for account: {}", account_id);
        Ok(account_rx)
    }

    /// Subscribes to price data updates for the specified instruments.
    ///
    /// This method creates a subscription to receive real-time price updates including
    /// bid/ask prices, sizes, and multiple currency levels for the given EPICs,
    /// and returns a channel receiver for consuming the updates.
    ///
    /// # Arguments
    ///
    /// * `epics` - List of instrument EPICs to subscribe to
    /// * `fields` - Set of price data fields to receive (e.g., BID_PRICE1, ASK_PRICE1, etc.)
    ///
    /// # Returns
    ///
    /// Returns a receiver channel for `PriceData` updates, or an error if
    /// the subscription setup failed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut receiver = client.price_subscribe(
    ///     vec!["IX.D.DAX.DAILY.IP".to_string()],
    ///     fields
    /// ).await?;
    ///
    /// tokio::spawn(async move {
    ///     while let Some(price_data) = receiver.recv().await {
    ///         println!("Price update: {:?}", price_data);
    ///     }
    /// });
    /// ```
    pub async fn price_subscribe(
        &mut self,
        epics: Vec<String>,
        fields: HashSet<StreamingPriceField>,
    ) -> Result<mpsc::UnboundedReceiver<PriceData>, AppError> {
        // Mark that we have at least one subscription on the price streamer
        self.has_price_stream_subs = true;

        let fields = get_streaming_price_fields(&fields);
        let account_id = self.account_id.clone();
        let price_epics: Vec<String> = epics
            .iter()
            .map(|epic| format!("PRICE:{account_id}:{epic}"))
            .collect();

        // Debug what we are about to subscribe to (items and fields)
        tracing::debug!("Pricing subscribe items: {:?}", price_epics);
        tracing::debug!("Pricing subscribe fields: {:?}", fields);

        let mut subscription =
            Subscription::new(SubscriptionMode::Merge, Some(price_epics), Some(fields))?;

        // Allow overriding the Pricing adapter name via env var to match server config
        let pricing_adapter =
            std::env::var("IG_PRICING_ADAPTER").unwrap_or_else(|_| "Pricing".to_string());
        tracing::debug!("Using Pricing data adapter: {}", pricing_adapter);
        subscription.set_data_adapter(Some(pricing_adapter))?;
        subscription.set_requested_snapshot(Some(Snapshot::Yes))?;

        // Create channel listener
        let (listener, item_receiver) = ChannelSubscriptionListener::create_channel();
        subscription.add_listener(Box::new(listener));

        // Configure client and add subscription
        let client = self.price_streamer_client.as_ref().ok_or_else(|| {
            AppError::WebSocketError("price streamer client not initialized".to_string())
        })?;

        {
            let mut client = client.lock().await;
            client
                .connection_options
                .set_forced_transport(Some(Transport::WsStreaming));
            LightstreamerClient::subscribe(client.subscription_sender.clone(), subscription)
                .await?;
        }

        // Create a channel for PriceData and spawn a task to convert ItemUpdate to PriceData
        let (price_tx, price_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut receiver = item_receiver;
            while let Some(item_update) = receiver.recv().await {
                let price_data = PriceData::from(&item_update);
                if price_tx.send(price_data).is_err() {
                    tracing::debug!("Price channel receiver dropped");
                    break;
                }
            }
        });

        info!(
            "Price subscription created for {} instruments (account: {})",
            epics.len(),
            account_id
        );
        Ok(price_rx)
    }

    /// Subscribes to chart data updates for the specified instruments and scale.
    ///
    /// This method creates a subscription to receive real-time chart updates including
    /// OHLC data, volume, and other chart metrics for the given EPICs and chart scale,
    /// and returns a channel receiver for consuming the updates.
    ///
    /// # Arguments
    ///
    /// * `epics` - List of instrument EPICs to subscribe to.
    /// * `scale` - Chart scale (e.g., Tick, 1Min, 5Min, etc.).
    /// * `fields` - Set of chart data fields to receive (e.g., OPEN, HIGH, LOW, CLOSE, VOLUME).
    ///
    /// # Returns
    ///
    /// Returns a receiver channel for `ChartData` updates, or an error if
    /// the subscription setup failed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut receiver = client.chart_subscribe(
    ///     vec!["IX.D.DAX.DAILY.IP".to_string()],
    ///     ChartScale::OneMin,
    ///     fields
    /// ).await?;
    ///
    /// tokio::spawn(async move {
    ///     while let Some(chart_data) = receiver.recv().await {
    ///         println!("Chart update: {:?}", chart_data);
    ///     }
    /// });
    /// ```
    pub async fn chart_subscribe(
        &mut self,
        epics: Vec<String>,
        scale: ChartScale,
        fields: HashSet<StreamingChartField>,
    ) -> Result<mpsc::UnboundedReceiver<ChartData>, AppError> {
        // Mark that we have at least one subscription on the market streamer
        self.has_market_stream_subs = true;

        let fields = get_streaming_chart_fields(&fields);

        let chart_items: Vec<String> = epics
            .iter()
            .map(|epic| format!("CHART:{epic}:{scale}",))
            .collect();

        // Candle data uses MERGE mode, tick data uses DISTINCT
        let mode = if matches!(scale, ChartScale::Tick) {
            SubscriptionMode::Distinct
        } else {
            SubscriptionMode::Merge
        };

        let mut subscription = Subscription::new(mode, Some(chart_items), Some(fields))?;

        subscription.set_data_adapter(None)?;
        subscription.set_requested_snapshot(Some(Snapshot::Yes))?;

        // Create channel listener
        let (listener, item_receiver) = ChannelSubscriptionListener::create_channel();
        subscription.add_listener(Box::new(listener));

        // Configure client and add subscription (reusing market_streamer_client)
        let client = self.market_streamer_client.as_ref().ok_or_else(|| {
            AppError::WebSocketError("market streamer client not initialized".to_string())
        })?;

        {
            let mut client = client.lock().await;
            client
                .connection_options
                .set_forced_transport(Some(Transport::WsStreaming));
            LightstreamerClient::subscribe(client.subscription_sender.clone(), subscription)
                .await?;
        }

        // Create a channel for ChartData and spawn a task to convert ItemUpdate to ChartData
        let (chart_tx, chart_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut receiver = item_receiver;
            while let Some(item_update) = receiver.recv().await {
                let chart_data = ChartData::from(&item_update);
                if chart_tx.send(chart_data).is_err() {
                    tracing::debug!("Chart channel receiver dropped");
                    break;
                }
            }
        });

        info!(
            "Chart subscription created for {} instruments (scale: {})",
            epics.len(),
            scale
        );

        Ok(chart_rx)
    }

    /// Connects all active Lightstreamer clients and maintains the connections.
    ///
    /// This method establishes connections for all streaming clients that have active
    /// subscriptions (market and price). Each client runs in its own task and
    /// all connections are maintained until a shutdown signal is received.
    ///
    /// # Arguments
    ///
    /// * `shutdown_signal` - Optional signal to gracefully shutdown all connections.
    ///   If None, a default signal handler for SIGINT/SIGTERM will be created.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when all connections are closed gracefully, or an error if
    /// any connection fails after maximum retry attempts.
    ///
    pub async fn connect(&mut self, shutdown_signal: Option<Arc<Notify>>) -> Result<(), AppError> {
        // Use provided signal or create a new one with signal hooks
        let signal = if let Some(sig) = shutdown_signal {
            sig
        } else {
            let sig = Arc::new(Notify::new());
            setup_signal_hook(Arc::clone(&sig)).await;
            sig
        };

        let mut tasks = Vec::new();

        // Connect market streamer only if there are active subscriptions
        if self.has_market_stream_subs {
            if let Some(client) = self.market_streamer_client.as_ref() {
                let client = Arc::clone(client);
                let signal = Arc::clone(&signal);
                let task =
                    tokio::spawn(
                        async move { Self::connect_client(client, signal, "Market").await },
                    );
                tasks.push(task);
            }
        } else {
            info!("Skipping Market streamer connection: no active subscriptions");
        }

        // Connect price streamer only if there are active subscriptions
        if self.has_price_stream_subs {
            if let Some(client) = self.price_streamer_client.as_ref() {
                let client = Arc::clone(client);
                let signal = Arc::clone(&signal);
                let task =
                    tokio::spawn(
                        async move { Self::connect_client(client, signal, "Price").await },
                    );
                tasks.push(task);
            }
        } else {
            info!("Skipping Price streamer connection: no active subscriptions");
        }

        if tasks.is_empty() {
            warn!("No streaming clients selected for connection (no active subscriptions)");
            return Ok(());
        }

        info!("Connecting {} streaming client(s)...", tasks.len());

        // Wait for all tasks to complete
        let results = futures::future::join_all(tasks).await;

        // Check if any task failed
        let mut has_error = false;
        for (idx, result) in results.iter().enumerate() {
            match result {
                Ok(Ok(_)) => {
                    debug!("Streaming client {} completed successfully", idx);
                }
                Ok(Err(e)) => {
                    error!("Streaming client {} failed: {:?}", idx, e);
                    has_error = true;
                }
                Err(e) => {
                    error!("Streaming client {} task panicked: {:?}", idx, e);
                    has_error = true;
                }
            }
        }

        if has_error {
            return Err(AppError::WebSocketError(
                "one or more streaming connections failed".to_string(),
            ));
        }

        info!("All streaming connections closed gracefully");
        Ok(())
    }

    /// Internal helper to connect a single Lightstreamer client with retry logic.
    async fn connect_client(
        client: Arc<Mutex<LightstreamerClient>>,
        signal: Arc<Notify>,
        client_type: &str,
    ) -> Result<(), AppError> {
        let mut retry_interval_millis: u64 = 0;
        let mut retry_counter: u64 = 0;

        while retry_counter < MAX_CONNECTION_ATTEMPTS {
            let connect_result = {
                let mut client = client.lock().await;
                client.connect_direct(Arc::clone(&signal)).await
            };

            // Convert error to String immediately to avoid Send issues
            let result_with_string_error = connect_result.map_err(|e| format!("{:?}", e));

            match result_with_string_error {
                Ok(_) => {
                    info!("{} streamer connected successfully", client_type);
                    break;
                }
                Err(error_msg) => {
                    // If server closed because there are no active subscriptions, treat as graceful
                    if error_msg.contains("No more requests to fulfill") {
                        info!(
                            "{} streamer closed gracefully: no active subscriptions (server reason: No more requests to fulfill)",
                            client_type
                        );
                        return Ok(());
                    }

                    error!("{} streamer connection failed: {}", client_type, error_msg);

                    if retry_counter < MAX_CONNECTION_ATTEMPTS - 1 {
                        tokio::time::sleep(std::time::Duration::from_millis(retry_interval_millis))
                            .await;
                        retry_interval_millis =
                            (retry_interval_millis + (200 * retry_counter)).min(5000);
                        retry_counter += 1;
                        warn!(
                            "{} streamer retrying (attempt {}/{}) in {:.2} seconds...",
                            client_type,
                            retry_counter + 1,
                            MAX_CONNECTION_ATTEMPTS,
                            retry_interval_millis as f64 / 1000.0
                        );
                    } else {
                        retry_counter += 1;
                    }
                }
            }
        }

        if retry_counter >= MAX_CONNECTION_ATTEMPTS {
            error!(
                "{} streamer failed after {} attempts",
                client_type, MAX_CONNECTION_ATTEMPTS
            );
            return Err(AppError::WebSocketError(format!(
                "{} streamer: maximum connection attempts ({}) exceeded",
                client_type, MAX_CONNECTION_ATTEMPTS
            )));
        }

        info!("{} streamer connection closed gracefully", client_type);
        Ok(())
    }

    /// Disconnects all active Lightstreamer clients.
    ///
    /// This method gracefully closes all streaming connections (market and price).
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all disconnections were successful.
    pub async fn disconnect(&mut self) -> Result<(), AppError> {
        let mut disconnected = 0;

        if let Some(client) = self.market_streamer_client.as_ref() {
            let mut client = client.lock().await;
            client.disconnect().await;
            info!("Market streamer disconnected");
            disconnected += 1;
        }

        if let Some(client) = self.price_streamer_client.as_ref() {
            let mut client = client.lock().await;
            client.disconnect().await;
            info!("Price streamer disconnected");
            disconnected += 1;
        }

        info!("Disconnected {} streaming client(s)", disconnected);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::is_transient_confirmation_error;
    use crate::error::AppError;
    use reqwest::StatusCode;

    #[test]
    fn test_confirmation_error_rate_limit_is_transient() {
        assert!(is_transient_confirmation_error(
            &AppError::RateLimitExceeded
        ));
    }

    #[test]
    fn test_confirmation_error_not_found_is_transient() {
        // Confirmation not yet available: IG returns 404 until the deal settles.
        assert!(is_transient_confirmation_error(&AppError::NotFound));
        assert!(is_transient_confirmation_error(&AppError::Unexpected(
            StatusCode::NOT_FOUND
        )));
    }

    #[test]
    fn test_confirmation_error_server_error_is_transient() {
        assert!(is_transient_confirmation_error(&AppError::Unexpected(
            StatusCode::INTERNAL_SERVER_ERROR
        )));
        assert!(is_transient_confirmation_error(&AppError::Unexpected(
            StatusCode::BAD_GATEWAY
        )));
    }

    #[test]
    fn test_confirmation_error_invalid_input_is_permanent() {
        assert!(!is_transient_confirmation_error(&AppError::InvalidInput(
            "bad".to_string()
        )));
    }

    #[test]
    fn test_confirmation_error_auth_and_deser_are_permanent() {
        assert!(!is_transient_confirmation_error(&AppError::Unauthorized));
        assert!(!is_transient_confirmation_error(
            &AppError::OAuthTokenExpired
        ));
        assert!(!is_transient_confirmation_error(
            &AppError::Deserialization("bad".to_string())
        ));
        assert!(!is_transient_confirmation_error(&AppError::Unexpected(
            StatusCode::BAD_REQUEST
        )));
    }
}
