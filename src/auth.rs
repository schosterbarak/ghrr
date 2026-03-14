//! Authentication module for the GHRR (GitHub Research Runner) application.
//!
//! This module handles GitHub authentication via environment variables, replacing
//! the authentication logic from the original Python `ghrr/main.py`:
//!
//! - Environment variable reads (Python lines 13-14):
//!   ```python
//!   gh_user = os.getenv('GITHUB_USER')
//!   gh_token = os.getenv('GITHUB_TOKEN')
//!   ```
//! - Parameter validation (Python lines 43-49):
//!   ```python
//!   def validate_params(args):
//!       if not gh_user:
//!           die("Please add GITHUB_USER environment variable")
//!       if not gh_token:
//!           die("Please add GITHUB_TOKEN environment variable")
//!   ```
//!
//! The [`AuthCredentials`] struct encapsulates both the GitHub username and personal
//! access token, providing a [`from_env()`](AuthCredentials::from_env) constructor
//! that reads and validates environment variables in a single step.
//!
//! # Error Handling
//!
//! When environment variables are missing or empty, `from_env()` returns
//! `Err(GhrrError::Auth(...))` with the exact same error messages as the Python
//! original. The caller (`main.rs`) is responsible for printing the error with
//! ANSI color codes and exiting:
//!
//! ```text
//! eprintln!("\x1b[93mError: {}\x1b[0m", error);
//! std::process::exit(1);
//! ```

use crate::errors::GhrrError;
use std::env;

/// Authentication credentials for the GitHub API.
///
/// Stores the GitHub username and personal access token read from
/// `GITHUB_USER` and `GITHUB_TOKEN` environment variables, respectively.
///
/// This struct replaces the Python module-level variables (lines 13-14):
/// ```python
/// gh_user = os.getenv('GITHUB_USER')
/// gh_token = os.getenv('GITHUB_TOKEN')
/// ```
///
/// # Examples
///
/// ```no_run
/// use ghrr::auth::AuthCredentials;
///
/// let credentials = AuthCredentials::from_env()
///     .expect("GitHub credentials must be set");
/// println!("Authenticated as: {}", credentials.username);
/// ```
#[derive(Debug, Clone)]
pub struct AuthCredentials {
    /// GitHub username from the `GITHUB_USER` environment variable.
    pub username: String,
    /// GitHub personal access token from the `GITHUB_TOKEN` environment variable.
    pub token: String,
}

