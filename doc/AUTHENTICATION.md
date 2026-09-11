# Authentication Guide

This guide explains how to authenticate with the IG Markets API using both API v2 and v3.

## Overview

The IG Markets API supports two authentication methods:

- **API v2**: Uses CST (Client Session Token) and X-SECURITY-TOKEN headers
- **API v3**: Uses OAuth 2.0 with access and refresh tokens

## API Version 2 (CST/X-SECURITY-TOKEN)

### Description

API v2 uses traditional session-based authentication with two tokens returned in HTTP headers:
- `CST`: Client Session Token
- `X-SECURITY-TOKEN`: Security token for API requests

### Configuration

Set the API version to 2 in your configuration:

```rust
use ig_client::config::Config;
use ig_client::utils::rate_limiter::RateLimitType;

let mut config = Config::with_rate_limit_type(RateLimitType::NonTradingAccount, 0.8);
config.api_version = Some(2);
```

Or via environment variable:

```bash
export IG_API_VERSION=2
```

### Usage

```rust
use ig_client::session::auth::IgAuth;
use ig_client::session::interface::IgAuthenticator;

let auth = IgAuth::new(&config);
let session = auth.login().await?;

// Session will have CST and X-SECURITY-TOKEN
println!("CST: {}", session.cst);
println!("Token: {}", session.token);
```

### Session Structure

```rust
IgSession {
    cst: String,                    // Client Session Token
    token: String,                  // X-SECURITY-TOKEN
    oauth_token: None,              // Not used in v2
    account_id: String,
    // ... other fields
}
```

## API Version 3 (OAuth)

### Description

API v3 uses OAuth 2.0 authentication, providing:
- Access token for API requests
- Refresh token for obtaining new access tokens
- Token expiration time
- Additional metadata (client ID, Lightstreamer endpoint)

### Configuration

Set the API version to 3 in your configuration:

```rust
use ig_client::config::Config;
use ig_client::utils::rate_limiter::RateLimitType;

let mut config = Config::with_rate_limit_type(RateLimitType::NonTradingAccount, 0.8);
config.api_version = Some(3);
```

Or via environment variable:

```bash
export IG_API_VERSION=3
```

### Usage

```rust
use ig_client::session::auth::IgAuth;
use ig_client::session::interface::IgAuthenticator;

let auth = IgAuth::new(&config);
let session = auth.login().await?;

// Session will have OAuth tokens
if let Some(oauth) = &session.oauth_token {
    println!("Access token: {}", oauth.access_token);
    println!("Refresh token: {}", oauth.refresh_token);
    println!("Expires in: {} seconds", oauth.expires_in);
    println!("Token type: {}", oauth.token_type);
}

// Additional fields available in v3
println!("Client ID: {}", session.client_id);
println!("Lightstreamer endpoint: {}", session.lightstreamer_endpoint);
```

### Session Structure

```rust
IgSession {
    cst: String::new(),             // Empty in v3
    token: String::new(),           // Empty in v3
    oauth_token: Some(OAuthToken {
        access_token: String,
        refresh_token: String,
        scope: String,
        token_type: String,         // Usually "Bearer"
        expires_in: String,         // Expiration in seconds
    }),
    account_id: String,
    client_id: String,              // Populated in v3
    lightstreamer_endpoint: String, // Populated in v3
    // ... other fields
}
```

## Auto-Detection (Default)

If you don't specify an API version, the client defaults to **v3 (OAuth)**:

```rust
let config = Config::with_rate_limit_type(RateLimitType::NonTradingAccount, 0.8);
// config.api_version is None, will use v3
```

## Checking Authentication Type

You can check which authentication method a session is using:

```rust
if session.is_oauth() {
    println!("Using OAuth (v3) authentication");
} else if session.is_cst_auth() {
    println!("Using CST (v2) authentication");
}
```

## Direct Method Calls

You can also call the version-specific methods directly:

```rust
// Force API v2
let session = auth.login_v2().await?;

// Force API v3
let session = auth.login_v3().await?;
```

## Examples

The workspace example
[`client_with_config`](../examples/simples/src/bin/client_with_config.rs)
constructs a client with injected configuration and makes an account request.
Run it from the repository root after setting `MYAPP_IG_USERNAME`,
`MYAPP_IG_PASSWORD`, `MYAPP_IG_ACCOUNT_ID`, and `MYAPP_IG_API_KEY` in the process
environment:

```bash
cargo run -p examples_simples --bin client_with_config
```

This example uses the `MYAPP_IG_*` namespace and does not load `.env` or read
`IG_*` credentials. It defaults to the demo endpoint and API v3. Login occurs
on the first API request. The example demonstrates this configuration path;
it does not compare v2 and v3 sessions or force token expiry.

## API Request Format

### API v2 Requests

```http
GET /api/endpoint HTTP/1.1
Host: api.ig.com
X-IG-API-KEY: your-api-key
CST: your-cst-token
X-SECURITY-TOKEN: your-security-token
```

### API v3 Requests

```http
GET /api/endpoint HTTP/1.1
Host: api.ig.com
X-IG-API-KEY: your-api-key
Authorization: Bearer your-access-token
```

## Migration Guide

### From v2 to v3

1. Update your configuration to use API v3:
   ```rust
   config.api_version = Some(3);
   ```

2. Update code that accesses tokens:
   ```rust
   // v2
   let cst = session.cst;
   let token = session.token;
   
   // v3
   if let Some(oauth) = &session.oauth_token {
       let access_token = &oauth.access_token;
   }
   ```

3. Use the new fields available in v3:
   ```rust
   let client_id = session.client_id;
   let lightstreamer_endpoint = session.lightstreamer_endpoint;
   ```

## Recommendations

- **New projects**: Use API v3 (OAuth) for better security and modern authentication
- **Existing projects**: Continue using API v2 if already implemented, or migrate to v3
- **WebSocket connections**: API v3 provides the Lightstreamer endpoint directly

## Environment Variables

```bash
# Required for both versions
export IG_USERNAME="your-username"
export IG_PASSWORD="your-password"
export IG_API_KEY="your-api-key"

# Optional: Specify API version (2 or 3)
export IG_API_VERSION=3

# Optional: Specify environment
export IG_REST_BASE_URL="https://demo-api.ig.com/gateway/deal"  # Demo
# export IG_REST_BASE_URL="https://api.ig.com/gateway/deal"     # Production
```

## Troubleshooting

### Invalid API Version Error

```
Error: Invalid API version: X. Must be 2 or 3
```

**Solution**: Ensure `api_version` is set to either `Some(2)` or `Some(3)`, or `None` for auto-detection.

### Missing OAuth Tokens

If you're using v3 but `session.oauth_token` is `None`, check that:
1. The API version is correctly set to 3
2. The authentication was successful
3. You're using the correct API endpoint

### CST Tokens Empty in v3

This is expected behavior. In v3, `session.cst` and `session.token` are empty strings. Use `session.oauth_token` instead.
