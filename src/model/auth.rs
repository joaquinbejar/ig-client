/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 19/10/25
******************************************************************************/
use crate::application::auth::Session;
use crate::constants::V2_SESSION_LIFETIME_SECS;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::warn;

/// Reports whether the effective expiry (`created_at + lifetime - margin`) has
/// been reached as of `now`, using checked `chrono` arithmetic throughout.
///
/// Timestamp math here is on protocol-state (session expiry), so overflow is
/// never wrapped or silently truncated: any arithmetic overflow fails safe by
/// reporting the token as expired (`true`), which triggers a refresh rather than
/// trusting a bogus far-future expiry.
#[must_use]
#[inline]
fn is_past_effective_expiry(
    created_at: chrono::DateTime<Utc>,
    lifetime_secs: i64,
    margin_secs: i64,
    now: chrono::DateTime<Utc>,
) -> bool {
    let effective = chrono::Duration::try_seconds(lifetime_secs)
        .and_then(|lifetime| created_at.checked_add_signed(lifetime))
        .and_then(|expiry| {
            chrono::Duration::try_seconds(margin_secs).and_then(|m| expiry.checked_sub_signed(m))
        });
    match effective {
        Some(effective_expiry) => effective_expiry <= now,
        // Overflow computing the effective expiry: treat as expired.
        None => true,
    }
}

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
                // Derive expiry from the token's own creation time and lifetime,
                // staying consistent with `V2Response::is_expired` (which uses
                // `created_at + expires_in`). Falls back to the full v2 lifetime
                // when `expires_in` was not set on the response. Saturating math
                // keeps a far-future / pre-epoch `created_at` from underflowing.
                let lifetime = v.expires_in.unwrap_or(V2_SESSION_LIFETIME_SECS);
                let created = u64::try_from(v.created_at.timestamp()).unwrap_or(0);
                let expires_at = created.saturating_add(lifetime);
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
                v.expires_in = Some(V2_SESSION_LIFETIME_SECS);
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
    /// Parses the IG `expires_in` field (seconds, delivered as a JSON string)
    /// into an integer count of seconds.
    ///
    /// On a malformed value this emits a single `WARN` (with the offending
    /// value, never the token itself) and falls back to `0`, which makes the
    /// token read as already expired and forces a refresh — an observable,
    /// safe degradation rather than a silent one.
    #[must_use]
    #[inline]
    fn expires_in_secs(&self) -> i64 {
        self.expires_in.parse::<i64>().unwrap_or_else(|_| {
            warn!(
                expires_in = %self.expires_in,
                "malformed expires_in; treating token as expired"
            );
            0
        })
    }

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
        let margin = i64::try_from(margin_seconds).unwrap_or(i64::MAX);
        is_past_effective_expiry(self.created_at, self.expires_in_secs(), margin, Utc::now())
    }

    /// Returns the Unix timestamp when the token expires (considering the margin)
    ///
    /// # Arguments
    /// * `margin_seconds` - Safety margin in seconds before actual expiry
    ///
    /// # Returns
    /// Unix timestamp (seconds since epoch) when the token should be considered
    /// expired. Saturates to `0` (already expired) if the computed expiry is
    /// before the Unix epoch or the arithmetic overflows.
    #[must_use]
    pub fn expire_at(&self, margin_seconds: i64) -> u64 {
        let effective = chrono::Duration::try_seconds(self.expires_in_secs())
            .and_then(|lifetime| self.created_at.checked_add_signed(lifetime))
            .and_then(|expiry| {
                chrono::Duration::try_seconds(margin_seconds)
                    .and_then(|m| expiry.checked_sub_signed(m))
            });

        match effective {
            Some(effective_expiry) => u64::try_from(effective_expiry.timestamp()).unwrap_or(0),
            None => 0,
        }
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
                let lifetime = i64::try_from(expires_in).unwrap_or(i64::MAX);
                let margin = i64::try_from(margin_seconds).unwrap_or(i64::MAX);
                is_past_effective_expiry(self.created_at, lifetime, margin, Utc::now())
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

#[cfg(test)]
mod expiry_tests {
    use super::*;

    fn oauth_token(expires_in: &str) -> OAuthToken {
        OAuthToken {
            access_token: "ACCESS".to_string(),
            refresh_token: "REFRESH".to_string(),
            scope: "read write".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: expires_in.to_string(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_oauth_token_is_expired_malformed_expires_in_treated_expired() {
        // A non-numeric `expires_in` must not panic; it falls back to 0 seconds
        // of lifetime, so the token reads as already expired.
        let token = oauth_token("not-a-number");
        assert!(token.is_expired(60));
    }

    #[test]
    fn test_oauth_token_is_expired_fresh_token_not_expired() {
        // A freshly created 3600s token is well within its lifetime.
        let token = oauth_token("3600");
        assert!(!token.is_expired(60));
    }

    #[test]
    fn test_oauth_token_expire_at_malformed_expires_in_saturates() {
        // Malformed lifetime -> effective expiry at (created_at - margin), which
        // is in the past; expire_at must not underflow into a huge u64.
        let token = oauth_token("garbage");
        let now = Utc::now().timestamp();
        let expires_at = token.expire_at(1);
        // Already-expired: the effective expiry is at or before "now".
        assert!(expires_at <= u64::try_from(now).unwrap_or(0));
    }

    fn v2_response(expires_in: Option<u64>) -> V2Response {
        V2Response {
            account_type: "CFD".to_string(),
            account_info: AccountInfo {
                balance: 0.0,
                deposit: 0.0,
                profit_loss: 0.0,
                available: 0.0,
            },
            currency_iso_code: "EUR".to_string(),
            currency_symbol: "E".to_string(),
            current_account_id: "ACC123".to_string(),
            lightstreamer_endpoint: "https://demo-apd.marketdatasystems.com".to_string(),
            accounts: Vec::new(),
            client_id: "CLIENT1".to_string(),
            timezone_offset: 1,
            has_active_demo_accounts: true,
            has_active_live_accounts: false,
            trailing_stops_enabled: false,
            rerouting_environment: None,
            dealing_enabled: true,
            security_headers: None,
            expires_in,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_v2_session_expires_at_derived_from_created_at_plus_lifetime() {
        // `expires_in` unset -> the full v2 lifetime is used, and expiry is
        // derived from `created_at`, not the call-time clock.
        let resp = v2_response(None);
        let created = u64::try_from(resp.created_at.timestamp()).unwrap_or(0);
        let session = SessionResponse::V2(resp).get_session();
        assert_eq!(session.expires_at, created + V2_SESSION_LIFETIME_SECS);
    }

    #[test]
    fn test_v2_response_is_expired_none_expires_in_treated_expired() {
        // No `expires_in` recorded -> fail safe as expired.
        let resp = v2_response(None);
        assert!(resp.is_expired(300));
    }

    #[test]
    fn test_v2_response_is_expired_fresh_lifetime_not_expired() {
        let resp = v2_response(Some(V2_SESSION_LIFETIME_SECS));
        assert!(!resp.is_expired(300));
    }
}