impl AuthCredentials {
    /// Reads GitHub credentials from environment variables and validates their presence.
    ///
    /// This method replaces the Python `validate_params()` function (lines 43-49) and
    /// the module-level `os.getenv()` calls (lines 13-14). It reads both `GITHUB_USER`
    /// and `GITHUB_TOKEN` from the process environment and validates that neither is
    /// missing nor empty.
    ///
    /// # Behavioral Parity with Python
    ///
    /// The Python code uses `os.getenv('GITHUB_USER')` which returns `None` when the
    /// variable is not set, and then checks `if not gh_user:` which is falsy for both
    /// `None` and empty string `""`. This Rust implementation replicates that behavior:
    ///
    /// - `env::var()` returns `Err(VarError::NotPresent)` when the variable is not set.
    /// - `env::var()` returns `Ok("")` when the variable is set to an empty string.
    /// - Both cases are treated as missing, matching Python's `if not gh_user:` semantics.
    ///
    /// The check order is preserved: `GITHUB_USER` is validated first, then `GITHUB_TOKEN`,
    /// exactly matching the Python `validate_params()` function order.
    ///
    /// # Errors
    ///
    /// Returns `Err(GhrrError::Auth(...))` with descriptive messages:
    /// - `"Please add GITHUB_USER environment variable"` — if `GITHUB_USER` is missing or empty
    /// - `"Please add GITHUB_TOKEN environment variable"` — if `GITHUB_TOKEN` is missing or empty
    ///
    /// These error messages match the Python original exactly (lines 45, 47).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ghrr::auth::AuthCredentials;
    ///
    /// match AuthCredentials::from_env() {
    ///     Ok(creds) => println!("Authenticated as {}", creds.username),
    ///     Err(e) => {
    ///         eprintln!("\x1b[93mError: {}\x1b[0m", e);
    ///         std::process::exit(1);
    ///     }
    /// }
    /// ```
    pub fn from_env() -> Result<Self, GhrrError> {
        // Read GITHUB_USER: env::var() returns Err if not set, Ok("") if set to empty.
        // Python's `if not gh_user:` catches both None and empty string, so we must
        // handle both cases by converting Ok("") → None via .filter(|s| !s.is_empty()).
        let username = env::var("GITHUB_USER")
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                GhrrError::Auth(
                    "Please add GITHUB_USER environment variable".to_string(),
                )
            })?;

        // Read GITHUB_TOKEN with the same None/empty handling.
        let token = env::var("GITHUB_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                GhrrError::Auth(
                    "Please add GITHUB_TOKEN environment variable".to_string(),
                )
            })?;

        Ok(Self { username, token })
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // IMPORTANT: These tests modify environment variables, which is inherently
    // not thread-safe. They must be run with `--test-threads=1` to prevent
    // race conditions between tests. In Rust 2024 edition, env::set_var() and
    // env::remove_var() are unsafe because they modify shared process state.

    /// Mutex protecting environment variable access in auth tests.
    ///
    /// `env::set_var` and `env::remove_var` are inherently process-global and
    /// non-thread-safe. When multiple tests modify them concurrently, they can
    /// observe each other's changes. This mutex serializes all auth tests that
    /// touch `GITHUB_USER` / `GITHUB_TOKEN` so they are safe to run under the
    /// default parallel test harness without `--test-threads=1`.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Helper: saves the current values of GITHUB_USER and GITHUB_TOKEN,
    /// runs the provided closure, then restores the original values.
    /// This prevents tests from leaking state into each other.
    ///
    /// All access is serialized by [`ENV_MUTEX`] to avoid data races when
    /// the test harness runs tests in parallel.
    fn with_env_vars<F, R>(user: Option<&str>, token: Option<&str>, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        // Acquire the mutex to serialize env var access across test threads
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        // Save original values
        let orig_user = env::var("GITHUB_USER").ok();
        let orig_token = env::var("GITHUB_TOKEN").ok();

        // Set test values
        // SAFETY: Access is serialized by ENV_MUTEX. No other test thread
        // will read or write these env vars while we hold the lock.
        unsafe {
            match user {
                Some(val) => env::set_var("GITHUB_USER", val),
                None => env::remove_var("GITHUB_USER"),
            }
            match token {
                Some(val) => env::set_var("GITHUB_TOKEN", val),
                None => env::remove_var("GITHUB_TOKEN"),
            }
        }

        // Run the test closure
        let result = f();

        // Restore original values
        unsafe {
            match orig_user {
                Some(val) => env::set_var("GITHUB_USER", &val),
                None => env::remove_var("GITHUB_USER"),
            }
            match orig_token {
                Some(val) => env::set_var("GITHUB_TOKEN", &val),
                None => env::remove_var("GITHUB_TOKEN"),
            }
        }

        result
    }

    // --- Success cases ---

    #[test]
    fn test_from_env_success() {
        with_env_vars(Some("testuser"), Some("testtoken"), || {
            let creds = AuthCredentials::from_env().unwrap();
            assert_eq!(creds.username, "testuser");
            assert_eq!(creds.token, "testtoken");
        });
    }

    #[test]
    fn test_from_env_preserves_values_with_special_chars() {
        with_env_vars(Some("user-with-dashes"), Some("ghp_abc123XYZ!@#"), || {
            let creds = AuthCredentials::from_env().unwrap();
            assert_eq!(creds.username, "user-with-dashes");
            assert_eq!(creds.token, "ghp_abc123XYZ!@#");
        });
    }

    #[test]
    fn test_from_env_preserves_whitespace_values() {
        // Whitespace-only values are NOT empty — they should be accepted.
        // Python's `if not gh_user:` treats whitespace as truthy, so " " passes.
        with_env_vars(Some(" "), Some("token"), || {
            let creds = AuthCredentials::from_env().unwrap();
            assert_eq!(creds.username, " ");
            assert_eq!(creds.token, "token");
        });
    }

    // --- Missing env var cases ---

    #[test]
    fn test_from_env_missing_user() {
        with_env_vars(None, Some("testtoken"), || {
            let result = AuthCredentials::from_env();
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(
                err.to_string(),
                "Please add GITHUB_USER environment variable"
            );
        });
    }

    #[test]
    fn test_from_env_missing_token() {
        with_env_vars(Some("testuser"), None, || {
            let result = AuthCredentials::from_env();
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(
                err.to_string(),
                "Please add GITHUB_TOKEN environment variable"
            );
        });
    }

    #[test]
    fn test_from_env_missing_both() {
        with_env_vars(None, None, || {
            let result = AuthCredentials::from_env();
            assert!(result.is_err());
            // GITHUB_USER is checked first, so its error should surface
            let err = result.unwrap_err();
            assert!(err.to_string().contains("GITHUB_USER"));
        });
    }

    // --- Empty string cases (matching Python's `if not gh_user:` for "") ---

    #[test]
    fn test_from_env_empty_user() {
        with_env_vars(Some(""), Some("testtoken"), || {
            let result = AuthCredentials::from_env();
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(
                err.to_string(),
                "Please add GITHUB_USER environment variable"
            );
        });
    }

    #[test]
    fn test_from_env_empty_token() {
        with_env_vars(Some("testuser"), Some(""), || {
            let result = AuthCredentials::from_env();
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(
                err.to_string(),
                "Please add GITHUB_TOKEN environment variable"
            );
        });
    }

    #[test]
    fn test_from_env_both_empty() {
        with_env_vars(Some(""), Some(""), || {
            let result = AuthCredentials::from_env();
            assert!(result.is_err());
            // GITHUB_USER is checked first
            let err = result.unwrap_err();
            assert!(err.to_string().contains("GITHUB_USER"));
        });
    }

    // --- Error type verification ---

    #[test]
    fn test_error_is_ghrr_auth_variant() {
        with_env_vars(None, Some("token"), || {
            let result = AuthCredentials::from_env();
            let err = result.unwrap_err();
            assert!(err.is_auth());
            assert!(!err.is_rate_limit());
        });
    }

    // --- Struct trait verification ---

    #[test]
    fn test_credentials_debug_trait() {
        with_env_vars(Some("user"), Some("secret"), || {
            let creds = AuthCredentials::from_env().unwrap();
            let debug_str = format!("{:?}", creds);
            assert!(debug_str.contains("AuthCredentials"));
            assert!(debug_str.contains("user"));
        });
    }

    #[test]
    fn test_credentials_clone_trait() {
        with_env_vars(Some("user"), Some("token"), || {
            let creds = AuthCredentials::from_env().unwrap();
            let cloned = creds.clone();
            assert_eq!(creds.username, cloned.username);
            assert_eq!(creds.token, cloned.token);
        });
    }
}
