# Integration Guide for OAuth Token Refresh

For the current catalog integration and migration from 0.16.5, use
[Market catalog enumeration and expiry](./MARKET_CATALOG.md) and the
[DATA-ENGINE 0.17.0 handoff](./DATA_ENGINE_0.17.0_HANDOFF.md). High-level enumeration
uses categories; the navigation examples below concern the older authentication
integration and do not describe the catalog traversal.

## Problem Statement

If you're seeing this error in your application:

```
ERROR: Unauthorized request to https://demo-api.ig.com/gateway/deal/marketnavigation/...
ERROR: Response body: {"errorCode":"error.security.oauth-token-invalid"}
```

This means your OAuth access token has expired and needs to be refreshed. This guide explains how to integrate automatic token refresh into your existing application.

> **Cargo features**: token refresh is part of the REST/session path and works
> with `default-features = false`. The default-ON `streaming` and `persistence`
> features only add the Lightstreamer client and the `storage` module
> respectively.

## Understanding the Issue

OAuth tokens (API v3) expire after a certain period. When a token expires:
1. All API requests return `401 Unauthorized`
2. The response body contains `{"errorCode":"error.security.oauth-token-invalid"}`
3. You need to use the `refresh_token` to obtain a new `access_token`

## Solution Overview

There are two main approaches to handle token expiration:

### Approach 1: Shared Mutable Session (Recommended for Web Servers/Long-Running Apps)

Use `Arc<Mutex<IgSession>>` to share the session across your application and update it when needed.

### Approach 2: Periodic Refresh (Recommended for Background Jobs)

Check and refresh the token periodically before making API calls.

## Implementation Steps

### Step 1: Update Your Application Structure

#### Before (Problematic):
```rust
// Session is immutable and shared
let session = auth.login().await?;
let session = Arc::new(session);

// Later in your code...
market_service.get_market_navigation(&session).await?; // ❌ Fails when token expires
```

#### After (Correct):
```rust
// Session is mutable and protected by Mutex
let session = auth.login().await?;
let session = Arc::new(Mutex::new(session));

// Store auth for later use
let auth = Arc::new(auth);
```

### Step 2: Create a Helper Function for API Calls

```rust
use ig_client::error::AppError;
use ig_client::session::auth::IgAuth;
use ig_client::session::interface::{IgAuthenticator, IgSession};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Makes an API call with automatic token refresh on expiration
async fn api_call_with_retry<F, T>(
    session: &Arc<Mutex<IgSession>>,
    auth: &IgAuth<'_>,
    operation: F,
) -> Result<T, AppError>
where
    F: Fn(&IgSession) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, AppError>> + '_>>,
{
    // Check if token needs refresh
    {
        let mut sess = session.lock().await;
        if sess.needs_token_refresh(Some(300)) {
            match auth.refresh(&sess).await {
                Ok(new_session) => {
                    *sess = new_session;
                }
                Err(_) => {
                    // If refresh fails, try full re-authentication
                    *sess = auth.login().await.map_err(|_| AppError::Unauthorized)?;
                }
            }
        }
    }

    // Try the operation
    let sess = session.lock().await;
    match operation(&sess).await {
        Ok(result) => Ok(result),
        Err(AppError::OAuthTokenExpired) => {
            // Token expired during operation - refresh and retry
            drop(sess);
            
            let mut sess = session.lock().await;
            match auth.refresh(&sess).await {
                Ok(new_session) => {
                    *sess = new_session;
                    drop(sess);
                    
                    let sess = session.lock().await;
                    operation(&sess).await
                }
                Err(_) => Err(AppError::Unauthorized),
            }
        }
        Err(e) => Err(e),
    }
}
```

### Step 3: Update Your API Calls

#### Before:
```rust
// Direct API call (fails when token expires)
let navigation = market_service.get_market_navigation(&session).await?;
```

#### After:
```rust
// API call with automatic retry
let navigation = api_call_with_retry(
    &session,
    &auth,
    |s| Box::pin(async move {
        market_service.get_market_navigation(s).await
    })
).await?;
```

### Step 4: Add Periodic Refresh (Optional but Recommended)

For long-running applications, add a background task that refreshes the token periodically:

```rust
use tokio::time::{interval, Duration};

// Spawn background refresh task
let session_clone = session.clone();
let auth_clone = auth.clone();

tokio::spawn(async move {
    let mut refresh_interval = interval(Duration::from_secs(1800)); // 30 minutes
    
    loop {
        refresh_interval.tick().await;
        
        let mut sess = session_clone.lock().await;
        if sess.needs_token_refresh(Some(300)) {
            match auth_clone.refresh(&sess).await {
                Ok(new_session) => {
                    *sess = new_session;
                    info!("Token refreshed successfully");
                }
                Err(e) => {
                    error!("Failed to refresh token: {:?}", e);
                }
            }
        }
    }
});
```

## Complete Example for Web Server

Here's a complete example for integrating token refresh into a web server (e.g., Axum, Actix-web):

```rust
use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use ig_client::{
    application::services::{MarketService, market_service::MarketServiceImpl},
    config::Config,
    error::AppError,
    session::{auth::IgAuth, interface::{IgAuthenticator, IgSession}},
    transport::http_client::IgHttpClientImpl,
};
use std::sync::Arc;
use tokio::sync::Mutex;

// Application state
struct AppState {
    session: Arc<Mutex<IgSession>>,
    auth: Arc<IgAuth<'static>>,
    market_service: Arc<MarketServiceImpl<IgHttpClientImpl>>,
}

// API handler
async fn get_navigation(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let navigation = api_call_with_retry(
        &state.session,
        &state.auth,
        |s| Box::pin(async move {
            state.market_service.get_market_navigation(s).await
        })
    ).await?;
    
    Ok(Json(serde_json::to_value(navigation)?))
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Initialize
    let config = Arc::new(Config::new());
    let auth = Arc::new(IgAuth::new(&config));
    let session = Arc::new(Mutex::new(auth.login().await?));
    
    let http_client = Arc::new(IgHttpClientImpl::new(config.clone()));
    let market_service = Arc::new(MarketServiceImpl::new(config, http_client));
    
    let state = Arc::new(AppState {
        session: session.clone(),
        auth: auth.clone(),
        market_service,
    });
    
    // Start periodic refresh
    let session_clone = session.clone();
    let auth_clone = auth.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1800));
        loop {
            interval.tick().await;
            let mut sess = session_clone.lock().await;
            if sess.needs_token_refresh(Some(300)) {
                if let Ok(new_session) = auth_clone.refresh(&sess).await {
                    *sess = new_session;
                }
            }
        }
    });
    
    // Build router
    let app = Router::new()
        .route("/api/v1/navigation", get(get_navigation))
        .with_state(state);
    
    // Run server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
```

## Quick Fix for Existing Code

If you have existing code that's failing, here's the minimal change needed:

### 1. Make session mutable:
```rust
// Change from:
let session = Arc::new(auth.login().await?);

// To:
let session = Arc::new(Mutex::new(auth.login().await?));
```

### 2. Before each API call, check and refresh:
```rust
// Add this before your API calls:
{
    let mut sess = session.lock().await;
    if sess.needs_token_refresh(Some(300)) {
        if let Ok(new_session) = auth.refresh(&sess).await {
            *sess = new_session;
        }
    }
}

// Then make your API call:
let sess = session.lock().await;
let result = market_service.get_market_navigation(&sess).await?;
```

## Testing Your Integration

To test that token refresh is working:

1. Set a short token expiry time (if possible in your test environment)
2. Make API calls over an extended period
3. Monitor logs for "Token refreshed successfully" messages
4. Verify that API calls continue to work after the initial token expires

## Troubleshooting

### Error: "Failed to refresh token"
- The refresh token itself may have expired
- Solution: Catch the error and call `auth.login()` to re-authenticate

### Error: "Cannot borrow as mutable"
- You're trying to refresh a non-mutable session
- Solution: Use `Arc<Mutex<IgSession>>` instead of `Arc<IgSession>`

### Tokens still expiring
- You may not be checking frequently enough
- Solution: Reduce the refresh interval or safety margin

## See Also

- [OAuth Token Refresh Guide](./OAUTH_TOKEN_REFRESH.md)
- [Long-Running Application Example](../examples/oauth_long_running_example.rs)
- [Basic OAuth Example](../examples/oauth_token_refresh_example.rs)
