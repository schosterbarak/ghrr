//! Custom error types for the GHRR (GitHub Research Runner) application.
//!
//! This module defines the [`GhrrError`] enum — a comprehensive, typed error hierarchy
//! that replaces the implicit error handling patterns from the original Python implementation:
//!
//! - `warn` lambda (Python line 24): `warn = lambda msg: print(f'\033[93mError: {msg}\033[0m', file=sys.stderr)`
//! - `die` lambda (Python line 25): `die = lambda msg: warn(msg) or exit(1)`
//! - `ForbiddenError` catch (Python lines 99-100, 135-136): Rate limit detection
//! - Generic `except Exception` catch (Python lines 102-104, 138-140): Unexpected error handling
//!
//! The enum derives [`thiserror::Error`] for automatic [`std::error::Error`] and [`std::fmt::Display`]
//! implementations, making it compatible with [`anyhow::Result`] for ergonomic error propagation
//! throughout the application.

use thiserror::Error;

/// Custom error enum covering all error categories in the GHRR application.
///
/// Each variant maps to a specific class of failure encountered during execution:
/// authentication failures, GitHub API errors, rate limiting, CSV output issues,
/// and repository resolution problems.
///
/// The enum implements `std::error::Error` via the `#[derive(Error)]` macro from `thiserror`,
/// enabling seamless integration with `anyhow::Result` and the `?` operator for
/// error propagation across module boundaries.
///
/// # Display Formatting
///
/// - `Auth`: Displays the raw message for exact Python parity (e.g., "Please add GITHUB_USER environment variable")
/// - `Api`: Displays "GitHub API error: {details}"
/// - `RateLimit`: Displays "Rate limit exceeded: {details}"
/// - `Csv`: Displays "CSV error: {details}"
/// - `Repository`: Displays "Repository error: {details}"
///
/// The caller (`main.rs`) is responsible for wrapping error messages with ANSI color codes
/// when printing to stderr:
/// ```text
/// eprintln!("\x1b[93mError: {}\x1b[0m", error);
/// ```
#[derive(Debug, Error)]
pub enum GhrrError {
    /// Authentication errors (missing or empty `GITHUB_USER` or `GITHUB_TOKEN` env vars).
    ///
    /// Replaces:
    /// - `die("Please add GITHUB_USER environment variable")` (Python line 45)
    /// - `die("Please add GITHUB_TOKEN environment variable")` (Python line 47)
    ///
    /// Uses `{0}` format to display the raw message string without any prefix,
    /// preserving exact parity with the Python error messages.
    #[error("{0}")]
    Auth(String),

    /// GitHub API errors (non-rate-limit failures).
    ///
    /// Replaces generic `except Exception` blocks (Python lines 102-104, 138-140)
    /// that catch unexpected errors during API calls.
    #[error("GitHub API error: {0}")]
    Api(String),

    /// Rate limit errors (HTTP 403 Forbidden with rate limit message).
    ///
    /// Replaces `ForbiddenError` catch combined with `wait_rate_limit()` invocation
    /// (Python lines 99-100, 135-136). When this error is detected, the caller
    /// should read the `X-RateLimit-Reset` header, compute the sleep duration
    /// with a 3-second buffer, and retry after sleeping.
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// CSV output errors (file creation failures, write errors, flush failures).
    ///
    /// Replaces implicit file I/O errors from Python's `open()` and `csv.writer`
    /// operations. Captures both `csv::Error` and `std::io::Error` via `From` conversions.
    #[error("CSV error: {0}")]
    Csv(String),

    /// Repository resolution errors.
    ///
    /// Replaces errors during the `gh.repository()` call (Python lines 117-118)
    /// within the repository resolution retry loop.
    #[error("Repository error: {0}")]
    Repository(String),
}

// ---------------------------------------------------------------------------
// From conversions for common error types — enables the `?` operator
// ---------------------------------------------------------------------------

impl From<csv::Error> for GhrrError {
    /// Converts a `csv::Error` into `GhrrError::Csv`.
    ///
    /// Enables the `?` operator for CSV writing operations so that
    /// `csv::Writer::write_record()` errors propagate automatically.
    fn from(err: csv::Error) -> Self {
        GhrrError::Csv(err.to_string())
    }
}

