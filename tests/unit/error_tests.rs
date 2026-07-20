use ig_client::error::{AppError, AuthError};
use reqwest::StatusCode;
use serde_json::Error as JsonError;
#[cfg(feature = "persistence")]
use sqlx::Error as SqlxError;
use std::error::Error;
use std::fmt::{self, Display};
use std::io::{Error as IoError, ErrorKind};

// Custom error type for testing
#[derive(Debug)]
struct TestError(String);

impl Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TestError: {}", self.0)
    }
}

impl Error for TestError {}

// Helper function to test Display implementation
fn assert_display_contains<T: Display>(value: &T, expected: &str) {
    let display_string = value.to_string();
    assert!(
        display_string.contains(expected),
        "Expected '{display_string}' to contain '{expected}', but it didn't"
    );
}

#[test]
fn test_app_error_from_io_error() {
    let io_error = IoError::new(ErrorKind::NotFound, "file not found");
    let app_error = AppError::from(io_error);

    match app_error {
        AppError::Io(_) => {
            // Test passed
        }
        _ => panic!("Expected AppError::Io, got {app_error:?}"),
    }

    assert_display_contains(&app_error, "io error");
}

#[test]
fn test_app_error_from_serde_json_error() {
    // Create a JSON error
    let json_str = r#"{"invalid": json"#;
    let json_error: JsonError = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();

    let app_error = AppError::from(json_error);

    match app_error {
        AppError::Json(_) => {
            // Test passed
        }
        _ => panic!("Expected AppError::Json, got {app_error:?}"),
    }

    assert_display_contains(&app_error, "json error");
}

#[test]
#[cfg(feature = "persistence")]
fn test_app_error_from_sqlx_error() {
    // Create a SqlxError (using a simple variant since we can't easily create a real one)
    let sqlx_error = SqlxError::RowNotFound;

    let app_error = AppError::from(sqlx_error);

    match app_error {
        AppError::Db(_) => {
            // Test passed
        }
        _ => panic!("Expected AppError::Db, got {app_error:?}"),
    }

    assert_display_contains(&app_error, "db error");
}

#[test]
fn test_app_error_from_auth_error() {
    // An AuthError is wrapped (not flattened) into AppError::Auth, preserving
    // both the specific auth variant and its contextual message.
    let auth_error = AuthError::BadCredentials;
    let app_error = AppError::from(auth_error);

    match app_error {
        AppError::Auth(AuthError::BadCredentials) => {
            // Test passed
        }
        _ => panic!("Expected AppError::Auth(BadCredentials), got {app_error:?}"),
    }

    // The wrapped auth message is carried through the AppError Display.
    assert_display_contains(&app_error, "auth error");
    assert_display_contains(&app_error, "bad credentials");

    // Test another variant
    let auth_error = AuthError::Unexpected(StatusCode::INTERNAL_SERVER_ERROR);
    let app_error = AppError::from(auth_error);

    match app_error {
        AppError::Auth(AuthError::Unexpected(_)) => {
            // Test passed
        }
        _ => panic!("Expected AppError::Auth(Unexpected), got {app_error:?}"),
    }
}

#[test]
fn test_app_error_from_missing_session_token_names_header() {
    // The concrete wiring that makes AuthError non-dead: a missing session
    // header maps to a typed auth error whose message names the header, and
    // survives conversion into AppError.
    let auth_error = AuthError::MissingSessionToken("cst".to_string());
    assert_display_contains(&auth_error, "missing cst header in login response");

    let app_error = AppError::from(auth_error);
    match &app_error {
        AppError::Auth(AuthError::MissingSessionToken(header)) => assert_eq!(header, "cst"),
        _ => panic!("Expected AppError::Auth(MissingSessionToken), got {app_error:?}"),
    }
    assert_display_contains(&app_error, "missing cst header in login response");
}

#[test]
fn test_auth_error_from_app_error_auth_round_trips() {
    // AppError::Auth unwraps back to the inner AuthError rather than being
    // re-stringified into AuthError::Other.
    let app_error = AppError::Auth(AuthError::MissingSessionToken(
        "x-security-token".to_string(),
    ));
    let auth_error = AuthError::from(app_error);
    match auth_error {
        AuthError::MissingSessionToken(header) => assert_eq!(header, "x-security-token"),
        _ => panic!("Expected AuthError::MissingSessionToken, got {auth_error:?}"),
    }
}

#[test]
fn test_app_error_unauthorized() {
    let app_error = AppError::Unauthorized;
    assert_display_contains(&app_error, "unauthorized");
}

