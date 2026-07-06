//! Database configuration for the storage layer.
//!
//! The `DatabaseConfig` data type is defined in `crate::application::config`
//! (alongside the other `*Config` types, so [`crate::application::config::Config`]
//! can embed it without an application → storage dependency). It is re-exported
//! here to preserve the historical `crate::storage::config::DatabaseConfig`
//! public path and to keep the storage-layer's pool construction co-located with
//! the config it consumes.

pub use crate::application::config::DatabaseConfig;
