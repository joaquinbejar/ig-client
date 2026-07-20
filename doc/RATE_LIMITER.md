# Rate Limiter Configuration

## Overview

The IG Client includes built-in rate limiting using the `governor` crate to ensure compliance with IG Markets API rate limits. The rate limiter is automatically integrated into the authentication module and controls all API requests.

## Configuration

Rate limiting is configured through environment variables that are loaded when creating a `Config` instance.

### Environment Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `IG_RATE_LIMIT_MAX_REQUESTS` | Maximum number of requests allowed per period | 4 | 4 |
| `IG_RATE_LIMIT_PERIOD_SECONDS` | Time period in seconds for the rate limit | 12 | 12 |
| `IG_RATE_LIMIT_BURST_SIZE` | Maximum number of requests that can be made at once (burst) | 3 | 3 |

The defaults are the canonical `DEFAULT_CONFIG_RATE_LIMIT_MAX_REQUESTS`,
`DEFAULT_CONFIG_RATE_LIMIT_PERIOD_SECONDS` and
`DEFAULT_CONFIG_RATE_LIMIT_BURST_SIZE` constants in `src/constants.rs`, which
also back `RateLimiterConfig::default()` — the value used by the environment-free
`Config::from_credentials` path.

### Example .env File

```env
# Rate Limiter Configuration
IG_RATE_LIMIT_MAX_REQUESTS=4
IG_RATE_LIMIT_PERIOD_SECONDS=12
IG_RATE_LIMIT_BURST_SIZE=3
```

## How It Works

### Token Bucket Algorithm

The rate limiter uses a token bucket algorithm:

1. **Tokens**: Each API request consumes one token
2. **Bucket Size**: Defined by `burst_size` - allows bursts of requests
3. **Refill Rate**: Tokens are refilled based on `max_requests` per `period_seconds`

### Example Scenarios

#### Scenario 1: Steady Rate
```
Config: 4 requests per 12 seconds, burst size 3

- Can make 3 requests immediately (burst)
- Then limited to ~1 request every 3 seconds
- Over 12 seconds, can make 4 requests total
```

#### Scenario 2: Burst Handling
```
Config: 4 requests per 12 seconds, burst size 10

- Can make 10 requests immediately (larger burst)
- Then rate-limited to maintain the average of 4 per 12 seconds
```

## Usage

### Automatic Integration

The rate limiter is automatically used by the `Auth` module:

```rust
use ig_client::application::client::Client;
use ig_client::application::interfaces::account::AccountService;
use ig_client::error::AppError;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Create client (rate limiter is automatically initialized)
    let client = Client::new();
    
    // All API requests are automatically rate-limited
    let accounts = client.get_accounts().await?;
    
    println!("Accounts: {:?}", accounts);
    Ok(())
}
```

### Manual Usage

You can also use the rate limiter directly:

```rust
use ig_client::application::config::RateLimiterConfig;
use ig_client::application::rate_limiter::RateLimiter;

#[tokio::main]
async fn main() {
    let config = RateLimiterConfig {
        max_requests: 60,
        period_seconds: 60,
        burst_size: 10,
    };
    
    let limiter = RateLimiter::new(&config);
    
    // Wait until a request can be made
    limiter.wait().await;
    // Make your API request here
    
    // Or check if a request can be made immediately
    if limiter.check() {
        // Make request
    } else {
        // Handle rate limit
    }
}
```

## IG Markets API Limits

According to IG Markets documentation, the API has the following limits:

### Trading Limits
- **60 requests per minute** for trading operations
- Applies to: placing orders, modifying positions, etc.

### Non-Trading Limits
- **60 requests per minute** for non-trading operations
- Applies to: market data, account info, etc.

### Historical Data Limits
- **10,000 data points per week** for historical data requests

### Recommended Settings

For general use:
```env
IG_RATE_LIMIT_MAX_REQUESTS=4
IG_RATE_LIMIT_PERIOD_SECONDS=12
IG_RATE_LIMIT_BURST_SIZE=3
```