#[test]
fn test_app_error_not_found() {
    let app_error = AppError::NotFound;
    assert_display_contains(&app_error, "not found");
}

#[test]
fn test_app_error_rate_limit_exceeded() {
    let app_error = AppError::RateLimitExceeded;
    assert_display_contains(&app_error, "rate limit exceeded");
}

#[test]
fn test_app_error_serialization_error() {
    let app_error = AppError::SerializationError("test error".to_string());
    assert_display_contains(&app_error, "serialization error");
    assert_display_contains(&app_error, "test error");
}

#[test]
fn test_app_error_websocket_error() {
    let app_error = AppError::WebSocketError("connection closed".to_string());
    assert_display_contains(&app_error, "websocket error");
    assert_display_contains(&app_error, "connection closed");
}

#[test]
fn test_app_error_unexpected() {
    let app_error = AppError::Unexpected(StatusCode::BAD_REQUEST);
    assert_display_contains(&app_error, "unexpected http status");
    assert_display_contains(&app_error, "400");
}

#[test]
fn test_app_error_invalid_input() {
    let app_error = AppError::InvalidInput("invalid parameter".to_string());
    assert_display_contains(&app_error, "invalid input");
    assert_display_contains(&app_error, "invalid parameter");
}

#[test]
fn test_app_error_deserialization() {
    let app_error = AppError::Deserialization("failed to deserialize".to_string());
    assert_display_contains(&app_error, "deserialization error");
    assert_display_contains(&app_error, "failed to deserialize");
}

#[test]
fn test_app_error_from_box_dyn_error() {
    // Create a Box<dyn Error> containing an IoError
    let io_error = IoError::new(ErrorKind::NotFound, "file not found");
    let boxed_error: Box<dyn Error> = Box::new(io_error);

    // Convert to AppError
    let app_error = AppError::from(boxed_error);

    match app_error {
        AppError::Io(_) => {
            // Test passed
        }
        _ => panic!("Expected AppError::Io, got {app_error:?}"),
    }

    // Create a Box<dyn Error> containing a JsonError
    let json_str = r#"{"invalid": json"#;
    let json_error: JsonError = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
    let boxed_error: Box<dyn Error> = Box::new(json_error);

    // Convert to AppError
    let app_error = AppError::from(boxed_error);

    match app_error {
        AppError::Json(_) => {
            // Test passed
        }
        _ => panic!("Expected AppError::Json, got {app_error:?}"),
    }

    // Create a Box<dyn Error> containing a different error type
    let boxed_error: Box<dyn Error> = Box::new(TestError("test error".to_string()));

    // Convert to AppError - the fallback now PRESERVES the original message via
    // Generic instead of fabricating a fake "unexpected http status: 500".
    let app_error = AppError::from(boxed_error);

    match &app_error {
        AppError::Generic(_) => {
            // Test passed
        }
        _ => panic!("Expected AppError::Generic, got {app_error:?}"),
    }
    // The original error text is not discarded.
    assert_display_contains(&app_error, "test error");
}

#[test]
fn test_auth_error_display() {
    let auth_error = AuthError::BadCredentials;
    assert_display_contains(&auth_error, "bad credentials");

    // We create a reqwest error indirectly since from_static is not available
    let auth_error = AuthError::Other("network error".to_string());
    assert_display_contains(&auth_error, "network error");

    let auth_error = AuthError::Other("custom error".to_string());
    assert_display_contains(&auth_error, "other error");
    assert_display_contains(&auth_error, "custom error");
}

#[test]
fn test_auth_error_from_box_dyn_error() {
    // Create a Box<dyn Error> containing an IoError
    let io_error = IoError::new(ErrorKind::NotFound, "file not found");
    let boxed_error: Box<dyn Error> = Box::new(io_error);

    // Convert to AuthError
    let auth_error = AuthError::from(boxed_error);

    match auth_error {
        AuthError::Io(_) => {
            // Test passed
        }
        _ => panic!("Expected AuthError::Io, got {auth_error:?}"),
    }

    // Create a Box<dyn Error> containing a JsonError
    let json_str = r#"{"invalid": json"#;
    let json_error: JsonError = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
    let boxed_error: Box<dyn Error> = Box::new(json_error);

    // Convert to AuthError
    let auth_error = AuthError::from(boxed_error);

    match auth_error {
        AuthError::Json(_) => {
            // Test passed
        }
        _ => panic!("Expected AuthError::Json, got {auth_error:?}"),
    }

    // Create a Box<dyn Error> containing a different error type
    let boxed_error: Box<dyn Error> = Box::new(TestError("test error".to_string()));

    // Convert to AuthError - should be Other
    let auth_error = AuthError::from(boxed_error);

    match auth_error {
        AuthError::Other(_) => {
            // Test passed
        }
        _ => panic!("Expected AuthError::Other, got {auth_error:?}"),
    }
}

