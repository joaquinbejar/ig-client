/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 19/10/25
******************************************************************************/
use crate::application::auth::Session;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::warn;

/// Response from session creation endpoint
///
/// This enum handles both API v2 and v3 session responses using serde's untagged feature
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SessionResponse {
    /// API v3 session response with OAuth tokens
    V3(V3Response),
    /// API v2 session response with CST/X-SECURITY-TOKEN
    V2(V2Response),
}

impl SessionResponse {
    /// Checks if this is a v3 session response
    #[must_use]
    #[inline]
    pub fn is_v3(&self) -> bool {
        matches!(self, SessionResponse::V3(_))
    }

    /// Checks if this is a v2 session response
    #[must_use]
    #[inline]
    pub fn is_v2(&self) -> bool {
        matches!(self, SessionResponse::V2(_))
    }

    /// Converts the response to a Session object
    #[must_use]
    pub fn get_session(&self) -> Session {
        match self {
            SessionResponse::V3(v) => Session {
                account_id: v.account_id.clone(),
                client_id: v.client_id.clone(),
                lightstreamer_endpoint: v.lightstreamer_endpoint.clone(),
                cst: None,
                x_security_token: None,
                oauth_token: Some(v.oauth_token.clone()),
                api_version: 3,
                expires_at: v.oauth_token.expire_at(1),
            },
            SessionResponse::V2(v) => {
                let (cst, x_security_token) = match v.security_headers.as_ref() {
                    Some(headers) => (
                        Some(headers.cst.clone()),
                        Some(headers.x_security_token.clone()),
                    ),
                    None => (None, None),
                };
                let expires_at = (Utc::now().timestamp() + (3600 * 6)) as u64; // 6 hours from now
                Session {
                    account_id: v.current_account_id.clone(),
                    client_id: v.client_id.clone(),
                    lightstreamer_endpoint: v.lightstreamer_endpoint.clone(),
                    cst,
                    x_security_token,
                    oauth_token: None,
                    api_version: 2,
                    expires_at,
                }
            }
        }
    }
    /// Converts the response to a Session object using v2 security headers
    ///
    /// # Arguments
    /// * `headers` - Security headers (CST and X-SECURITY-TOKEN)
    pub fn get_session_v2(&mut self, headers: &SecurityHeaders) -> Session {
        match self {
            SessionResponse::V3(_) => {
                warn!("Returing V3 session from V2 headers - this may be unexpected");
                self.get_session()
            }
            SessionResponse::V2(v) => {
                v.set_security_headers(headers);
                v.expires_in = Some(21600); // 6 hours
                self.get_session()
            }
        }
    }

    /// Checks if the session is expired
    ///
    /// # Arguments
    /// * `margin_seconds` - Safety margin in seconds before actual expiration
    #[must_use]
    #[inline]
    pub fn is_expired(&self, margin_seconds: u64) -> bool {
        match self {
            SessionResponse::V3(v) => v.oauth_token.is_expired(margin_seconds),
            SessionResponse::V2(v) => v.is_expired(margin_seconds),
        }
    }
}

/// API v3 session response
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V3Response {
    /// Client identifier
    pub client_id: String,
    /// Account identifier
    pub account_id: String,
    /// Timezone offset in minutes
    pub timezone_offset: i32,
    /// Lightstreamer WebSocket endpoint URL
    pub lightstreamer_endpoint: String,
    /// OAuth token information
    pub oauth_token: OAuthToken,
}

/// OAuth token information returned by API v3
///
/// `Deserialize` / `Serialize` behaviour is unchanged so real IG payloads still
/// round-trip; only the `Debug` representation is overridden to redact the
/// `access_token` and `refresh_token`.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct OAuthToken {
    /// OAuth access token
    pub access_token: String,
    /// OAuth refresh token
    pub refresh_token: String,
    /// Token scope
    pub scope: String,
    /// Token type (typically "Bearer")
    pub token_type: String,
    /// Token expiry time in seconds
    pub expires_in: String,
    /// Timestamp when this token was created (for expiry calculation)
    #[serde(skip, default = "chrono::Utc::now")]
    pub created_at: chrono::DateTime<Utc>,
}

// Manual redacting `Debug` — `access_token` / `refresh_token` are credentials
// and must never appear in logs, panics or `Debug` output.
impl fmt::Debug for OAuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthToken")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("scope", &self.scope)
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl OAuthToken {
    /// Checks if the OAuth token is expired or will expire soon
    ///
    /// # Arguments
    /// * `margin_seconds` - Safety margin in seconds before actual expiry
    ///
    /// # Returns
    /// `true` if the token is expired or will expire within the margin, `false` otherwise
    #[must_use]
    #[inline]
    pub fn is_expired(&self, margin_seconds: u64) -> bool {
        let expires_in_secs = self.expires_in.parse::<i64>().unwrap_or(0);
        let expiry_time = self.created_at + chrono::Duration::seconds(expires_in_secs);
        let now = Utc::now();
        let margin = chrono::Duration::seconds(margin_seconds as i64);

        expiry_time - margin <= now
    }

    /// Returns the Unix timestamp when the token expires (considering the margin)
    ///
    /// # Arguments
    /// * `margin_seconds` - Safety margin in seconds before actual expiry
    ///
    /// # Returns
    /// Unix timestamp (seconds since epoch) when the token should be considered expired
    #[must_use]
    pub fn expire_at(&self, margin_seconds: i64) -> u64 {
        let expires_in_secs = self.expires_in.parse::<i64>().unwrap_or(0);
        let expiry_time = self.created_at + chrono::Duration::seconds(expires_in_secs);
        let margin = chrono::Duration::seconds(margin_seconds);

        // Subtract margin to get the "effective" expiry time
        let effective_expiry = expiry_time - margin;

        effective_expiry.timestamp() as u64
    }
}

