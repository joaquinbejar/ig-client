// Common utilities for integration tests

use ig_client::prelude::*;
use tokio::runtime::Runtime;
use tracing::info;

/// Creates a test client
pub fn create_test_client() -> Client {
    Client::try_new().expect("client construction should succeed")
}

/// Performs login and optionally switches to the account specified in the config
pub fn login_with_account_switch() -> Session {
    setup_logger();

    // Create a runtime for the async operations
    let rt = Runtime::new().expect("Failed to create runtime");

    // Login and get a session
    rt.block_on(async {
        login_with_account_switch_async()
            .await
            .expect("Failed to login")
    })
}

/// Async version of login_with_account_switch for use in async tests
/// Returns a Result with the session or an error message
/// Note: Account switching is now automatic during Client initialization
pub async fn login_with_account_switch_async() -> Result<Session, String> {
    setup_logger();
    let http_client = HttpClient::new_lazy(Config::default())
        .map_err(|e| format!("lazy HTTP client construction failed: {e}"))?;

    // Login and get a session
    // The Client automatically handles account switching during initialization
    // based on the configured account_id
    match http_client.get_session().await {
        Ok(session) => {
            info!("Logged in with account: {}", session.account_id);
            Ok(session)
        }
        Err(e) => Err(format!("Failed to login: {e:?}")),
    }
}