impl From<std::io::Error> for GhrrError {
    /// Converts a `std::io::Error` into `GhrrError::Csv` with an "I/O error:" prefix.
    ///
    /// I/O errors in this application are primarily associated with CSV file
    /// operations (opening files, flushing writers), so they map to the `Csv` variant.
    fn from(err: std::io::Error) -> Self {
        GhrrError::Csv(format!("I/O error: {}", err))
    }
}

impl From<octocrab::Error> for GhrrError {
    /// Converts an `octocrab::Error` into `GhrrError::Api`.
    ///
    /// All octocrab errors are treated as general API errors by default.
    /// Rate limit detection (distinguishing 403 Forbidden responses) is handled
    /// at a higher level in `github_client.rs` where the HTTP response headers
    /// are inspected before constructing the appropriate `GhrrError` variant.
    fn from(err: octocrab::Error) -> Self {
        GhrrError::Api(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// Helper methods for error classification
// ---------------------------------------------------------------------------

impl GhrrError {
    /// Returns `true` if this error represents a GitHub API rate limit condition.
    ///
    /// Equivalent to catching `ForbiddenError` in the Python implementation (line 99).
    /// Used in the retry loops (`iterate_users` and repository resolution) to determine
    /// whether to invoke the rate-limit wait logic or apply a generic backoff delay.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ghrr::errors::GhrrError;
    /// let err = GhrrError::RateLimit("API rate limit exceeded".to_string());
    /// assert!(err.is_rate_limit());
    ///
    /// let err = GhrrError::Api("connection timeout".to_string());
    /// assert!(!err.is_rate_limit());
    /// ```
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, GhrrError::RateLimit(_))
    }

    /// Returns `true` if this error represents an authentication failure.
    ///
    /// Used by the caller to distinguish auth errors (which are fatal and should
    /// cause immediate exit) from other error types that may be retried.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ghrr::errors::GhrrError;
    /// let err = GhrrError::Auth("Please add GITHUB_USER environment variable".to_string());
    /// assert!(err.is_auth());
    ///
    /// let err = GhrrError::Api("connection timeout".to_string());
    /// assert!(!err.is_auth());
    /// ```
    pub fn is_auth(&self) -> bool {
        matches!(self, GhrrError::Auth(_))
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Display formatting tests ---

    #[test]
    fn test_auth_error_display() {
        let err = GhrrError::Auth("Please add GITHUB_USER environment variable".to_string());
        assert_eq!(
            err.to_string(),
            "Please add GITHUB_USER environment variable"
        );
    }

    #[test]
    fn test_auth_error_display_token() {
        let err = GhrrError::Auth("Please add GITHUB_TOKEN environment variable".to_string());
        assert_eq!(
            err.to_string(),
            "Please add GITHUB_TOKEN environment variable"
        );
    }

    #[test]
    fn test_api_error_display() {
        let err = GhrrError::Api("connection timeout".to_string());
        assert_eq!(err.to_string(), "GitHub API error: connection timeout");
    }

    #[test]
    fn test_rate_limit_error_display() {
        let err = GhrrError::RateLimit("API rate limit exceeded".to_string());
        assert_eq!(
            err.to_string(),
            "Rate limit exceeded: API rate limit exceeded"
        );
    }

    #[test]
    fn test_csv_error_display() {
        let err = GhrrError::Csv("file not writable".to_string());
        assert_eq!(err.to_string(), "CSV error: file not writable");
    }

    #[test]
    fn test_repository_error_display() {
        let err = GhrrError::Repository("not found".to_string());
        assert_eq!(err.to_string(), "Repository error: not found");
    }

    // --- Error classification tests ---

    #[test]
    fn test_is_rate_limit_true_for_rate_limit_variant() {
        let err = GhrrError::RateLimit("exceeded".to_string());
        assert!(err.is_rate_limit());
    }

    #[test]
    fn test_is_rate_limit_false_for_api_variant() {
        let err = GhrrError::Api("other".to_string());
        assert!(!err.is_rate_limit());
    }

    #[test]
    fn test_is_rate_limit_false_for_auth_variant() {
        let err = GhrrError::Auth("missing".to_string());
        assert!(!err.is_rate_limit());
    }

    #[test]
    fn test_is_rate_limit_false_for_csv_variant() {
        let err = GhrrError::Csv("write error".to_string());
        assert!(!err.is_rate_limit());
    }

    #[test]
    fn test_is_rate_limit_false_for_repository_variant() {
        let err = GhrrError::Repository("not found".to_string());
        assert!(!err.is_rate_limit());
    }

    #[test]
    fn test_is_auth_true_for_auth_variant() {
        let err = GhrrError::Auth("missing".to_string());
        assert!(err.is_auth());
    }

    #[test]
    fn test_is_auth_false_for_api_variant() {
        let err = GhrrError::Api("other".to_string());
        assert!(!err.is_auth());
    }

    #[test]
    fn test_is_auth_false_for_rate_limit_variant() {
        let err = GhrrError::RateLimit("exceeded".to_string());
        assert!(!err.is_auth());
    }

    #[test]
    fn test_is_auth_false_for_csv_variant() {
        let err = GhrrError::Csv("write error".to_string());
        assert!(!err.is_auth());
    }

    #[test]
    fn test_is_auth_false_for_repository_variant() {
        let err = GhrrError::Repository("not found".to_string());
        assert!(!err.is_auth());
    }

    // --- std::error::Error trait compliance ---

    #[test]
    fn test_error_implements_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(GhrrError::Auth("test".to_string()));
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_error_implements_send() {
        fn assert_send<T: Send>() {}
        assert_send::<GhrrError>();
    }

    #[test]
    fn test_error_implements_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GhrrError>();
    }

    // --- From conversion tests ---

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let ghrr_err: GhrrError = io_err.into();
        match &ghrr_err {
            GhrrError::Csv(msg) => {
                assert!(msg.starts_with("I/O error:"));
                assert!(msg.contains("file missing"));
            }
            other => panic!("Expected Csv variant, got {:?}", other),
        }
    }

