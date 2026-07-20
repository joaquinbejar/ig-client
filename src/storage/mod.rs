/// Module containing database configuration structures
pub mod config;
/// Historical prices storage and retrieval
#[cfg(feature = "persistence")]
#[cfg_attr(docsrs, doc(cfg(feature = "persistence")))]
pub mod historical_prices;
/// Market data database operations
#[cfg(feature = "persistence")]
#[cfg_attr(docsrs, doc(cfg(feature = "persistence")))]
pub mod market_database;
/// Market hierarchy persistence models
#[cfg(feature = "persistence")]
#[cfg_attr(docsrs, doc(cfg(feature = "persistence")))]
pub mod market_persistence;
/// Storage utility functions
#[cfg(feature = "persistence")]
#[cfg_attr(docsrs, doc(cfg(feature = "persistence")))]
pub mod utils;