For conservative use (to stay well under limits):
```env
IG_RATE_LIMIT_MAX_REQUESTS=50
IG_RATE_LIMIT_PERIOD_SECONDS=60
IG_RATE_LIMIT_BURST_SIZE=5
```

For high-frequency applications:
```env
IG_RATE_LIMIT_MAX_REQUESTS=60
IG_RATE_LIMIT_PERIOD_SECONDS=60
IG_RATE_LIMIT_BURST_SIZE=20
```

## Monitoring

The rate limiter logs when it's waiting and when retrying:

```
DEBUG ig_client::client: Waiting for rate limiter...
WARN  ig_client::client: Rate limit exceeded (attempt 1): {"errorCode":"error.public-api.exceeded-api-key-allowance"}. Waiting 10 seconds before retry...
WARN  ig_client::client: Rate limit exceeded (attempt 2): {"errorCode":"error.public-api.exceeded-api-key-allowance"}. Waiting 10 seconds before retry...
```

You can monitor rate limiting behavior by enabling debug/warn logging:

```rust
use tracing_subscriber;

tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

### Example Log Output

When rate limits are exceeded, you'll see:

```
2025-10-20T03:38:39.251273Z  INFO ig_client::application::auth: Refreshing OAuth token
2025-10-20T03:38:39.304788Z  WARN ig_client::application::client: Rate limit exceeded (attempt 1): {"errorCode":"error.public-api.exceeded-api-key-allowance"}. Waiting 10 seconds before retry...
2025-10-20T03:38:49.305123Z  DEBUG ig_client::application::client: Retrying request after rate limit delay
2025-10-20T03:38:49.456789Z  INFO ig_client::application::client: Request successful after retry
```

## Best Practices

1. **Set Conservative Limits**: Start with conservative settings and adjust based on your needs
2. **Monitor Your Usage**: Watch for rate limit warnings in logs
3. **Handle Errors Gracefully**: Implement retry logic with exponential backoff
4. **Separate Limiters**: Consider separate rate limiters for trading vs non-trading operations
5. **Test Thoroughly**: Test your application under load to ensure rate limits are respected

## Error Handling

If you exceed rate limits, the IG API will return:

```json
{
    "errorCode": "error.public-api.exceeded-api-key-allowance"
}
```

The rate limiter prevents this by:
1. **Proactive rate limiting**: Waiting before making requests based on configured limits
2. **Automatic retry**: If a rate limit error is still received (403 with exceeded-api-key-allowance), the client automatically:
   - Logs a warning with the retry attempt number
   - Waits 10 seconds
   - Retries the request indefinitely until it succeeds

This two-layer approach ensures maximum reliability:
- The local rate limiter prevents most rate limit errors
- The automatic retry handles edge cases where the API's limits differ from configuration

## Advanced Configuration

### Multiple Rate Limiters

For applications that need separate limits for different operations:

```rust
use ig_client::application::config::RateLimiterConfig;
use ig_client::application::rate_limiter::RateLimiter;

// Trading operations limiter
let trading_config = RateLimiterConfig {
    max_requests: 60,
    period_seconds: 60,
    burst_size: 5,
};
let trading_limiter = RateLimiter::new(&trading_config);

// Market data limiter
let market_data_config = RateLimiterConfig {
    max_requests: 60,
    period_seconds: 60,
    burst_size: 20,
};
let market_data_limiter = RateLimiter::new(&market_data_config);
```

### Dynamic Adjustment

You can create new rate limiters with different settings at runtime:

```rust
// Start with conservative settings
let mut config = RateLimiterConfig {
    max_requests: 30,
    period_seconds: 60,
    burst_size: 5,
};

let limiter = RateLimiter::new(&config);

// Later, adjust settings
config.max_requests = 60;
let new_limiter = RateLimiter::new(&config);
```

## See Also

- [Configuration Guide](./CONFIGURATION.md)
- [Authentication Guide](./AUTHENTICATION.md)
- [IG Markets API Documentation](https://labs.ig.com/rest-trading-api-reference)