/// API v2 session response
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2Response {
    /// Account type (e.g., "CFD", "SPREADBET")
    pub account_type: String,
    /// Account information
    pub account_info: AccountInfo,
    /// Currency ISO code (e.g., "GBP", "USD")
    pub currency_iso_code: String,
    /// Currency symbol (e.g., "£", "$")
    pub currency_symbol: String,
    /// Current active account ID
    pub current_account_id: String,
    /// Lightstreamer WebSocket endpoint URL
    pub lightstreamer_endpoint: String,
    /// List of all accounts owned by the user
    pub accounts: Vec<Account>,
    /// Client identifier
    pub client_id: String,
    /// Timezone offset in minutes
    pub timezone_offset: i32,
    /// Whether user has active demo accounts
    pub has_active_demo_accounts: bool,
    /// Whether user has active live accounts
    pub has_active_live_accounts: bool,
    /// Whether trailing stops are enabled
    pub trailing_stops_enabled: bool,
    /// Rerouting environment if applicable
    pub rerouting_environment: Option<String>,
    /// Whether dealing is enabled
    pub dealing_enabled: bool,
    /// Security headers (CST and X-SECURITY-TOKEN)
    #[serde(skip)]
    pub security_headers: Option<SecurityHeaders>,
    /// Token expiry time in seconds
    #[serde(skip)]
    pub expires_in: Option<u64>,
    /// Timestamp when this token was created (for expiry calculation)
    #[serde(skip, default = "chrono::Utc::now")]
    pub created_at: chrono::DateTime<Utc>,
}

impl V2Response {
    /// Sets the security headers for this session
    ///
    /// # Arguments
    /// * `headers` - Security headers containing CST and X-SECURITY-TOKEN
    pub fn set_security_headers(&mut self, headers: &SecurityHeaders) {
        self.security_headers = Some(headers.clone());
    }

    /// Checks if the session is expired
    ///
    /// # Arguments
    /// * `margin_seconds` - Safety margin in seconds before actual expiration
    ///
    /// # Returns
    /// `true` if the session is expired or `expires_in` was never set
    #[must_use]
    pub fn is_expired(&self, margin_seconds: u64) -> bool {
        match self.expires_in {
            Some(expires_in) => {
                let expiry_time = self.created_at + chrono::Duration::seconds(expires_in as i64);
                let now = Utc::now();
                let margin = chrono::Duration::seconds(margin_seconds as i64);
                expiry_time - margin <= now
            }
            // If expires_in was never set, treat as expired for safety
            None => true,
        }
    }
}

/// Security headers for API v2 authentication
#[derive(Clone, Deserialize, Serialize)]
pub struct SecurityHeaders {
    /// Client Session Token
    pub cst: String,
    /// Security token for request authentication
    pub x_security_token: String,
    /// API key for the application
    pub x_ig_api_key: String,
}

// Manual redacting `Debug` — every field here (CST, X-SECURITY-TOKEN and the
// API key) is a credential, so all three are masked.
impl fmt::Debug for SecurityHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecurityHeaders")
            .field("cst", &"<redacted>")
            .field("x_security_token", &"<redacted>")
            .field("x_ig_api_key", &"<redacted>")
            .finish()
    }
}

/// Account balance information
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    /// Total account balance
    pub balance: f64,
    /// Amount deposited
    pub deposit: f64,
    /// Current profit or loss
    pub profit_loss: f64,
    /// Available funds for trading
    pub available: f64,
}

/// Trading account information
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Unique account identifier
    pub account_id: String,
    /// Human-readable account name
    pub account_name: String,
    /// Whether this is the preferred/default account
    pub preferred: bool,
    /// Account type (e.g., "CFD", "SPREADBET")
    pub account_type: String,
}

#[cfg(test)]
mod redaction_tests {
    use super::*;

    #[test]
    fn test_oauth_token_debug_redacts_tokens() {
        let token = OAuthToken {
            access_token: "SECRET-ACCESS-VALUE".to_string(),
            refresh_token: "SECRET-REFRESH-VALUE".to_string(),
            scope: "read write".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: "60".to_string(),
            created_at: Utc::now(),
        };
        let rendered = format!("{token:?}");

        assert!(!rendered.contains("SECRET-ACCESS-VALUE"));
        assert!(!rendered.contains("SECRET-REFRESH-VALUE"));
        assert!(rendered.contains("<redacted>"));
        // Non-secret fields stay visible.
        assert!(rendered.contains("Bearer"));
        assert!(rendered.contains("read write"));
    }

    #[test]
    fn test_security_headers_debug_redacts_tokens() {
        let headers = SecurityHeaders {
            cst: "SECRET-CST-VALUE".to_string(),
            x_security_token: "SECRET-XST-VALUE".to_string(),
            x_ig_api_key: "SECRET-API-KEY-VALUE".to_string(),
        };
        let rendered = format!("{headers:?}");

        assert!(!rendered.contains("SECRET-CST-VALUE"));
        assert!(!rendered.contains("SECRET-XST-VALUE"));
        assert!(!rendered.contains("SECRET-API-KEY-VALUE"));
        assert!(rendered.contains("<redacted>"));
    }
}
