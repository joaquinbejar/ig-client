/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 19/10/25
******************************************************************************/
use crate::application::auth::WebsocketInfo;
use crate::application::config::Config;
use crate::application::http::HttpClient;
use crate::application::interfaces::account::AccountService;
use crate::application::interfaces::costs::CostsService;
use crate::application::interfaces::market::MarketService;
use crate::application::interfaces::operations::OperationsService;
use crate::application::interfaces::order::OrderService;
use crate::application::interfaces::sentiment::SentimentService;
use crate::application::interfaces::watchlist::WatchlistService;
#[cfg(feature = "streaming")]
use crate::application::streaming_convert::StreamingUpdate;
use crate::error::AppError;
use crate::model::requests::RecentPricesRequest;
use crate::model::requests::{
    AddToWatchlistRequest, CloseCostsRequest, CreateWatchlistRequest, EditCostsRequest,
    OpenCostsRequest, UpdateWorkingOrderRequest,
};
use crate::model::requests::{
    ClosePositionRequest, CreateOrderRequest, CreateWorkingOrderRequest, UpdatePositionRequest,
};
use crate::model::responses::{
    AccountActivityResponse, AccountsResponse, OrderConfirmationResponse, PositionsResponse,
    TransactionHistoryResponse, WorkingOrdersResponse,
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
#[cfg(feature = "streaming")]
use crate::model::streaming::{
    StreamingAccountDataField, StreamingChartField, StreamingMarketField, StreamingPriceField,
    get_streaming_account_data_fields, get_streaming_chart_fields, get_streaming_market_fields,
    get_streaming_price_fields,
};
#[cfg(feature = "streaming")]
use crate::presentation::account::AccountFields;
#[cfg(feature = "streaming")]
use crate::presentation::chart::{ChartData, ChartScale};
use crate::presentation::market::{MarketData, MarketDetails};
#[cfg(feature = "streaming")]
use crate::presentation::price::PriceData;
#[cfg(feature = "streaming")]
use crate::presentation::trade::TradeFields;
use async_trait::async_trait;
use futures::StreamExt;
#[cfg(feature = "streaming")]
use lightstreamer_rs::{
    Client as LsClient, ClientConfig, ClosedReason, Continuity, Credentials, FieldSchema,
    ItemGroup, ServerAddress, SessionEvent, SessionEvents, Snapshot, Subscription,
    SubscriptionEvent, SubscriptionMode,
};
use reqwest::StatusCode;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
#[cfg(feature = "streaming")]
use tokio::sync::{Notify, mpsc, watch};
#[cfg(feature = "streaming")]
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, info, warn};
#[cfg(feature = "streaming")]
use tracing::{error, trace};

/// Maximum number of concurrent `get_market_details` requests issued while
/// resolving per-symbol expiry dates in [`Client::get_vec_db_entries`].
///
/// Kept small so the shared rate limiter stays in control: this only overlaps
/// network latency, it does not widen the request budget.
const MARKET_DETAILS_CONCURRENCY: usize = 6;

/// Awaits every task in `tasks` and then clears the list.
///
/// The caller must have signalled the tasks to stop first (see
/// [`StreamerClient::disconnect`]); this only waits for them to observe it, so
/// no update is dropped mid-conversion. A task that panicked yields a
/// [`tokio::task::JoinError`], which is logged and otherwise ignored — teardown
/// must not fail because a converter did.
#[cfg(feature = "streaming")]
async fn join_tasks(tasks: &mut Vec<JoinHandle<()>>) {
    for handle in tasks.drain(..) {
        if let Err(e) = handle.await {
            warn!(error = %e, "streaming converter task did not exit cleanly");
        }
    }
}

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

