# Running the examples

The 13 example packages provide 58 binary targets. Run them from the repository
root with `cargo run -p PACKAGE --bin BINARY -- ARGUMENTS`. They are workspace
members, so use `--bin`, rather than Cargo's standalone `--example` option.

Most examples use `Client::try_new()` and the `IG_*` environment variables or a
local `.env` file. The default REST gateway is IG demo; `IG_REST_BASE_URL` selects
another endpoint. The `client_with_config` example instead requires
`MYAPP_IG_USERNAME`, `MYAPP_IG_PASSWORD`, `MYAPP_IG_ACCOUNT_ID`, and
`MYAPP_IG_API_KEY`. EPICs and deal IDs must be accessible to that account; fixed
instruments and historical date ranges in individual demos may need changing.

See [market examples](market/README.md) for complete catalogue enumeration,
one-page category inspection, detail exports, expiry semantics, and the changed
flat catalogue file consumed by filtered storage.

## Updated commands and behavior

```sh
cargo run -p examples_simples --bin client_with_config
cargo run -p examples_simples --bin simple_rate_limiter -- EPIC
cargo run -p examples_positions --bin positions_switch_account -- ACCOUNT_ID
cargo run -p examples_positions --bin positions_switch_account_v2 -- ACCOUNT_ID
cargo run -p examples_other --bin activity_debug
```

`simple_rate_limiter` requires an explicit EPIC and demonstrates finite retries.
The account-switch examples require a target account, and use the same configured
client to switch and query positions. `activity_debug` retrieves one decoded
activity page using the shared authentication path, including OAuth v3; set
`RUST_LOG=debug` to display the decoded response. `tx_loop` fetches the already
aggregated transaction history once per scheduled tick and stores it in
PostgreSQL.

The working-order demonstration takes explicit order parameters:

```sh
cargo run -p examples_orders --bin workingorders_example -- EPIC LIMIT_LEVEL SIZE
```

It submits a buy limit order, confirms its reference and EPIC, and targets only
that confirmed deal ID for cancellation. If the created order is no longer
working, it stops without selecting another account order. After submitting
cancellation it performs a bounded check that this order leaves the working list;
absence does not distinguish cancellation from execution or expiry. The order
can execute before cancellation.

Orders, position-closing, watchlist CRUD, preferences, and account-switch examples
change the configured account. Storage examples require PostgreSQL and write to
it. Streaming examples require a working Lightstreamer session. Run these only
when their described operations are intended.

## Local verification

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --workspace --bins
cargo test -p examples_market -p examples_orders -p examples_other -p examples_positions -p examples_simples
```

The new regression tests use synthetic in-memory fixtures for detail identity and
expiry association, page arguments, Unicode display, and selecting the created
working order among unrelated orders. Test harnesses do not execute the example
`main` functions. Compilation and fixture tests do not establish end-to-end
success against IG, Lightstreamer, or PostgreSQL.
