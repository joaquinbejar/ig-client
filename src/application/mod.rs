/// Authentication and session management
pub mod auth;
/// Main client implementation
pub mod client;
/// Application configuration module
pub mod config;
/// Dynamic market streamer with thread-safe subscription management
#[cfg(feature = "streaming")]
#[cfg_attr(docsrs, doc(cfg(feature = "streaming")))]
pub mod dynamic_streamer;
/// HTTP client and request execution with rate limiting and finite retry
pub mod http;
/// Service interfaces and traits
pub mod interfaces;
/// Market navigation hierarchy traversal helpers
pub mod market_hierarchy;
/// Rate limiter module for API request throttling
pub mod rate_limiter;
/// Adapters from Lightstreamer `ItemUpdate` to presentation-layer DTOs
#[cfg(feature = "streaming")]
#[cfg_attr(docsrs, doc(cfg(feature = "streaming")))]
pub mod streaming_convert;
