# New Simplified Client API

For catalog enumeration in 0.17.0, see
[Market catalog enumeration and expiry](./MARKET_CATALOG.md), including public
struct changes and DATA-ENGINE adoption steps.

## Overview

The new `Client` API provides a dramatically simplified interface for using the IG Markets API. It handles all authentication complexity internally, including:

- ✅ Automatic login (API v2 or v3)
- ✅ Automatic OAuth token refresh
- ✅ Automatic re-authentication when tokens expire
- ✅ Transparent error handling and retry logic
- ✅ Simple, clean API for making requests

## Cargo features

Everything documented here — `Client`, `Client::with_config` and the REST
service traits — is available with **no features enabled**. Two default-ON
features gate the heavy legs:

- `streaming` (pulls `lightstreamer-rs` 1.0, MIT) — `StreamerClient`,
  `DynamicMarketStreamer`, `Listener`.
- `persistence` (pulls `sqlx`) — the `storage` module.

For a REST/poll-only client with neither crate in the dependency graph:

```toml
ig-client = { version = "0.12.3", default-features = false }
```

## Quick Start

### Basic Usage

```rust
use ig_client::application::client::Client;
use ig_client::application::interfaces::account::AccountService;
use ig_client::error::AppError;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Create client - authentication is handled automatically!
    let client = Client::new();
    
    // Make API calls using service traits
    let accounts = client.get_accounts().await?;
    
    println!("Accounts: {:?}", accounts);
    Ok(())
}
```

That's it! No need to:
- Manually login
- Store session tokens
- Check token expiration
- Refresh tokens
- Handle authentication errors

## API Methods

### Creating a Client

```rust
// Create and authenticate immediately
let client = Client::new(config).await?;

// Or create without authenticating (lazy initialization)
let client = Client::new_lazy(config);
// Authentication happens on first request
```

### Making Requests

```rust
// GET request
let accounts: AccountsResponse = client.get("/accounts").await?;

// POST request
let body = CreateOrderRequest { /* ... */ };
let response: OrderResponse = client.post("/workingorders/otc", body).await?;

// PUT request
let body = UpdateOrderRequest { /* ... */ };
let response: OrderResponse = client.put("/workingorders/otc/123", body).await?;

// DELETE request
let response: DeleteResponse = client.delete("/workingorders/otc/123").await?;

// Custom request with specific API version
let response: Response = client.request(
    Method::GET,
    "/accounts",
    None::<()>,
    Some("2")  // Use API v2
).await?;
```

### Account Management

```rust
// Switch to a different account
client.switch_account("ACCOUNT_ID", Some(true)).await?;

// Get current session info
let session = client.get_session().await?;
println!("Current account: {}", session.account_id);

// Check if using OAuth
if session.is_oauth() {
    println!("Using OAuth authentication");
}

// Logout
client.logout().await?;
```

## How It Works

### Automatic Token Refresh

The client automatically handles OAuth token expiration:

1. **Proactive Refresh**: Before each request, checks if token needs refresh
2. **Reactive Refresh**: If a request fails with `oauth-token-invalid`, automatically refreshes and retries
3. **Fallback Re-authentication**: If refresh token is expired, performs full re-authentication

```rust
// You just make requests - token management is automatic!
for i in 1..=100 {
    let accounts = client.get("/accounts").await?;
    // Even if token expires during the loop, it's handled automatically
    tokio::time::sleep(Duration::from_secs(60)).await;
}
```

### Error Handling

The client provides clean error handling:

```rust
use ig_client::error::AppError;

match client.get::<AccountsResponse>("/accounts").await {
    Ok(accounts) => println!("Success: {:?}", accounts),
    Err(AppError::Unauthorized) => println!("Authentication failed"),
    Err(AppError::OAuthTokenExpired) => {
        // This should never happen - client handles it automatically!
        println!("Token expired (shouldn't see this)")
    }
    Err(AppError::RateLimitExceeded) => println!("Rate limit hit"),
    Err(e) => println!("Other error: {:?}", e),
}
```

## Comparison: Old vs New API

### Old API (Complex)

```rust
// Create config
let config = Arc::new(Config::new());

// Create HTTP client
let http_client = Arc::new(IgHttpClientImpl::new(config.clone()));

// Create authenticator
let auth = IgAuth::new(&config);

// Login
let mut session = auth.login().await?;

// Create service
let account_service = AccountServiceImpl::new(config.clone(), http_client);

// Make request
let accounts = account_service.get_accounts(&session).await?;

// Handle token expiration manually
if session.needs_token_refresh(Some(300)) {
    session = auth.refresh(&session).await?;
}

// Make another request
let accounts = account_service.get_accounts(&session).await?;
```

### New API (Simple)

```rust
// Create client
let client = Client::new(Config::new()).await?;

// Make requests - everything is handled automatically!
let accounts: AccountsResponse = client.get("/accounts").await?;
let accounts: AccountsResponse = client.get("/accounts").await?;
```

## Advanced Usage

### Using with Existing Services

You can still use the new client with existing service implementations:

