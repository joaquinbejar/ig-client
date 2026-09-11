# OAuth Token Refresh Guide

This guide explains how to handle OAuth token expiration and automatic renewal when using the IG Markets API client with OAuth authentication (API v3).

## Overview

When using OAuth authentication (API v3), access tokens expire after a certain period (typically indicated in the `expires_in` field). The client provides several mechanisms to handle token expiration:

1. **Proactive refresh** - Check and refresh tokens before they expire
2. **Reactive refresh** - Handle expiration errors and refresh automatically
3. **Helper utilities** - Use built-in utilities for automatic token management

## Token Expiration Detection

The client automatically detects OAuth token expiration in two ways:

### 1. Proactive Detection

Check if a token needs refresh before making API calls:

```rust
use ig_client::session::interface::IgSession;

// Check if token needs refresh (with 5 minute safety margin)
if session.needs_token_refresh(Some(300)) {
    println!("Token needs refresh");
    session = auth.refresh(&session).await?;
}
```

### 2. Reactive Detection

The HTTP client automatically detects the `error.security.oauth-token-invalid` error and returns `AppError::OAuthTokenExpired`:

```rust
use ig_client::error::AppError;

match service.some_operation(&session).await {
    Err(AppError::OAuthTokenExpired) => {
        // Token expired - need to refresh
    }
    result => result
}
```

## Refresh Methods

### Manual Refresh

Use the `refresh()` method from the `IgAuthenticator` trait:

```rust
use ig_client::session::auth::IgAuth;
use ig_client::session::interface::IgAuthenticator;

let auth = IgAuth::new(&config);
let mut session = auth.login().await?;

// Later, when token expires or needs refresh
session = auth.refresh(&session).await?;
```

### Using Helper Utilities

The client provides helper utilities in `utils::session_helper` for automatic token management:

#### Option 1: `refresh_if_needed`

Simple utility that checks and refreshes if necessary:

```rust
use ig_client::utils::session_helper::refresh_if_needed;

let mut session = auth.login().await?;

// Before making API calls
refresh_if_needed(&mut session, &auth, Some(300)).await?;

// Now make your API calls
let accounts = account_service.get_accounts(&session).await?;
```

#### Option 2: `with_auto_refresh`

Wraps an operation with automatic token refresh:

```rust
use ig_client::utils::session_helper::with_auto_refresh;

let mut session = auth.login().await?;

let result = with_auto_refresh(
    &mut session,
    &auth,
    |s| async move { 
        market_service.get_market_details(s, "EPIC").await 
    }
).await?;
```

## Recommended Patterns

### Pattern 1: Periodic Refresh in Long-Running Processes

For applications that run for extended periods:

```rust
use tokio::time::{interval, Duration};

let mut session = auth.login().await?;
let mut refresh_interval = interval(Duration::from_secs(3600)); // 1 hour

loop {
    tokio::select! {
        _ = refresh_interval.tick() => {
            // Proactively refresh token every hour
            if let Err(e) = refresh_if_needed(&mut session, &auth, Some(300)).await {
                error!("Failed to refresh token: {:?}", e);
                // Re-authenticate if refresh fails
                session = auth.login().await?;
            }
        }
        // Your other async operations here
    }
}
```

### Pattern 2: Retry on Expiration

For individual API calls:

```rust
async fn make_api_call_with_retry(
    service: &impl AccountService,
    session: &mut IgSession,
    auth: &IgAuth<'_>,
) -> Result<AccountsResponse, AppError> {
    match service.get_accounts(session).await {
        Ok(result) => Ok(result),
        Err(AppError::OAuthTokenExpired) => {
            // Token expired - refresh and retry
            *session = auth.refresh(session).await
                .map_err(|_| AppError::Unauthorized)?;
            
            // Retry the operation
            service.get_accounts(session).await
        }
        Err(e) => Err(e),
    }
}
```

### Pattern 3: Proactive Check Before Operations

For batch operations or loops:

```rust
async fn process_markets(
    market_service: &impl MarketService,
    session: &mut IgSession,
    auth: &IgAuth<'_>,
    epics: Vec<String>,
) -> Result<(), AppError> {
    for epic in epics {
        // Check before each operation
        refresh_if_needed(session, auth, Some(300)).await?;
        
        // Make the API call
        let details = market_service.get_market_details(session, &epic).await?;
        println!("Market: {}", details.instrument.name);
    }
    Ok(())
}
```

## Runnable Client Example

[`client_with_config`](../examples/simples/src/bin/client_with_config.rs) uses
the current `Client` API, which manages session authentication and refresh
internally. Set `MYAPP_IG_USERNAME`, `MYAPP_IG_PASSWORD`, `MYAPP_IG_ACCOUNT_ID`,
and `MYAPP_IG_API_KEY` in the process environment, then run from the repository
root:

```bash
cargo run -p examples_simples --bin client_with_config
```

The example defaults to demo API v3, makes an account request, and does not
load `.env` or read `IG_*` credentials. Its short execution does not force
token expiry or prove a refresh occurred. Offline session-refresh regression
coverage is in
[`test_auth_flow.rs`](../tests/unit/application/test_auth_flow.rs).

## Important Notes

1. **Refresh Token Validity**: The refresh token itself can also expire. If refresh fails, you'll need to re-authenticate with `login()`.

2. **Thread Safety**: The `IgSession` is `Clone`, so you can share it across threads, but token refresh requires a mutable reference. Use appropriate synchronization (e.g., `Arc<Mutex<IgSession>>`) in multi-threaded scenarios.

3. **Safety Margin**: Always use a safety margin (e.g., 300 seconds = 5 minutes) when checking token expiration to account for network latency and clock skew.

4. **API Version**: OAuth token refresh only works with API v3. Ensure your configuration has `api_version = Some(3)`.

## Troubleshooting

### Error: `error.security.oauth-token-invalid`

This error indicates the OAuth access token has expired. Solutions:

1. Use `refresh_if_needed()` before making API calls
2. Catch `AppError::OAuthTokenExpired` and refresh the token
3. Use `with_auto_refresh()` to wrap your operations

### Refresh Fails with `Unauthorized`

If token refresh fails, the refresh token itself may have expired. Re-authenticate:

```rust
match auth.refresh(&session).await {
    Ok(new_session) => session = new_session,
    Err(_) => {
        // Refresh token expired - need to login again
        session = auth.login().await?;
    }
}
```

### Token Expires During Long Operations

For operations that take longer than the token validity period:

1. Break the operation into smaller chunks
2. Check and refresh the token between chunks
3. Use the `with_auto_refresh()` wrapper for automatic handling

## See Also

- [Client Configuration Example](../examples/simples/src/bin/client_with_config.rs)
- [Session and Token Types](../src/model/auth.rs)
- [Authentication and Refresh Implementation](../src/application/auth.rs)