#[test]
fn test_auth_error_from_box_dyn_error_send_sync() {
    // We can't easily create a Box<dyn Error + Send + Sync> with reqwest::Error
    // since it would require an actual HTTP request, but we can test the other paths

    // Create a Box<dyn Error + Send + Sync> containing an IoError
    let io_error = IoError::new(ErrorKind::NotFound, "file not found");
    let boxed_error: Box<dyn Error + Send + Sync> = Box::new(io_error);

    // Convert to AuthError
    let auth_error = AuthError::from(boxed_error);

    match auth_error {
        AuthError::Io(_) => {
            // Test passed
        }
        _ => panic!("Expected AuthError::Io, got {auth_error:?}"),
    }

    // Create a Box<dyn Error + Send + Sync> containing a JsonError
    let json_str = r#"{"invalid": json"#;
    let json_error: JsonError = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
    let boxed_error: Box<dyn Error + Send + Sync> = Box::new(json_error);

    // Convert to AuthError
    let auth_error = AuthError::from(boxed_error);

    match auth_error {
        AuthError::Json(_) => {
            // Test passed
        }
        _ => panic!("Expected AuthError::Json, got {auth_error:?}"),
    }

    // Create a Box<dyn Error + Send + Sync> containing a different error type
    let boxed_error: Box<dyn Error + Send + Sync> = Box::new(TestError("test error".to_string()));

    // Convert to AuthError - should be Other
    let auth_error = AuthError::from(boxed_error);

    match auth_error {
        AuthError::Other(_) => {
            // Test passed
        }
        _ => panic!("Expected AuthError::Other, got {auth_error:?}"),
    }
}

#[test]
#[allow(deprecated)] // FetchError is deprecated; this pins its Display until removal.
fn test_fetch_error_display() {
    use ig_client::error::FetchError;

    let fetch_error = FetchError::Parser("parsing failed".to_string());
    assert_display_contains(&fetch_error, "parser error");
    assert_display_contains(&fetch_error, "parsing failed");
}

#[test]
#[cfg(feature = "persistence")]
#[allow(deprecated)] // FetchError is deprecated; this pins its Display until removal.
fn test_fetch_error_sqlx_display() {
    use ig_client::error::FetchError;

    let fetch_error = FetchError::Sqlx(SqlxError::RowNotFound);
    assert_display_contains(&fetch_error, "db error");
}

#[test]
fn test_auth_error_rate_limit_exceeded() {
    let auth_error = AuthError::RateLimitExceeded;
    assert_display_contains(&auth_error, "rate limit exceeded");
}

#[test]
fn test_auth_error_unexpected() {
    let auth_error = AuthError::Unexpected(StatusCode::BAD_REQUEST);
    assert_display_contains(&auth_error, "unexpected http status");
    assert_display_contains(&auth_error, "400");
}

#[test]
fn test_app_error_oauth_token_expired() {
    let app_error = AppError::OAuthTokenExpired;
    assert_display_contains(&app_error, "oauth token expired");
}

#[test]
fn test_app_error_historical_data_allowance_exceeded() {
    let error = AppError::HistoricalDataAllowanceExceeded {
        allowance_expiry: 604800,
    };
    assert_display_contains(&error, "historical data allowance exceeded");
    assert_display_contains(&error, "604800");
}

#[test]
fn test_app_error_historical_data_allowance_exceeded_zero_expiry() {
    let error = AppError::HistoricalDataAllowanceExceeded {
        allowance_expiry: 0,
    };
    assert_display_contains(&error, "historical data allowance exceeded");
    assert_display_contains(&error, "0 seconds");
}

#[test]
fn test_app_error_historical_data_allowance_is_not_rate_limit() {
    let historical = AppError::HistoricalDataAllowanceExceeded {
        allowance_expiry: 3600,
    };
    let rate_limit = AppError::RateLimitExceeded;

    // Ensure the two error types produce different display messages
    let historical_msg = historical.to_string();
    let rate_limit_msg = rate_limit.to_string();
    assert_ne!(historical_msg, rate_limit_msg);
    assert!(historical_msg.contains("historical data allowance"));
    assert!(!historical_msg.contains("rate limit exceeded"));
}
