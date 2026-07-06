use std::env;
use std::sync::Once;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

static INIT: Once = Once::new();

/// Maps a `LOGLEVEL`-style string to a [`tracing::Level`].
///
/// Matching is case-insensitive. Any value that is not one of `TRACE`, `DEBUG`,
/// `WARN`, or `ERROR` (including empty or unknown strings) falls back to the
/// default level, [`Level::INFO`].
#[must_use]
#[inline]
pub(crate) fn level_from_str(s: &str) -> Level {
    match s.trim().to_uppercase().as_str() {
        "TRACE" => Level::TRACE,
        "DEBUG" => Level::DEBUG,
        "WARN" => Level::WARN,
        "ERROR" => Level::ERROR,
        // Unknown / empty / "INFO" all resolve to the default.
        _ => Level::INFO,
    }
}

/// Sets up the logger for the application.
///
/// The logger level is determined by the `LOGLEVEL` environment variable
/// (case-insensitive). If the variable is not set or holds an unrecognized
/// value, it defaults to `INFO`.
///
/// This installs a global subscriber and is intended for binaries, examples,
/// and tests only — library code must not install a global subscriber.
pub fn setup_logger() {
    INIT.call_once(|| {
        let level = level_from_str(&env::var("LOGLEVEL").unwrap_or_default());

        let subscriber = FmtSubscriber::builder().with_max_level(level).finish();

        // Ignore the error: a subscriber may already be installed by the host
        // process (another binary, a test harness). `INIT` guards against this
        // crate double-installing, and we never want setup to panic.
        let _ = tracing::subscriber::set_global_default(subscriber);

        tracing::debug!(%level, "log level configured");
    });
}

#[cfg(test)]
mod tests {
    use super::level_from_str;
    use tracing::Level;

    #[test]
    fn test_level_from_str_known_uppercase_variants() {
        assert_eq!(level_from_str("TRACE"), Level::TRACE);
        assert_eq!(level_from_str("DEBUG"), Level::DEBUG);
        assert_eq!(level_from_str("INFO"), Level::INFO);
        assert_eq!(level_from_str("WARN"), Level::WARN);
        assert_eq!(level_from_str("ERROR"), Level::ERROR);
    }

    #[test]
    fn test_level_from_str_is_case_insensitive() {
        assert_eq!(level_from_str("debug"), Level::DEBUG);
        assert_eq!(level_from_str("info"), Level::INFO);
        assert_eq!(level_from_str("Warn"), Level::WARN);
        assert_eq!(level_from_str("eRrOr"), Level::ERROR);
    }

    #[test]
    fn test_level_from_str_trims_surrounding_whitespace() {
        assert_eq!(level_from_str("  debug  "), Level::DEBUG);
    }

    #[test]
    fn test_level_from_str_unknown_defaults_to_info() {
        assert_eq!(level_from_str("INVALID"), Level::INFO);
        assert_eq!(level_from_str(""), Level::INFO);
        assert_eq!(level_from_str("warning"), Level::INFO);
    }
}
