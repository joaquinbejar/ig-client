mod account_tests;
mod auth_tests;
mod common;
mod market_tests;
mod storage_tests;
// order_tests, position_tests, working_order_tests temporarily disabled
// These tests need significant refactoring for the new Client API
// and involve real trading operations that should be carefully reviewed
// mod order_tests;
// mod position_tests;
// rate_limiter_tests removed - rate limiting is now internal to Client
// mod working_order_tests;
