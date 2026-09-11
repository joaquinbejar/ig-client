//! Constructing a `Client` from configuration the embedding application owns.
//!
//! `Client::try_new()` is the convenience path: it loads a local `.env` file and
//! reads the global `IG_*` environment namespace. An embedding application that
//! keeps credentials in its own namespaced variables — and must not have the
//! crate reach for globals or a `.env` file — uses `Config::from_credentials`
//! plus `Client::with_config` instead. Neither touches `dotenvy` nor `IG_*`.
//!
//! Run with the embedder's own variables set:
//!
//! ```bash
//! MYAPP_IG_USERNAME=... MYAPP_IG_PASSWORD=... MYAPP_IG_ACCOUNT_ID=... \
//! MYAPP_IG_API_KEY=... cargo run -p examples_simples --bin client_with_config
//! ```

use ig_client::prelude::*;
use std::env;
use tracing::info;

/// Reads a variable from the embedder's own namespace.
fn required_var(name: &str) -> Result<String, AppError> {
    env::var(name).map_err(|_| AppError::InvalidInput(format!("{name} is not set")))
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    setup_logger();

    // Credentials come from the embedder's namespace, never from `IG_*`.
    let credentials = Credentials::new(
        required_var("MYAPP_IG_USERNAME")?,
        required_var("MYAPP_IG_PASSWORD")?,
        required_var("MYAPP_IG_ACCOUNT_ID")?,
        required_var("MYAPP_IG_API_KEY")?,
    );

    // `from_credentials` reads no environment variable and loads no `.env`
    // file; struct update overrides only the sections we care about.
    let config = Config {
        rest_api: RestApiConfig {
            base_url: env::var("MYAPP_IG_REST_BASE_URL")
                .unwrap_or_else(|_| RestApiConfig::default().base_url),
            timeout: 30,
        },
        ..Config::from_credentials(credentials)
    };

    let client = Client::with_config(config)?;

    // `Config`'s Display redacts credentials, so this logs no secrets.
    info!(
        "client built from injected config, REST base URL: {}",
        client.config().rest_api.base_url
    );

    // Session login happens transparently on the first API call.
    let accounts = client.get_accounts().await?;
    info!("{} account(s) available", accounts.accounts.len());

    Ok(())
}
