# Rate Limiter Configuration

## Overview

The IG Client uses `governor` to pace authentication and API requests through
the shared HTTP client. Each request is charged to its endpoint class; retries
and session replays consume request capacity too. Local pacing cannot guarantee
that a server allowance is available.

## Configuration

`Config::new()` reads the following environment variables. The environment-free
`Config::from_credentials()` path can override the resulting configuration's
`rate_limiter` field with an explicit `RateLimiterConfig`.

### Environment Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `IG_RATE_LIMIT_MAX_REQUESTS` | Number of non-trading tokens replenished per period | 4 | 4 |
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

Each endpoint class has a token bucket:

1. **Tokens**: Each API request consumes one token
2. **Bucket Size**: Non-trading uses `burst_size`; trading and historical use a fixed burst of one
3. **Refill Rate**: The non-trading bucket replenishes at `max_requests` per `period_seconds`

This is an average refill rate with burst capacity, not a fixed-window request
count. The shared account bucket can impose additional waits.

### Example Scenarios

#### Scenario 1: Steady Rate
```
Config: 4 requests per 12 seconds, burst size 3

- Can make 3 requests immediately (burst)
- Then limited to ~1 request every 3 seconds
- Four tokens replenish over 12 seconds; the initial burst is additional capacity
```

#### Scenario 2: Burst Handling
```
Config: 4 requests per 12 seconds, burst size 10

- Can make 10 requests immediately (larger burst)
- Then rate-limited to maintain the average of 4 per 12 seconds
```

## Usage

### Automatic Integration

Use `Client` service methods to retain shared pacing and session handling:

```rust
use ig_client::application::client::Client;
use ig_client::application::interfaces::account::AccountService;
use ig_client::error::AppError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Create client (rate limiter is automatically initialized)
    let client = Client::try_new()?;
    
    // All API requests are automatically rate-limited
    let accounts = client.get_accounts().await?;
    
    info!(count = accounts.accounts.len(), "accounts retrieved");
    Ok(())
}
```

### Manual Usage

You can also use a standalone limiter. `RateLimiter::new` is infallible and
returns `RateLimiter`; `Client::try_new` and service calls return `Result`.

```rust
use ig_client::application::config::RateLimiterConfig;
use ig_client::application::rate_limiter::{RateLimitClass, RateLimiter};

#[tokio::main]
async fn main() {
    let config = RateLimiterConfig {
        max_requests: 60,
        period_seconds: 60,
        burst_size: 10,
    };
    
    let limiter = RateLimiter::new(&config);
    
    // Wait for and consume one non-trading request slot.
    limiter.wait_for(RateLimitClass::NonTrading).await;
    // Make your API request here
    
    // For a different request, try to consume a slot immediately.
    if limiter.check_for(RateLimitClass::NonTrading) {
        // Make that request without calling wait_for again.
    } else {
        // Handle rate limit
    }
}
```

`wait()` and `check()` remain non-trading compatibility methods. A successful
`check_for()` consumes a token; it is not a read-only capacity query. The client
already paces its requests, so callers do not need another limiter around it.

## Budgets Implemented by the Client

| Class | Local pacing | Operations |
|-------|--------------|------------|
| `NonTrading` | Configured requests per period and burst size | Categories, markets, accounts, position/order reads, and other non-trading endpoints |
| `Trading` | One request per second, burst size one | Order and position mutations |
| `Historical` | One request per second, burst size one | Historical price requests |

Non-trading and historical requests also share a process-wide account bucket
of 30 requests per minute with burst size one. Trading does not use that account
bucket. Adding API keys does not multiply the shared account budget. These are
the client's pacing policies; server allowances can still reject a request.

### Configuration Examples

Default non-trading configuration:
```env
IG_RATE_LIMIT_MAX_REQUESTS=4
IG_RATE_LIMIT_PERIOD_SECONDS=12
IG_RATE_LIMIT_BURST_SIZE=3
```

An explicit slower refill rate:
```env
IG_RATE_LIMIT_MAX_REQUESTS=10
IG_RATE_LIMIT_PERIOD_SECONDS=60
IG_RATE_LIMIT_BURST_SIZE=1
```

Changing these settings only changes the configurable non-trading bucket.
It does not widen trading, historical, or shared account budgets.

## Monitoring

The shared HTTP layer logs request method, URL, and rate class at debug level.
Transient retries log `attempt`, `max_retries`, and `delay_ms` at warning level.
Enable a subscriber in the application to inspect these events:

```rust
use tracing_subscriber;

tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

## Best Practices

1. **Set Conservative Limits**: Start with conservative settings and adjust based on your needs
2. **Monitor Your Usage**: Watch for rate limit warnings in logs
3. **Handle Errors Gracefully**: Handle typed failures after the finite internal retry budget; avoid adding unbounded outer retries
4. **Share the Client**: Reuse its request pool and endpoint-class pacing
5. **Test Thoroughly**: Test your application under load to ensure rate limits are respected

## Error Handling

For example, a per-key allowance rejection can contain:

```json
{
    "errorCode": "error.public-api.exceeded-api-key-allowance"
}
```

The client distinguishes allowance failures from transient HTTP failures:

- HTTP 429 and 5xx responses use finite retries. By default, a request has up to
  three retries after its initial attempt. Exhaustion returns the typed failure.
- `MAX_RETRY_COUNT` sets the retry count; zero means one attempt with no retries.
  `RETRY_DELAY_SECS` sets the exponential backoff base, defaulting to 10 seconds.
  Backoff grows as `base * 2^attempt`, capped at 60 seconds before adding up to
  25% jitter. It is not a fixed ten-second delay.
- A 403 per-key allowance response becomes `AppError::ApiKeyAllowanceExceeded`
  without transient backoff. A non-trading client with multiple configured keys
  can try another eligible key; it does not cycle through the pool indefinitely.
- Account, trading, and historical allowance failures return their respective
  `AppError::AccountAllowanceExceeded`, `TradingAllowanceExceeded`, and
  `HistoricalDataAllowanceExceeded` variants without transient backoff.
- Request transport errors propagate as `AppError::Network`. A rejected session
  can trigger one reauthentication and replay on the same key.

The retry count is a per-request transport budget, not a guarantee about the
total number of sends including key selection and session recovery. All sends
continue to pass through the shared pacing controls.

## Advanced Configuration

### Independent Endpoint Classes

A single limiter already separates the endpoint classes:

```rust
use ig_client::application::config::RateLimiterConfig;
use ig_client::application::rate_limiter::{RateLimitClass, RateLimiter};

let limiter = RateLimiter::new(&RateLimiterConfig::default());
let trading_slot_available = limiter.check_for(RateLimitClass::Trading);
let market_slot_available = limiter.check_for(RateLimitClass::NonTrading);
// Each true result reserves one request in that class.
```

### Dynamic Adjustment

A standalone limiter can be rebuilt with different non-trading settings:

```rust
use ig_client::application::config::RateLimiterConfig;
use ig_client::application::rate_limiter::RateLimiter;

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

The new limiter has fresh independent buckets. This does not modify an existing
`Client` or share its request history; recreating limiters per request defeats
pacing.

## See Also

- [Configuration API](../src/application/config.rs)
- [Authentication Guide](./AUTHENTICATION.md)
- [IG Markets API Documentation](https://labs.ig.com/rest-trading-api-reference)
