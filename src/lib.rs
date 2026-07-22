//! # IG Markets API Client for Rust
//!
//! A comprehensive Rust client for the IG Markets trading API. This library
//! provides a type-safe, async-first way to access IG Markets' REST and
//! real-time streaming APIs for trading and market-data retrieval.
//!
//! ## Overview
//!
//! The IG Markets API Client for Rust offers a reliable interface to the IG
//! Markets trading platform. It handles authentication and session management,
//! automatic token refresh, rate limiting, finite retry with backoff, and
//! real-time streaming over the Lightstreamer protocol, exposing a clean,
//! idiomatic Rust API.
//!
//! ## Features
//!
//! - **Authentication**: IG session v2 (`CST` / `X-SECURITY-TOKEN` headers) and
//!   v3 (OAuth bearer) with automatic, transparent token refresh and account
//!   switching.
//! - **Account Management**: Accounts, balances, positions, working orders,
//!   preferences, activity, and transaction history.
//! - **Market Data**: Market search, instrument details, market navigation, and
//!   historical prices at several resolutions.
//! - **Order Management**: Create, update, and close positions and working
//!   orders with typed request builders.
//! - **Watchlists**: Full CRUD over watchlists and their instruments.
//! - **Client Sentiment**: Sentiment for single, multiple, and related markets.
//! - **Indicative Costs**: Costs and charges for opening, closing, or editing
//!   positions, plus cost history.
//! - **Real-time Streaming** (feature `streaming`, on by default): Market,
//!   price, trade, and account updates over Lightstreamer, with thread-safe
//!   dynamic subscription management.
//! - **Rate Limiting**: `governor`-backed pacing configured per IG's trading vs
//!   non-trading budgets.
//! - **Finite Retry**: Exponential backoff with jitter via `RetryConfig`
//!   (bounded — no unbounded retry loops).
//! - **Type Safety**: Strongly typed request / response DTOs and domain enums.
//! - **Async**: Built on `tokio` with a shared, pooled `reqwest` client.
//! - **Persistence** (feature `persistence`, on by default): PostgreSQL storage
//!   via `sqlx`.
//!
//! ## Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! ig-client = "0.12.3"
//! tokio = { version = "1", features = ["full"] }  # Async runtime
//! tracing = "0.1"                                  # Logging facade
//! # Optional, only if you use the PostgreSQL persistence layer:
//! sqlx = { version = "0.9", features = ["runtime-tokio", "tls-native-tls", "postgres"] }
//! ```
//!
//! You do not need to add `dotenv` yourself — `Config::new()` loads a local
//! `.env` file internally. If your application supplies its own configuration
//! and must not read a `.env` file or the `IG_*` namespace, use
//! `Config::from_credentials()` with `Client::with_config()` instead (see
//! [Programmatic configuration](#programmatic-configuration-embedders)).
//!
//! ### Cargo features
//!
//! | Feature | Default | Pulls in | Gives you |
//! |---|---|---|---|
//! | `streaming` | on | `lightstreamer-rs` | `StreamerClient`, `DynamicMarketStreamer`, `StreamingUpdate` and its DTO adapters, `application::interfaces::listener` |
//! | `persistence` | on | `sqlx` | the `storage` module: `MarketDatabaseService`, historical prices, connection pooling |
//!
//! Both are on by default, so the crate behaves exactly as before unless you
//! opt out. Turn them off for a REST/poll-only client:
//!
//! ```toml
//! [dependencies]
//! ig-client = { version = "0.12.3", default-features = false }
//! ```
//!
//! That leaves `Client`, `Client::with_config`, every REST service trait
//! (`MarketService`, `AccountService`, `OrderService`, …), the DTOs and the
//! rate limiter fully available, with neither `lightstreamer-rs` nor `sqlx` in
//! the dependency graph. Two reasons to care:
//!
//! - **Dependency surface**: `lightstreamer-rs` brings a WebSocket stack that
//!   a REST-only integration never opens. (It was GPL-3.0-only up to 0.3.x;
//!   1.0 is a clean-room rewrite under MIT, so the licence reason for turning
//!   `streaming` off has gone away.)
//! - **Build weight**: `sqlx` brings a full PostgreSQL driver that a
//!   market-data-only integration never uses.
//!
//! Turning `streaming` off removes the streaming client types; the
//! `Streaming*Field` selectors and the presentation DTOs are plain data types
//! and stay available either way. Turning `persistence` off removes everything
//! under `storage` except `storage::config` (a re-export of the sqlx-free
//! `application::config::DatabaseConfig`), along with the `AppError::Db`
//! variant.
//!
//! One side effect worth knowing: `sqlx` enables `tracing`'s `log` feature, so
//! with `persistence` on this crate's events are also emitted as `log` records.
//! With it off they are tracing events only, which matters if your application
//! collects logs through the `log` facade.
//!
//! ### Requirements
//!
//! - Rust 2024 edition on the stable toolchain.
//! - An IG Markets account (demo or live) and API credentials.
//! - A PostgreSQL database (optional, only for the persistence layer).
//!
//! ## Configuration
//!
//! `Config::new()` reads configuration from the environment (and a local `.env`
//! file, if present). Create a `.env` file in your project root with the
//! following variables:
//!
//! ```text
//! IG_USERNAME=your_username
//! IG_PASSWORD=your_password
//! IG_API_KEY=your_api_key
//! IG_ACCOUNT_ID=your_account_id
//! IG_API_VERSION=3                                   # 2 (CST/XST) or 3 (OAuth); defaults to 3
//! IG_REST_BASE_URL=https://demo-api.ig.com/gateway/deal   # Use demo or live as needed
//! IG_REST_TIMEOUT=30                                 # REST request timeout in seconds
//! IG_WS_URL=wss://demo-apd.marketdatasystems.com     # Lightstreamer endpoint
//! IG_WS_RECONNECT_INTERVAL=5                         # Reconnect interval in seconds
//! IG_RATE_LIMIT_MAX_REQUESTS=4                       # Rate-limiter budget
//! IG_RATE_LIMIT_PERIOD_SECONDS=12                    # Rate-limiter period (seconds)
//! IG_RATE_LIMIT_BURST_SIZE=3                         # Rate-limiter burst size
//! DATABASE_URL=postgres://user:password@localhost/ig_db   # Optional persistence
//! DATABASE_MAX_CONNECTIONS=5                         # Optional connection-pool size
//! TX_LOOP_INTERVAL_HOURS=1                           # Transaction loop interval (hours)
//! TX_PAGE_SIZE=20                                    # Transaction page size
//! TX_DAYS_LOOKBACK=7                                 # Days to look back for transactions
//! ```
//!
//! Live examples and integration tests default to the IG **demo** environment;
//! pointing anything at production requires an explicit opt-in via
//! `IG_REST_BASE_URL` / `IG_WS_URL`.
//!
//! ### Programmatic configuration (embedders)
//!
//! There are two configuration paths and they do not mix:
//!
//! - `Config::new()` / `Client::try_new()` — the **convenience path**. Loads a
//!   local `.env` file and reads the global `IG_*` namespace, as above.
//! - `Config::from_credentials()` / `Client::with_config()` — the **injection
//!   path**. Reads no environment variable and loads no `.env` file, for an
//!   embedding application that owns its configuration source (its own
//!   namespaced variables, a config file, a secrets manager). Useful under
//!   `#![forbid(unsafe_code)]`, where pre-populating `IG_*` is not an option
//!   because `std::env::set_var` is `unsafe` on edition 2024.
//!
//! ```rust,no_run
//! use ig_client::prelude::*;
//!
//! // Fail fast on a missing variable: an empty credential would only surface
//! // later as a confusing authentication failure. Never log the value.
//! fn required_var(name: &str) -> Result<String, AppError> {
//!     std::env::var(name).map_err(|_| AppError::InvalidInput(format!("{name} is not set")))
//! }
//!
//! # fn main() -> Result<(), AppError> {
//! let credentials = Credentials::new(
//!     required_var("MYAPP_IG_USERNAME")?,
//!     required_var("MYAPP_IG_PASSWORD")?,
//!     required_var("MYAPP_IG_ACCOUNT_ID")?,
//!     required_var("MYAPP_IG_API_KEY")?,
//! );
//!
//! // Struct update overrides individual sections and stays env-free,
//! // because the base value is the env-free constructor.
//! let config = Config {
//!     rest_api: RestApiConfig {
//!         base_url: "https://demo-api.ig.com/gateway/deal".to_string(),
//!         timeout: 30,
//!     },
//!     ..Config::from_credentials(credentials)
//! };
//!
//! let client = Client::with_config(config)?;
//! println!("REST base URL: {}", client.config().rest_api.base_url);
//! # Ok(())
//! # }
//! ```
//!
//! Non-credential fields default to the IG **demo** endpoints; see
//! `examples/simples/src/bin/client_with_config.rs` for a runnable version.
//!
//! For streaming (feature `streaming`), build the streamer from the injected
//! client with `StreamerClient::with_client(&client)` — `StreamerClient::new()`
//! goes through `Client::try_new()` and therefore back to `.env` / `IG_*`.
//!
//! Two knobs are still resolved from the process environment on the injected
//! path, because they are not part of `Config`:
//!
//! - Retry policy — `MAX_RETRY_COUNT` and `RETRY_DELAY_SECS` are read per
//!   request via `RetryConfig::default()`; unset means the crate defaults.
//! - `IG_PRICING_ADAPTER` — the Lightstreamer price adapter name, defaulting
//!   to `Pricing` when unset.
//!
//! Neither carries a credential, and both have safe defaults, so an embedder
//! that sets neither variable is unaffected. Moving them into `Config` is a
//! breaking change scheduled for a future minor release.
//!
//! ## Usage
//!
//! The main entry points are `Config` and `Client`. A single `Client`
//! implements every REST service trait (`AccountService`, `MarketService`,
//! `OrderService`, `WatchlistService`, `SentimentService`, `CostsService`,
//! `OperationsService`). Session login and token refresh are performed
//! transparently on first use. The prelude re-exports the full public surface,
//! including the streaming API and the service traits, so a single glob import
//! is usually enough:
//!
//! ```rust
//! use ig_client::prelude::*;
//! ```
//!
//! ### Client setup and account information
//!
//! ```rust,no_run
//! use ig_client::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), AppError> {
//!     // Configuration is read from the environment / a local `.env` file.
//!     let config = Config::new();
//!     println!("configured API version: {:?}", config.api_version);
//!
//!     // `try_new` is the fallible constructor for the REST client.
//!     // (`new()` / `default()` no longer exist.) Session login and automatic
//!     // token refresh happen transparently on the first API call.
//!     let client = Client::try_new()?;
//!
//!     // `Client` implements `AccountService`.
//!     let accounts = client.get_accounts().await?;
//!     println!("{} account(s) available", accounts.accounts.len());
//!     Ok(())
//! }
//! ```
//!
//! ### Market data
//!
//! ```rust,no_run
//! use ig_client::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), AppError> {
//!     let client = Client::try_new()?;
//!
//!     // `Client` implements `MarketService`.
//!     let results = client.search_markets("EUR/USD").await?;
//!     if let Some(market) = results.markets.first() {
//!         let details = client.get_market_details(&market.epic).await?;
//!         println!("{}: {}", market.epic, details.instrument.name);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ### Placing an order
//!
//! ```rust,no_run
//! use ig_client::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), AppError> {
//!     let client = Client::try_new()?;
//!
//!     // Build a market order with the typed constructor. `size` is rounded to
//!     // two decimals; a `None` currency code defaults to "EUR".
//!     let order = CreateOrderRequest::market(
//!         "CS.D.EURUSD.CFD.IP".to_string(), // epic
//!         Direction::Buy,                   // direction
//!         1.0,                              // size
//!         Some("USD".to_string()),          // currency_code
//!         None,                             // deal_reference
//!     );
//!
//!     // `Client` implements `OrderService`.
//!     let confirmation = client.create_order(&order).await?;
//!     println!("deal reference: {}", confirmation.deal_reference);
//!     Ok(())
//! }
//! ```
//!
//! ### Real-time streaming
//!
//! *Requires the default `streaming` feature.*
//!
//! `DynamicMarketStreamer` wraps the lower-level `StreamerClient` and manages
//! subscriptions in a thread-safe way. Its constructor is **synchronous** — it
//! only wires up in-memory channels; the network connection is established later
//! by `start`. Updates arrive as `PriceData`, resolved through the prelude.
//!
//! ```rust,no_run
//! use ig_client::prelude::*;
//! use std::collections::HashSet;
//!
//! # #[cfg(feature = "streaming")]
//! #[tokio::main]
//! async fn main() -> Result<(), AppError> {
//!     // Fields to receive on each market tick.
//!     let fields = HashSet::from([StreamingMarketField::Bid, StreamingMarketField::Offer]);
//!
//!     // Synchronous construction — no `await`, no fallibility.
//!     let mut streamer = DynamicMarketStreamer::new(fields);
//!
//!     // Take the receiver before connecting.
//!     let mut receiver = streamer.get_receiver().await?;
//!
//!     // Subscribe to an instrument and open the Lightstreamer connection.
//!     streamer.add("IX.D.DAX.DAILY.IP".to_string()).await?;
//!     streamer.start().await?;
//!
//!     // Consume `PriceData` updates.
//!     while let Some(price) = receiver.recv().await {
//!         let price: PriceData = price;
//!         println!("price update: {price}");
//!     }
//!     Ok(())
//! }
//! # #[cfg(not(feature = "streaming"))]
//! # fn main() {}
//! ```
//!
//! ## Available Services
//!
//! Every service trait below is implemented by `Client`; bring the traits into
//! scope via `use ig_client::prelude::*;`.
//!
//! ### `AccountService`
//! - `get_accounts()` — all accounts for the authenticated user
//! - `get_positions()` / `get_positions_w_filter(filter)` — open positions
//! - `get_working_orders()` — working orders
//! - `get_activity(from, to)` / `get_activity_with_details(from, to)` — activity
//! - `get_activity_by_period(period_ms)` — activity for a period in milliseconds
//! - `get_transactions(from, to)` — transaction history (paginated internally)
//! - `get_preferences()` / `update_preferences(trailing_stops_enabled)`
//!
//! ### `MarketService`
//! - `search_markets(term)` — search markets by keyword
//! - `get_market_details(epic)` / `get_multiple_market_details(epics)`
//! - `get_historical_prices(epic, resolution, from, to)` and
//!   `get_historical_prices_by_date_range(epic, resolution, start, end)`
//! - `get_historical_prices_by_count_v1` / `_v2(epic, resolution, num_points)`
//! - `get_recent_prices(params)`
//! - `get_market_navigation()` / `get_market_navigation_node(node_id)`
//! - `get_all_markets()` / `get_vec_db_entries()`
//! - `get_categories()` / `get_category_instruments(category_id, page, size)`
//!
//! ### `OrderService`
//! - `create_order(request)` — open a position
//! - `get_order_confirmation(deal_reference)` and
//!   `get_order_confirmation_w_retry(deal_reference, retries, delay_ms)`
//! - `update_position(deal_id, update)` / `update_level_in_position(deal_id, level)`
//! - `close_position(request)` / `get_position(deal_id)`
//! - `create_working_order(request)` / `update_working_order(deal_id, update)` /
//!   `delete_working_order(deal_id)`
//!
//! ### `WatchlistService`
//! - `get_watchlists()` / `create_watchlist(name, epics)`
//! - `get_watchlist(id)` / `delete_watchlist(id)`
//! - `add_to_watchlist(id, epic)` / `remove_from_watchlist(id, epic)`
//!
//! ### `SentimentService`
//! - `get_client_sentiment(market_ids)`
//! - `get_client_sentiment_by_market(market_id)`
//! - `get_related_sentiment(market_id)`
//!
//! ### `CostsService`
//! - `get_indicative_costs_open(request)` / `_close(request)` / `_edit(request)`
//! - `get_costs_history(from, to)` / `get_durable_medium(quote_reference)`
//!
//! ### `OperationsService`
//! - `get_client_apps()` — API application details
//! - `disable_client_app()` — disable the current API key
//!
//! ## Rate Limiting
//!
//! All requests are paced by a `governor`-backed `RateLimiter` so the client
//! stays within IG's published limits (trading and non-trading endpoints have
//! different budgets). The budget is configured from the environment via
//! `IG_RATE_LIMIT_MAX_REQUESTS`, `IG_RATE_LIMIT_PERIOD_SECONDS`, and
//! `IG_RATE_LIMIT_BURST_SIZE` (see `RateLimiterConfig`). On top of pacing,
//! transient failures (429 / 5xx / connection errors) are retried with
//! **finite** exponential backoff and jitter via `RetryConfig`; non-idempotent
//! trading calls are never retried blindly.
//!
//! ## Architecture
//!
//! The crate is organized as a module-oriented library under `src/`:
//!
//! - **`application`** — all I/O. The REST `Client` and its service-trait
//!   implementations, `Auth` / `Session` (login, refresh, logout, account
//!   switching), `Config`, the `RateLimiter`, and the streaming layer
//!   (`StreamerClient`, `DynamicMarketStreamer`). Service traits live under
//!   `application::interfaces`.
//! - **`model`** — pure request / response DTOs, streaming message DTOs, session
//!   DTOs, and the retry policy. Serde only; no I/O.
//! - **`presentation`** — domain entities per area: account, chart, instrument,
//!   market, order, price, trade, transaction. Serde only; no I/O.
//! - **`storage`** — PostgreSQL persistence via `sqlx`, behind the
//!   `persistence` feature (sits on top of `model` / `presentation`).
//! - **`utils`** — leaf helpers: env-var config, logging, finance (P&L), parsing,
//!   deal-reference id generation.
//! - **`error`** — canonical typed error enums: `AppError`, `AuthError`, and
//!   `FetchError`.
//! - **`constants`** — crate-wide endpoint paths, header names, and defaults.
//! - **`prelude`** — curated public re-exports (the recommended import surface).
//!
//! ### API Documentation
//!
//! Browse the API documentation on [docs.rs](https://docs.rs/ig-client) or
//! generate it locally with:
//!
//! ```bash
//! make doc-open
//! ```
//!
//! ## Project Structure
//!
//! ```text
//! src/
//! ├── application/       # I/O: client, auth, config, rate limiter, streaming
//! │   ├── interfaces/    # Service traits (account, market, order, costs, …)
//! │   ├── auth.rs        # Auth / Session: login, refresh, logout, switch
//! │   ├── client.rs      # Client (REST services) + StreamerClient
//! │   ├── config.rs      # Config, Credentials, REST / WS / rate-limiter config
//! │   ├── rate_limiter.rs      # governor-backed request pacing
//! │   └── dynamic_streamer.rs  # DynamicMarketStreamer (subscriptions)
//! ├── model/             # Pure DTOs: requests, responses, streaming, retry
//! ├── presentation/      # Domain entities (account, market, order, price, …)
//! ├── storage/           # PostgreSQL persistence via sqlx (feature `persistence`)
//! ├── utils/             # config, logger, finance, parsing, id helpers
//! ├── constants.rs       # Endpoint paths, header names, defaults
//! ├── error.rs           # AppError / AuthError / FetchError
//! ├── prelude.rs         # Curated public re-exports
//! └── lib.rs             # Public API and crate docs (README source)
//! examples/              # Runnable demos (workspace members)
//! tests/                 # unit/ and env-gated integration/ tests
//! benches/               # Criterion benchmarks
//! ```
//!
//! ## Development
//!
//! This project includes a Makefile with common development tasks:
//!
//! ```bash
//! make build        # Debug build
//! make release      # Release build
//! make test         # Run all tests
//! make fmt          # Format code with rustfmt
//! make lint         # Run clippy
//! make readme       # Regenerate README.md from these crate docs
//! make pre-push     # Run all checks before pushing
//! ```
//!
//! ## Contributing
//!
//! Contributions are welcome:
//!
//! 1. Fork the repository.
//! 2. Create a feature branch: `git checkout -b feature/my-feature`.
//! 3. Make your changes and commit them.
//! 4. Run the checks: `make pre-push`.
//! 5. Push the branch and open a pull request.
//!
//! Please make sure your code passes all tests and linting checks before
//! submitting a pull request.

// Enables the "Available on crate feature ..." badges on docs.rs. Inert on
// stable: nothing sets `docsrs` outside the docs.rs builder.
#![cfg_attr(docsrs, feature(doc_cfg))]

/// Core application logic and services
pub mod application;

/// User interface and presentation layer
pub mod presentation;

/// Module containing global constants used throughout the library
pub mod constants;

/// Error types and handling
pub mod error;

/// Data persistence and storage
pub mod storage;

/// Data models for IG Markets API entities
pub mod model;
/// Prelude module for convenient imports
pub mod prelude;
/// Utility functions and helpers
pub mod utils;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the library version
pub fn version() -> &'static str {
    VERSION
}
