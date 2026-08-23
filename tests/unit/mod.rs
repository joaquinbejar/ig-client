//! Offline unit tests: pure logic and HTTP behaviour driven against a mock
//! server, with no IG credentials and no network.

mod application;
mod error_tests;
mod model;
mod presentation;
#[cfg(feature = "persistence")]
mod storage;
mod utils;