/// Appends a `Z` zone designator to a zone-less ISO-8601 timestamp.
///
/// IG's costs-history endpoint parses `from`/`to` as ISO-8601 instants and
/// rejects zone-less timestamps with a 500 (`could not be parsed at index
/// 19`). Callers across this crate pass the same zone-less local ISO form
/// the other history endpoints accept (`2026-01-01T00:00:00`), so this
/// helper appends `Z` when no designator (`Z` or a `±hh:mm` offset after
/// the time part) is present. Inputs that already carry a designator pass
/// through unchanged.
#[must_use]
fn ensure_zone_designator(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.ends_with('Z') {
        return trimmed.to_string();
    }
    let Some(time_index) = trimmed.find('T') else {
        // Date-only input: expand to midnight UTC.
        return format!("{trimmed}T00:00:00Z");
    };
    let has_offset = trimmed
        .get(time_index..)
        .is_some_and(|time_part| time_part.contains('+') || time_part.contains('-'));
    if has_offset {
        trimmed.to_string()
    } else {
        format!("{trimmed}Z")
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
    /// Creates a new client instance from the environment, without performing
    /// initial authentication, returning an error if the underlying HTTP client
    /// cannot be constructed.
    ///
    /// This is the environment convenience path: the configuration comes from
    /// [`Config::default`], which loads a local `.env` file and reads the
    /// `IG_*` environment namespace. Embedders that supply their own
    /// configuration should use [`Client::with_config`] instead, which touches
    /// neither.
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

    /// Creates a new client instance from a caller-supplied [`Config`], without
    /// reading a `.env` file or the `IG_*` environment namespace.
    ///
    /// This is the injection path for applications that own their
    /// configuration source (their own namespaced environment variables, a
    /// config file, a secrets manager) and must not have the crate reach for
    /// globals. Build the `Config` with
    /// [`Config::from_credentials`](crate::application::config::Config::from_credentials),
    /// which is likewise env-free; [`Client::try_new`] remains the `.env`
    /// convenience path.
    ///
    /// As with [`try_new`](Self::try_new), no authentication is performed here:
    /// session login and token refresh happen transparently on the first API
    /// call.
    ///
    /// Two knobs live outside [`Config`] and are still resolved from the
    /// process environment on this path: the retry policy (`MAX_RETRY_COUNT` /
    /// `RETRY_DELAY_SECS`, read per request by
    /// [`RetryConfig::default`](crate::model::retry::RetryConfig)) and
    /// `IG_PRICING_ADAPTER` (the Lightstreamer price adapter name). Neither
    /// carries a credential and both have safe defaults, so an embedder that
    /// sets neither is unaffected.
    ///
    /// For streaming, pair this with
    /// `StreamerClient::with_client`:
    /// `StreamerClient::new`
    /// builds its own client via [`try_new`](Self::try_new) and would go back
    /// to the `.env` / `IG_*` path.
    ///
    /// ```rust,no_run
    /// use ig_client::prelude::*;
    ///
    /// // Fail fast on a missing variable: an empty credential would only
    /// // surface later as a confusing authentication failure.
    /// fn required_var(name: &str) -> Result<String, AppError> {
    ///     std::env::var(name).map_err(|_| AppError::InvalidInput(format!("{name} is not set")))
    /// }
    ///
    /// # fn main() -> Result<(), AppError> {
    /// let credentials = Credentials::new(
    ///     required_var("MYAPP_IG_USERNAME")?,
    ///     required_var("MYAPP_IG_PASSWORD")?,
    ///     required_var("MYAPP_IG_ACCOUNT_ID")?,
    ///     required_var("MYAPP_IG_API_KEY")?,
    /// );
    /// let client = Client::with_config(Config::from_credentials(credentials))?;
    /// # let _ = client;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    /// * `config` - The configuration the client and its session layer will use.
    ///
    /// # Returns
    /// * `Ok(Client)` - A client ready to use with `config`.
    /// * `Err(AppError)` - If the underlying HTTP client cannot be constructed.
    ///
    /// # Errors
    /// Returns [`AppError::Network`] if the underlying `reqwest` client cannot
    /// be built (e.g. the system TLS backend fails to initialize).
    pub fn with_config(config: Config) -> Result<Self, AppError> {
        let http_client = Arc::new(HttpClient::new_lazy(config)?);
        Ok(Self { http_client })
    }

    /// Returns the configuration this client was built with.
    ///
    /// Useful to confirm which environment the client is pointed at (e.g.
    /// `client.config().rest_api.base_url`). `Config`'s `Debug` / `Display`
    /// redact credentials and the database URL, so rendering it that way is
    /// safe. Its `Serialize` impl does **not** redact — never serialize a
    /// `Config` into logs, telemetry or an error payload.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &Config {
        self.http_client.config()
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
        // Deserialize straight into the typed DTO: the previous
        // `serde_json::Value` -> `from_value` hop allocated the whole JSON tree
        // twice and dropped the epic from any deserialization error.
        let market_details: MarketDetails = self.http_client.get(&path, Some(3)).await?;
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
        num_points: u32,
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
        num_points: u32,
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

        // Move the root response fields out instead of cloning the (potentially
        // large) DTO. The same market epic can appear under multiple navigation
        // nodes, so track seen epics and keep only the first occurrence.
        let mut seen_epics: HashSet<String> = HashSet::new();
        let mut all_markets: Vec<MarketData> = Vec::new();
        for market in root_response.markets {
            if seen_epics.insert(market.epic.clone()) {
                all_markets.push(market);
            }
        }
        let mut nodes_to_process = root_response.nodes;
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

                        // Deduplicate by epic across nodes to avoid storing the
                        // same market many times.
                        for market in node_response.markets {
                            if seen_epics.insert(market.epic.clone()) {
                                all_markets.push(market);
                                level_market_count += 1;
                            }
                        }
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

        // Build `symbol -> (representative epic, fallback expiry)` in ONE pass
        // instead of re-scanning the full entries Vec per unique symbol
        // (previously O(symbols x entries)). The first entry seen for a symbol
        // supplies both the epic to query and the fallback expiry, matching the
        // previous `find`-first behaviour.
        let mut symbol_info: std::collections::HashMap<String, (String, String)> =
            std::collections::HashMap::new();
        for entry in &vec_db_entries {
            if entry.symbol.is_empty() || entry.epic.is_empty() {
                continue;
            }
            symbol_info
                .entry(entry.symbol.clone())
                .or_insert_with(|| (entry.epic.clone(), entry.expiry.clone()));
        }

        info!(
            "Found {} unique symbols to fetch expiry dates for",
            symbol_info.len()
        );

        // Fetch market details with bounded concurrency. The shared `RateLimiter`
        // still paces the underlying requests; `buffer_unordered` just overlaps
        // the network latency instead of issuing one request at a time.
        let symbol_expiry_map: std::collections::HashMap<String, String> =
            futures::stream::iter(symbol_info)
                .map(|(symbol, (epic, fallback_expiry))| async move {
                    match self.get_market_details(&epic).await {
                        Ok(market_details) => {
                            let expiry_date = market_details
                                .instrument
                                .expiry_details
                                .as_ref()
                                .map(|details| details.last_dealing_date.clone())
                                .unwrap_or_else(|| market_details.instrument.expiry.clone());

                            info!(
                                symbol = %symbol,
                                expiry = %expiry_date,
                                "fetched expiry date for symbol"
                            );
                            (symbol, expiry_date)
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to get market details for epic {} (symbol {}): {:?}",
                                epic,
                                symbol,
                                e
                            );
                            (symbol, fallback_expiry)
                        }
                    }
                })
                .buffer_unordered(MARKET_DETAILS_CONCURRENCY)
                .collect()
                .await;

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
        page_number: Option<u32>,
        page_size: Option<u32>,
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
        // IG requires pageSize (400 without it) and parses from/to as
        // ISO-8601 instants with a zone designator (500 without one).
        const PAGE_SIZE: u32 = 500;
        let from = ensure_zone_designator(from);
        let to = ensure_zone_designator(to);
        let mut all_entries = Vec::new();
        let mut current_page: u32 = 1;
        #[allow(unused_assignments)]
        let mut last_pagination = None;

        loop {
            let path = format!(
                "indicativecostsandcharges/history/from/{}/to/{}?pageSize={}&pageNumber={}",
                from, to, PAGE_SIZE, current_page
            );
            info!("Getting costs history page {}", current_page);

            let result: CostsHistoryResponse = self.http_client.get(&path, Some(1)).await?;

            let total_pages = result.pagination.total_pages;
            last_pagination = Some(result.pagination);
            all_entries.extend(result.costs_and_charges_history);

            if i64::from(current_page) >= total_pages {
                break;
            }
            current_page += 1;
        }

        debug!("Costs history obtained: {} entries", all_entries.len());

        Ok(CostsHistoryResponse {
            pagination: last_pagination.ok_or_else(|| {
                AppError::InvalidInput("Could not retrieve pagination".to_string())
            })?,
            costs_and_charges_history: all_entries,
        })
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
/// One Lightstreamer session carries every IG channel: market data
/// (`MARKET:`), detailed prices (`PRICE:`, served by the `Pricing` data
/// adapter), trade confirmations (`TRADE:`), account balances (`ACCOUNT:`) and
/// candles (`CHART:`). The data adapter is a property of the *subscription*, so
/// one session is enough — the pair of connections this type used to open was a
/// workaround for the previous client library.
///
/// # Lifecycle
///
/// The session is opened lazily by the first `*_subscribe` call and lives until
/// [`disconnect`](Self::disconnect) or `Drop`. [`connect`](Self::connect) does
/// not open it; it consumes the session event stream and blocks until the
/// shutdown signal fires or the session ends for good, which is what makes it
/// usable as the "run until stopped" body of a streaming binary.
///
/// # Channels
///
/// Each `*_subscribe` returns an unbounded receiver of decoded DTOs. The sender
/// is owned by a converter task spawned per subscription; when the caller drops
/// the receiver that task logs and exits, and when the session ends the
/// subscription stream closes and the task exits. Every one of those tasks is
/// tracked and joined by [`disconnect`](Self::disconnect), so none outlives the
/// client.
#[cfg(feature = "streaming")]
#[cfg_attr(docsrs, doc(cfg(feature = "streaming")))]
pub struct StreamerClient {
    account_id: String,
    /// The validated session configuration, used to open the session on the
    /// first subscription. It carries the Lightstreamer password (the IG
    /// session token), so this type deliberately has no `Debug` impl of its
    /// own; the upstream `Credentials` redacts the password in its own.
    config: ClientConfig,
    /// The live session, `None` until the first subscription and again after
    /// `disconnect`. `Client::subscribe` takes `&self`, so no lock is needed.
    client: Option<LsClient>,
    /// The session event stream, taken by `connect`.
    session_events: Option<SessionEvents>,
    /// Shutdown signal for the converter tasks. `watch` rather than `Notify`:
    /// it is level-triggered, so a task busy converting an update when the
    /// signal fires still observes it.
    shutdown_tx: watch::Sender<bool>,
    /// Handles for the per-subscription update -> DTO converter tasks. Each
    /// `*_subscribe` call spawns one; `disconnect` signals and joins them so
    /// they do not idle for the process lifetime.
    converter_tasks: Vec<JoinHandle<()>>,
    // Flags indicating whether there is at least one active subscription of
    // each kind, so `connect` can report what it is actually waiting on.
    has_market_stream_subs: bool,
    has_price_stream_subs: bool,
}

#[cfg(feature = "streaming")]
impl StreamerClient {
    /// Creates a new streaming client instance with its own REST session.
    ///
    /// This builds a fresh [`Client`] and logs in to obtain the Lightstreamer
    /// endpoint and credentials. No connection is established yet — the session
    /// opens on the first subscription.
    ///
    /// When the caller already holds a [`Client`] with an active REST session,
    /// prefer [`with_client`](Self::with_client) to reuse that session instead
    /// of performing a second login.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] if the login / session lookup fails, or
    /// [`AppError::InvalidInput`] if IG returned an endpoint the Lightstreamer
    /// client rejects.
    pub async fn new() -> Result<Self, AppError> {
        let client = Client::try_new()?;
        Self::with_client(&client).await
    }

    /// Creates a new streaming client that reuses the caller's existing REST
    /// session.
    ///
    /// Unlike [`new`](Self::new), this does not build a second HTTP client or
    /// perform a second login: it reuses `client`'s cached session (via
    /// [`Client::ws_info`]) to obtain the Lightstreamer endpoint and
    /// credentials.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] if the session lookup fails, or
    /// [`AppError::InvalidInput`] if IG returned an endpoint the Lightstreamer
    /// client rejects.
    pub async fn with_client(client: &Client) -> Result<Self, AppError> {
        let ws_info = client.ws_info().await?;

        // The Lightstreamer password IS the IG session token pair
        // (`CST-…|XST-…`). It goes into `Credentials`, whose `Debug` redacts
        // it, and is never logged or echoed anywhere on this path.
        let config = ClientConfig::builder(ServerAddress::try_new(ws_info.server.as_str())?)
            .with_credentials(Credentials::new(
                ws_info.account_id.as_str(),
                ws_info.get_ws_password(),
            ))
            .build()?;

        let (shutdown_tx, _) = watch::channel(false);

        Ok(Self {
            account_id: ws_info.account_id.clone(),
            config,
            client: None,
            session_events: None,
            shutdown_tx,
            converter_tasks: Vec::new(),
            has_market_stream_subs: false,
            has_price_stream_subs: false,
        })
    }

    /// Opens the Lightstreamer session if it is not open yet, and returns it.
    ///
    /// Called by every `*_subscribe`: the session cannot be opened in the
    /// constructor because `lightstreamer-rs` connects eagerly, and connecting
    /// before there is anything to subscribe to would open a socket that is
    /// only ever closed again.
    ///
    /// The configuration is kept rather than consumed, so subscribing again
    /// after [`disconnect`](Self::disconnect) opens a fresh session with the
    /// same endpoint and credentials.
    async fn ensure_session(&mut self) -> Result<&LsClient, AppError> {
        if self.client.is_none() {
            let (client, events) = LsClient::connect(self.config.clone()).await?;
            info!(account_id = %self.account_id, "Lightstreamer session opened");
            self.client = Some(client);
            self.session_events = Some(events);
        }

        self.client.as_ref().ok_or_else(|| {
            AppError::WebSocketError("streaming session not initialized".to_string())
        })
    }

    /// Subscribes and spawns the converter task that turns the subscription's
    /// event stream into a channel of decoded DTOs.
    ///
    /// The converter owns the `Updates` stream, so dropping it (when the task
    /// ends) unsubscribes. It stops on the shutdown signal, on the receiver
    /// being dropped, or on the stream closing — never on a decode failure,
    /// which the `From` impls degrade to a default.
    async fn subscribe_and_convert<T, C>(
        &mut self,
        subscription: Subscription,
        label: &str,
        convert: C,
    ) -> Result<mpsc::UnboundedReceiver<T>, AppError>
    where
        T: Send + 'static,
        C: Fn(&StreamingUpdate) -> T + Send + 'static,
    {
        let updates = self.ensure_session().await?.subscribe(subscription).await?;

        let (tx, rx) = mpsc::unbounded_channel();
        let mut shutdown = self.shutdown_tx.subscribe();
        let label = label.to_owned();

        let handle = tokio::spawn(async move {
            let mut updates = updates;
            loop {
                let event = tokio::select! {
                    _ = shutdown.changed() => {
                        debug!(subscription = %label, "converter stopped by shutdown signal");
                        return;
                    }
                    event = updates.next() => event,
                };

                let Some(event) = event else {
                    debug!(subscription = %label, "converter stopped: subscription stream closed");
                    return;
                };

                match event {
                    SubscriptionEvent::Update(update) => {
                        let data = convert(&StreamingUpdate::from(update.as_ref()));
                        if tx.send(data).is_err() {
                            debug!(subscription = %label, "converter stopped: receiver dropped");
                            return;
                        }
                    }
                    SubscriptionEvent::Activated {
                        item_count,
                        field_count,
                        ..
                    } => info!(
                        subscription = %label,
                        item_count,
                        field_count,
                        "subscription started"
                    ),
                    // Terminal for this subscription. The server's own code and
                    // message; never a credential.
                    SubscriptionEvent::Rejected(e) => {
                        error!(subscription = %label, error = %e, "IG refused the subscription");
                        return;
                    }
                    SubscriptionEvent::Unsubscribed => {
                        info!(subscription = %label, "subscription ended");
                        return;
                    }
                    SubscriptionEvent::Overflow {
                        item_index,
                        dropped_count,
                    } => warn!(
                        subscription = %label,
                        item_index,
                        dropped_count,
                        "IG dropped updates for this item"
                    ),
                    other => debug!(subscription = %label, event = ?other, "subscription event"),
                }
            }
        });
        self.converter_tasks.push(handle);

        Ok(rx)
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
    /// # Errors
    ///
    /// Returns [`AppError::InvalidInput`] if `epics` or `fields` is empty or
    /// contains a name Lightstreamer rejects, and [`AppError::WebSocketError`]
    /// if the session cannot be opened or the subscription cannot be sent.
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
        let epic_count = epics.len();
        let items: Vec<String> = epics
            .into_iter()
            .map(|epic| format!("MARKET:{epic}"))
            .collect();
        let subscription = Subscription::new(
            SubscriptionMode::Merge,
            ItemGroup::from_items(items)?,
            FieldSchema::from_fields(get_streaming_market_fields(&fields))?,
        )
        .with_snapshot(Snapshot::On);

        let receiver = self
            .subscribe_and_convert(subscription, "market", |update| PriceData::from(update))
            .await?;
        self.has_market_stream_subs = true;

        info!("Market subscription created for {epic_count} instruments");
        Ok(receiver)
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
    /// # Errors
    ///
    /// Returns [`AppError::WebSocketError`] if the session cannot be opened or
    /// the subscription cannot be sent.
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
        let account_id = self.account_id.clone();
        let subscription = Subscription::new(
            SubscriptionMode::Distinct,
            ItemGroup::from_items([format!("TRADE:{account_id}")])?,
            FieldSchema::from_fields(["CONFIRMS", "OPU", "WOU"])?,
        )
        .with_snapshot(Snapshot::On);

        let receiver = self
            .subscribe_and_convert(subscription, "trade", |update| {
                crate::presentation::trade::TradeData::from(update).fields
            })
            .await?;
        self.has_market_stream_subs = true;

        info!(account_id = %account_id, "Trade subscription created");
        Ok(receiver)
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
    /// # Errors
    ///
    /// Returns [`AppError::InvalidInput`] if `fields` is empty or contains a
    /// name Lightstreamer rejects, and [`AppError::WebSocketError`] if the
    /// session cannot be opened or the subscription cannot be sent.
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
        let account_id = self.account_id.clone();
        let subscription = Subscription::new(
            SubscriptionMode::Merge,
            ItemGroup::from_items([format!("ACCOUNT:{account_id}")])?,
            FieldSchema::from_fields(get_streaming_account_data_fields(&fields))?,
        )
        .with_snapshot(Snapshot::On);

        let receiver = self
            .subscribe_and_convert(subscription, "account", |update| {
                crate::presentation::account::AccountData::from(update).fields
            })
            .await?;
        self.has_market_stream_subs = true;

        info!(account_id = %account_id, "Account subscription created");
        Ok(receiver)
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
    /// # Errors
    ///
    /// Returns [`AppError::InvalidInput`] if `epics` or `fields` is empty or
    /// contains a name Lightstreamer rejects, and [`AppError::WebSocketError`]
    /// if the session cannot be opened or the subscription cannot be sent.
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
        let account_id = self.account_id.clone();
        let epic_count = epics.len();
        let items: Vec<String> = epics
            .into_iter()
            .map(|epic| format!("PRICE:{account_id}:{epic}"))
            .collect();
        let field_names = get_streaming_price_fields(&fields);

        debug!(?items, ?field_names, "Pricing subscription shape");

        // The `Pricing` data adapter name is a server-side configuration
        // detail; it is overridable so a differently-configured IG environment
        // does not need a code change.
        let pricing_adapter =
            std::env::var("IG_PRICING_ADAPTER").unwrap_or_else(|_| "Pricing".to_string());
        debug!(adapter = %pricing_adapter, "Using Pricing data adapter");

        let subscription = Subscription::new(
            SubscriptionMode::Merge,
            ItemGroup::from_items(items)?,
            FieldSchema::from_fields(field_names)?,
        )
        .with_data_adapter(pricing_adapter)
        .with_snapshot(Snapshot::On);

        let receiver = self
            .subscribe_and_convert(subscription, "price", |update| PriceData::from(update))
            .await?;
        self.has_price_stream_subs = true;

        info!(account_id = %account_id, "Price subscription created for {epic_count} instruments");
        Ok(receiver)
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
    /// # Errors
    ///
    /// Returns [`AppError::InvalidInput`] if `epics` or `fields` is empty or
    /// contains a name Lightstreamer rejects, and [`AppError::WebSocketError`]
    /// if the session cannot be opened or the subscription cannot be sent.
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
        let epic_count = epics.len();
        let items: Vec<String> = epics
            .into_iter()
            .map(|epic| format!("CHART:{epic}:{scale}"))
            .collect();

        // Candle data is a running value (MERGE); tick data is a sequence of
        // independent events (DISTINCT).
        let mode = if matches!(scale, ChartScale::Tick) {
            SubscriptionMode::Distinct
        } else {
            SubscriptionMode::Merge
        };

        let subscription = Subscription::new(
            mode,
            ItemGroup::from_items(items)?,
            FieldSchema::from_fields(get_streaming_chart_fields(&fields))?,
        )
        .with_snapshot(Snapshot::On);

        let receiver = self
            .subscribe_and_convert(subscription, "chart", |update| ChartData::from(update))
            .await?;
        self.has_market_stream_subs = true;

        info!("Chart subscription created for {epic_count} instruments (scale: {scale})");
        Ok(receiver)
    }

    /// Consumes the session event stream and blocks until shutdown.
    ///
    /// The Lightstreamer session is already open by the time this is called
    /// (the first subscription opened it) and reconnection is handled by
    /// `lightstreamer-rs` itself, with bounded jittered backoff. What this
    /// method adds is observation: it reports what every reconnection *meant*
    /// — in particular a session that was replaced rather than preserved, after
    /// which every subscription has been re-executed and a fresh snapshot is on
    /// its way — and it returns when the session ends for good.
    ///
    /// # Arguments
    ///
    /// * `shutdown_signal` - Signalled by the caller to stop. When `None`, this
    ///   waits for `SIGINT` / `SIGTERM` instead.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the shutdown signal fired or the session was closed by
    /// this client.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::WebSocketError`] when the session ended for a reason
    /// this client did not ask for: refused by IG, reconnection budget
    /// exhausted, or an internal failure in the streaming crate.
    pub async fn connect(&mut self, shutdown_signal: Option<Arc<Notify>>) -> Result<(), AppError> {
        let Some(mut events) = self.session_events.take() else {
            // Either nothing was subscribed (so no session was ever opened) or
            // the events were already consumed by an earlier `connect`.
            warn!("No streaming session to run: subscribe first, and call connect once");
            return Ok(());
        };

        info!(
            market_subscriptions = self.has_market_stream_subs,
            price_subscriptions = self.has_price_stream_subs,
            "Streaming session running"
        );

        // Built once and polled across every iteration. Re-creating it inside
        // the loop would re-register the signal handlers on every event and
        // could drop a signal that arrived between two of them.
        let shutdown = wait_for_shutdown(shutdown_signal);
        tokio::pin!(shutdown);

        loop {
            let event = tokio::select! {
                () = &mut shutdown => {
                    info!("Streaming session stopping: shutdown requested");
                    return Ok(());
                }
                event = events.next() => event,
            };

            let Some(event) = event else {
                // The stream ended without a `Closed` event, which only happens
                // if the client was dropped underneath us.
                debug!("Session event stream ended");
                return Ok(());
            };

            match event {
                SessionEvent::Connected(connected) => match connected.continuity {
                    // Only a *replaced* session invalidates derived state: it
                    // re-executes every subscription, so anything computed from
                    // the previous one is stale. New / Preserved / Recovered all
                    // keep it — a first connect is not a replacement to warn
                    // about, which the old is_preserved() split got wrong.
                    Continuity::Replaced { .. } => warn!(
                        continuity = ?connected.continuity,
                        "Streaming session replaced: subscriptions re-executed, expect fresh snapshots"
                    ),
                    _ => info!(
                        continuity = ?connected.continuity,
                        "Streaming session connected"
                    ),
                },
                SessionEvent::Resubscribed(subscriptions) => {
                    info!(
                        count = subscriptions.len(),
                        "Subscriptions re-created on a new session"
                    );
                }
                SessionEvent::Disconnected { reason, retry_in } => match retry_in {
                    Some(delay) => warn!(
                        ?reason,
                        retry_in_ms = delay.as_millis(),
                        "Streaming session disconnected, reconnecting"
                    ),
                    None => warn!(?reason, "Streaming session disconnected, giving up"),
                },
                SessionEvent::Closed(reason) => return Self::report_close(&reason),
                SessionEvent::RequestRejected(e) => {
                    warn!(error = %e, "IG refused a streaming control request");
                }
                SessionEvent::RequestNotSent { reason } => {
                    warn!(%reason, "A streaming control request never left the client");
                }
                // The raw line can carry market data, so it stays at TRACE.
                SessionEvent::Unrecognized { line } => {
                    trace!(%line, "Unrecognized streaming notification");
                }
                other => debug!(event = ?other, "Session event"),
            }
        }
    }

    /// Turns a terminal [`ClosedReason`] into this crate's result.
    ///
    /// A close this client asked for is success. Everything else is a failure
    /// carrying IG's own reason — there is no message-sniffing here: 1.0 has a
    /// discriminant for a clean shutdown and this is it.
    fn report_close(reason: &ClosedReason) -> Result<(), AppError> {
        match reason {
            ClosedReason::ByClient => {
                info!("Streaming session closed by this client");
                Ok(())
            }
            ClosedReason::ByServer(e) => {
                error!(error = %e, "IG closed the streaming session");
                Err(AppError::WebSocketError(format!(
                    "IG closed the streaming session: {e}"
                )))
            }
            ClosedReason::ReconnectExhausted { attempts, last } => {
                error!(attempts, last_reason = ?last, "Streaming reconnection budget exhausted");
                Err(AppError::WebSocketError(format!(
                    "streaming reconnection budget exhausted after {attempts} attempts"
                )))
            }
            ClosedReason::Internal { reason } => {
                error!(%reason, "Streaming client failed internally");
                Err(AppError::WebSocketError(format!(
                    "streaming client failed internally: {reason}"
                )))
            }
            other => {
                error!(reason = ?other, "Streaming session closed");
                Err(AppError::WebSocketError(format!(
                    "streaming session closed: {other:?}"
                )))
            }
        }
    }

    /// Disconnects the Lightstreamer session and tears down every converter
    /// task.
    ///
    /// The order matters: the converters are signalled and joined first, which
    /// drops their subscription streams and so unsubscribes, and only then is
    /// the session closed. Calling this more than once is safe — the task list
    /// is drained and the session handle is taken.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::WebSocketError`] if closing the session failed. The
    /// converter tasks are stopped either way.
    pub async fn disconnect(&mut self) -> Result<(), AppError> {
        // Ignore the send error: it only means every converter has already
        // exited, which is precisely the state we are asking for.
        let _ = self.shutdown_tx.send(true);

        let converter_count = self.converter_tasks.len();
        join_tasks(&mut self.converter_tasks).await;
        if converter_count > 0 {
            debug!("Stopped {converter_count} converter task(s)");
        }

        self.session_events = None;

        if let Some(client) = self.client.take() {
            client.disconnect().await?;
            info!("Streaming session closed");
        }

        Ok(())
    }
}

/// Waits for the caller's shutdown signal, or for `SIGINT` / `SIGTERM` when
/// there is none.
///
/// `lightstreamer-rs` 1.0 deliberately does not install signal handlers — that
/// is not a protocol client's job — so the wait lives here.
#[cfg(feature = "streaming")]
async fn wait_for_shutdown(signal: Option<Arc<Notify>>) {
    if let Some(signal) = signal {
        signal.notified().await;
        return;
    }

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        // A handler that cannot be installed must not silently disable
        // shutdown, so fall back to waiting forever only after saying so.
        match (
            signal(SignalKind::interrupt()),
            signal(SignalKind::terminate()),
        ) {
            (Ok(mut sigint), Ok(mut sigterm)) => {
                tokio::select! {
                    _ = sigint.recv() => info!("SIGINT received"),
                    _ = sigterm.recv() => info!("SIGTERM received"),
                }
            }
            (sigint, sigterm) => {
                if let Err(e) = sigint {
                    error!(error = %e, "cannot install the SIGINT handler");
                }
                if let Err(e) = sigterm {
                    error!(error = %e, "cannot install the SIGTERM handler");
                }
                std::future::pending::<()>().await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!(error = %e, "cannot wait for Ctrl-C");
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(feature = "streaming")]
impl Drop for StreamerClient {
    /// Signals and then abandons any converter task that
    /// [`StreamerClient::disconnect`] did not already join, so dropping the
    /// client never leaves one running. Dropping the session handle closes the
    /// Lightstreamer session; neither step can await, which is why
    /// `disconnect` is still the way to observe the close completing.
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
        for handle in self.converter_tasks.drain(..) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ensure_zone_designator, is_transient_confirmation_error};
    use crate::error::AppError;
    use reqwest::StatusCode;

    #[test]
    fn test_ensure_zone_designator_appends_z_when_missing() {
        assert_eq!(
            ensure_zone_designator("2026-01-01T00:00:00"),
            "2026-01-01T00:00:00Z"
        );
        assert_eq!(
            ensure_zone_designator(" 2026-01-01T00:00:00 "),
            "2026-01-01T00:00:00Z"
        );
    }

    #[test]
    fn test_ensure_zone_designator_expands_date_only_to_midnight_utc() {
        assert_eq!(ensure_zone_designator("2026-01-01"), "2026-01-01T00:00:00Z");
    }

    #[test]
    fn test_ensure_zone_designator_keeps_existing_designator() {
        assert_eq!(
            ensure_zone_designator("2026-01-01T00:00:00Z"),
            "2026-01-01T00:00:00Z"
        );
        assert_eq!(
            ensure_zone_designator("2026-01-01T00:00:00+01:00"),
            "2026-01-01T00:00:00+01:00"
        );
        assert_eq!(
            ensure_zone_designator("2026-01-01T00:00:00-05:00"),
            "2026-01-01T00:00:00-05:00"
        );
    }

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

// The streaming helpers these tests exercise only exist with the `streaming`
// feature, so they live in their own gated module rather than behind per-test
// attributes.
#[cfg(all(test, feature = "streaming"))]
mod streaming_tests {
    use super::{ClosedReason, StreamerClient, join_tasks};
    use crate::error::AppError;
    use lightstreamer_rs::ServerError;
    use std::time::Duration;
    use tokio::sync::watch;
    use tokio::task::JoinHandle;

    // --- Terminal close classification -------------------------------------
    //
    // These replace the previous `is_graceful_close` tests, which matched a
    // marker string inside an error message because the 0.3 error type had no
    // graceful-close discriminant. 1.0 has one.

    #[test]
    fn test_close_by_client_is_success() {
        let result = StreamerClient::report_close(&ClosedReason::ByClient);
        assert!(
            result.is_ok(),
            "a close this client asked for is not a failure: {result:?}"
        );
    }

    #[test]
    fn test_close_by_server_is_an_error() {
        // IG's Metadata Adapter refuses with a code below the protocol's range.
        let reason = ClosedReason::ByServer(ServerError::new(-1, "Insufficient permissions"));
        let result = StreamerClient::report_close(&reason);
        assert!(
            matches!(result, Err(AppError::WebSocketError(_))),
            "a server-initiated close must surface as an error: {result:?}"
        );
    }

    #[test]
    fn test_close_after_exhausted_reconnection_is_an_error() {
        let result = StreamerClient::report_close(&ClosedReason::ReconnectExhausted {
            attempts: 8,
            last: None,
        });
        match result {
            Err(AppError::WebSocketError(message)) => {
                assert!(
                    message.contains('8'),
                    "the attempt count belongs in the message: {message}"
                );
            }
            other => panic!("expected a websocket error, got {other:?}"),
        }
    }

    #[test]
    fn test_close_on_internal_failure_is_an_error() {
        let reason = ClosedReason::Internal {
            reason: "bug".to_string(),
        };
        assert!(matches!(
            StreamerClient::report_close(&reason),
            Err(AppError::WebSocketError(_))
        ));
    }

    // --- Converter shutdown ------------------------------------------------

    #[tokio::test]
    async fn test_watch_signal_stops_every_converter() {
        // One `watch` sender stands in for `StreamerClient::shutdown_tx`, and
        // three tasks for the converters. Unlike `Notify::notify_one`, a
        // `watch` send reaches every one of them, and unlike
        // `Notify::notify_waiters` it is level-triggered, so a task that is not
        // parked yet still observes it.
        let (tx, _) = watch::channel(false);
        let mut tasks: Vec<JoinHandle<()>> = Vec::new();
        for _ in 0..3 {
            let mut shutdown = tx.subscribe();
            tasks.push(tokio::spawn(async move {
                tokio::select! {
                    _ = shutdown.changed() => {}
                    () = std::future::pending::<()>() => {}
                }
            }));
        }

        // Sent before any task is necessarily parked: the signal must not be
        // missed.
        assert!(tx.send(true).is_ok());

        let joined = tokio::time::timeout(Duration::from_secs(1), join_tasks(&mut tasks)).await;
        assert!(joined.is_ok(), "converters did not observe the shutdown");
        assert!(tasks.is_empty(), "join_tasks must drain the list");
    }

    #[tokio::test]
    async fn test_join_tasks_waits_for_completion() {
        let mut tasks: Vec<JoinHandle<()>> = vec![tokio::spawn(async {})];
        join_tasks(&mut tasks).await;
        assert!(tasks.is_empty());
    }
}
