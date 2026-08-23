//! Integration tests that talk to IG. They need real credentials in the
//! environment and are ignored by default.

mod account_tests;
mod auth_tests;
mod common;
mod market_tests;
#[cfg(feature = "persistence")]
mod storage_tests;