```rust
let client = Client::new(config).await?;

// Get the underlying Auth instance
let auth = client.auth();

// Get current session
let session = client.get_session().await?;

// Use with existing services
let market_service = MarketServiceImpl::new(config, http_client);
let markets = market_service.get_market_navigation(&session).await?;
```

### Long-Running Applications

For applications that run for extended periods:

```rust
let client = Client::new(config).await?;

// The client handles token refresh automatically
loop {
    // Make requests without worrying about token expiration
    let accounts = client.get::<AccountsResponse>("/accounts").await?;
    
    // Process data...
    
    tokio::time::sleep(Duration::from_secs(3600)).await;
}
```

### Type-Safe Responses

Use your own response types:

```rust
#[derive(Deserialize)]
struct AccountsResponse {
    accounts: Vec<Account>,
}

#[derive(Deserialize)]
struct Account {
    #[serde(rename = "accountId")]
    account_id: String,
    #[serde(rename = "accountName")]
    account_name: String,
    balance: Balance,
}

let accounts: AccountsResponse = client.get("/accounts").await?;
for account in accounts.accounts {
    println!("Account: {} - {}", account.account_id, account.account_name);
}
```

## Migration Guide

### From Old API to New API

1. **Replace service creation**:
   ```rust
   // Old
   let http_client = Arc::new(IgHttpClientImpl::new(config.clone()));
   let account_service = AccountServiceImpl::new(config, http_client);
   
   // New
   let client = Client::new(config).await?;
   ```

2. **Replace authentication**:
   ```rust
   // Old
   let auth = IgAuth::new(&config);
   let session = auth.login().await?;
   
   // New
   // Authentication is automatic!
   ```

3. **Replace API calls**:
   ```rust
   // Old
   let accounts = account_service.get_accounts(&session).await?;
   
   // New
   let accounts: AccountsResponse = client.get("/accounts").await?;
   ```

4. **Remove token refresh logic**:
   ```rust
   // Old
   if session.needs_token_refresh(Some(300)) {
       session = auth.refresh(&session).await?;
   }
   
   // New
   // Not needed - handled automatically!
   ```

## Examples

See the following examples:

### Account & Authentication
- `examples/other/` - Account preferences, operations
- `examples/positions/` - Position management

### Market Data
- `examples/market/` - Market search and details
- `examples/sentiment/` - Client sentiment data (v0.11.1)
- `examples/watchlist/` - Watchlist management (v0.11.1)

### Trading
- `examples/orders/` - Order creation and management
- `examples/costs/` - Indicative costs (v0.11.1)

### Streaming
- `examples/streaming/` - Real-time data streaming
- `examples/chart/` - Chart data

Run with:
```bash
cargo run -p examples_watchlist --bin watchlist_list
cargo run -p examples_sentiment --bin sentiment_single
```

## Benefits

### For Developers

- ✅ **Less Code**: 90% reduction in boilerplate
- ✅ **Fewer Errors**: No manual token management
- ✅ **Cleaner**: Simple, intuitive API
- ✅ **Safer**: Automatic error handling and retry logic

### For Applications

- ✅ **More Reliable**: Automatic token refresh prevents failures
- ✅ **Better UX**: No interruptions due to expired tokens
- ✅ **Easier Maintenance**: Less code to maintain
- ✅ **Future-Proof**: Handles both API v2 and v3

## Technical Details

### Architecture

```
Client
├── Auth (manages authentication)
│   ├── Session (current session state)
│   ├── login() - Initial authentication
│   ├── refresh_token() - OAuth token refresh
│   └── switch_account() - Account switching
└── HTTP Client (makes requests)
    ├── Automatic token refresh on 401
    ├── Automatic retry logic
    └── Error handling
```

### Thread Safety

The `Client` is thread-safe and can be shared across threads:

```rust
let client = Arc::new(Client::new(config).await?);

let client_clone = client.clone();
tokio::spawn(async move {
    let accounts = client_clone.get::<AccountsResponse>("/accounts").await?;
    // ...
});
```

### Performance

- **Minimal Overhead**: Only checks token expiration when needed
- **Efficient**: Reuses HTTP connections
- **Smart Caching**: Session state is cached internally

## Available Services (v0.11.1)

The Client implements the following service traits:

| Service | Description |
|---------|-------------|
| `AccountService` | Account info, positions, working orders, preferences, activity |
| `MarketService` | Market search, details, historical prices, category catalog enumeration, explicit navigation |
| `OrderService` | Order creation, confirmation, position management |
| `WatchlistService` | Watchlist CRUD, add/remove instruments |
| `SentimentService` | Client sentiment for single/multiple markets |
| `CostsService` | Indicative costs for opening/closing positions |
| `OperationsService` | API application management |

## See Also

- [Market catalog enumeration and expiry](./MARKET_CATALOG.md)
- [OAuth Token Refresh Guide](./OAUTH_TOKEN_REFRESH.md)
- [Integration Guide](./INTEGRATION_GUIDE.md)
- [API Documentation](../src/application/client.rs)