    #[test]
    fn test_from_io_error_preserves_message() {
        let io_err =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let ghrr_err: GhrrError = io_err.into();
        assert_eq!(
            ghrr_err.to_string(),
            "CSV error: I/O error: access denied"
        );
    }

    #[test]
    fn test_from_csv_error() {
        // Create a csv::Error by attempting to build one from an invalid UTF-8 scenario.
        // csv::Error implements From<io::Error>, so we can use that path.
        let csv_err: csv::Error = csv::Error::from(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bad csv data",
        ));
        let ghrr_err: GhrrError = csv_err.into();
        match &ghrr_err {
            GhrrError::Csv(msg) => {
                assert!(msg.contains("bad csv data"));
            }
            other => panic!("Expected Csv variant, got {:?}", other),
        }
    }

    // --- Debug formatting ---

    #[test]
    fn test_debug_formatting() {
        let err = GhrrError::Auth("test debug".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Auth"));
        assert!(debug_str.contains("test debug"));
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_message() {
        let err = GhrrError::Api(String::new());
        assert_eq!(err.to_string(), "GitHub API error: ");
    }

    #[test]
    fn test_unicode_message() {
        let err = GhrrError::Auth("认证失败: 缺少令牌".to_string());
        assert_eq!(err.to_string(), "认证失败: 缺少令牌");
    }

    #[test]
    fn test_long_message() {
        let long_msg = "x".repeat(10_000);
        let err = GhrrError::Repository(long_msg.clone());
        assert_eq!(err.to_string(), format!("Repository error: {}", long_msg));
    }

    #[test]
    fn test_message_with_special_chars() {
        let err = GhrrError::Api("error: 'rate' \"limit\" <exceeded> & retry".to_string());
        assert_eq!(
            err.to_string(),
            "GitHub API error: error: 'rate' \"limit\" <exceeded> & retry"
        );
    }
}
