[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Crates.io](https://img.shields.io/crates/v/ig-client.svg)](https://crates.io/crates/ig-client)
[![Downloads](https://img.shields.io/crates/d/ig-client.svg)](https://crates.io/crates/ig-client)
[![Stars](https://img.shields.io/github/stars/joaquinbejar/ig-client.svg)](https://github.com/joaquinbejar/ig-client/stargazers)
[![Issues](https://img.shields.io/github/issues/joaquinbejar/ig-client.svg)](https://github.com/joaquinbejar/ig-client/issues)
[![PRs](https://img.shields.io/github/issues-pr/joaquinbejar/ig-client.svg)](https://github.com/joaquinbejar/ig-client/pulls)
[![Build Status](https://img.shields.io/github/workflow/status/joaquinbejar/ig-client/CI)](https://github.com/joaquinbejar/ig-client/actions)
[![Coverage](https://img.shields.io/codecov/c/github/joaquinbejar/ig-client)](https://codecov.io/gh/joaquinbejar/ig-client)
[![Dependencies](https://img.shields.io/librariesio/github/joaquinbejar/ig-client)](https://libraries.io/github/joaquinbejar/ig-client)
[![Documentation](https://img.shields.io/badge/docs-latest-blue.svg)](https://docs.rs/ig-client)
[![Wiki](https://img.shields.io/badge/wiki-latest-blue.svg)](https://deepwiki.com/joaquinbejar/ig-client)

{{readme}}

## What's New in 0.12.3

- The crate now has Cargo **features**, both on by default so existing users are
  unaffected ([#83](https://github.com/joaquinbejar/ig-client/issues/83)):
  - `streaming` — the Lightstreamer leg (`StreamerClient`,
    `DynamicMarketStreamer`, `Listener`, the `ItemUpdate` adapters), pulling in
    `lightstreamer-rs`.
  - `persistence` — the `storage` module, pulling in `sqlx`.
- `ig-client = { version = "0.12.3", default-features = false }` gives a
  REST/poll-only client with **neither** crate in the dependency graph. This
  matters for licensing: `lightstreamer-rs` is GPL-3.0-only, so a permissively
  licensed consumer with a copyleft-rejecting `cargo deny` policy previously
  could not depend on `ig-client` at all.
- Fixed two latent dependencies on features that only arrived transitively:
  - Two modules imported `tracing::log::debug`, which resolved only because
    `sqlx` enabled `tracing`'s `log` feature. These are now native `tracing`
    events. **Behaviour change:** `tracing::log::debug` emitted a `log` record,
    and `setup_logger` installs no `log` bridge, so those two `.env`-loading
    messages were previously discarded — they now appear at DEBUG. Relatedly,
    the streaming listener's per-tick payload line moved from DEBUG to TRACE:
    it fires on every tick and renders account payloads (P&L, equity, margin).
  - `tokio`'s `sync` feature is now declared explicitly. The crate uses
    `tokio::sync` on the REST path but was receiving the feature only through
    `reqwest`'s own dependencies.

## What's New in 0.12.2

- `Client::with_config(config)` builds a client from a caller-supplied
  `Config`, reading no environment variable and loading no `.env` file — the
  injection path for applications that own their configuration source.
  `Client::try_new()` is unchanged and remains the `.env` / `IG_*` convenience
  path ([#81](https://github.com/joaquinbejar/ig-client/issues/81)).
- `Config::from_credentials(credentials)` and `Credentials::new(..)` build a
  configuration entirely from caller-supplied values, with `Default` impls on
  `RestApiConfig`, `WebSocketConfig`, `RateLimiterConfig` and `DatabaseConfig`
  supplying the non-credential defaults (the same values `Config::new()` falls
  back to).
- `Client::config()` / `HttpClient::config()` expose the effective
  configuration (secrets stay redacted in `Debug` / `Display`).

## What's New in 0.12.1

- The `historical_prices` unique-constraint migration now tolerates
  PostgreSQL SQLSTATE `42P07` ("relation already exists") when an index with
  the constraint's name already exists without an attached constraint —
  previously this failed application startup on every run ([#79](https://github.com/joaquinbejar/ig-client/issues/79)).

## What's New in 0.12.0

A large correctness, safety, and API-consistency release. Highlights:

### Security & reliability
- Credentials, session tokens (CST / X-SECURITY-TOKEN / OAuth) and the DB
  connection URL are redacted from `Debug`/`Display` and never logged.
- Retries are finite by default with exponential backoff + jitter; HTTP 429
  is retried; unbounded retry loops are gone.
- The rate limiter honours the configured `max_requests` and separates the
  trading, non-trading and historical budgets.
- Token expiry/refresh is consistent (single refresh-and-replay on 401); the
  streaming connection no longer holds a lock across its lifetime and every
  spawned task has a shutdown path.

### Correctness
- Order size rounds to the nearest tick (no more `0.29 → 0.28`); P&L math is
  unified and correct when a market price is missing; position netting takes
  the larger side's direction.
- Storage: the unique-constraint migration actually runs, empty-epic stats no
  longer panic, `instrument_type` is stored unquoted, and historical prices
  persist in UTC (`snapshotTimeUTC`).
- DTOs capture previously-dropped IG fields (`affectedDeals`/`profit` on
  confirms, `EXECUTE_AND_ELIMINATE`, OPU stop/limit/trailing fields); IG
  numeric fields are `i64`/unsigned, not `i32`.

### API & structure (breaking — 0.12.0)
- Constructors are fallible and panic-free: use `Client::try_new()`,
  `Auth::try_new()`, `HttpClient::new_lazy()` (`new()`/`Default` removed).
- Module boundaries restored: `model`/`presentation` are pure DTO layers;
  `HttpClient` and the streaming adapters live in `application`.
- Typed errors are wired up (`AuthError`, deserialization context with the
  auth-response body redacted). The prelude now exports the streaming API and
  all service traits.

### Testing
- Offline test coverage for the auth flow, HTTP retry/status mapping,
  streaming lifecycle, and serde round-trips (via a `wiremock` dev-dependency).

## Contribution 

We welcome contributions to this project! If you would like to contribute, please follow these steps:

1. Fork the repository.
2. Create a new branch for your feature or bug fix.
3. Make your changes and ensure that the project still builds and all tests pass.
4. Commit your changes and push your branch to your forked repository.
5. Submit a pull request to the main repository.

### **Contact Information**

If you have any questions, issues, or would like to provide feedback, please feel free to contact the project maintainer:

- **Author**: Joaquín Béjar García
- **Email**: jb@taunais.com
- **Telegram**: [@joaquin_bejar](https://t.me/joaquin_bejar)
- **Repository**: <https://github.com/joaquinbejar/ig-client>
- **Documentation**: <https://docs.rs/ig-client>

We appreciate your interest and look forward to your contributions!

## ✍️ License

Licensed under MIT license

## Disclaimer

This software is not officially associated with IG Markets. Trading financial instruments carries risk, and this library is provided as-is without any guarantees. Always test thoroughly with a demo account before using in a live trading environment.