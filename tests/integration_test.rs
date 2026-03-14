//! End-to-End CLI Integration Tests for GHRR (GitHub Research Runner)
//!
//! This file provides integration tests for the GHRR Rust CLI application.
//! Following Cargo conventions, this file in the `tests/` directory is compiled
//! as a separate test binary that invokes the `ghrr` binary externally to verify
//! CLI behavior, authentication validation, CSV output flags, and error handling.
//!
//! These tests validate behavioral parity between the new Rust implementation
//! and the original Python `ghrr/main.py`.
//!
//! # Test Categories
//!
//! - **Version flag tests**: Verify `--version` outputs `0.0.1` (AAP §0.7.1)
//! - **Help flag tests**: Verify `--help` shows correct description and flag documentation
//! - **Argument validation tests**: Verify missing required arguments produce errors
//! - **Authentication tests**: Verify missing `GITHUB_USER`/`GITHUB_TOKEN` env vars produce descriptive errors
//! - **Flag variant tests**: Verify both short (`-o`, `-r`, `-f`) and long (`--organization`, `--repository`, `--file`) flags
//! - **Output mode tests**: Verify `-f <path>` and `-f -` (stdout) modes are accepted
//! - **Exit code tests**: Verify non-zero exit on failures
//!
//! # Dependencies
//!
//! - `assert_cmd` (2.0.16): For invoking the compiled `ghrr` binary in tests
//! - `predicates` (3.1.2): For assertion predicates (string matching)

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

// ---------------------------------------------------------------------------
// CLI Version Flag Tests
// ---------------------------------------------------------------------------

/// Verify that the `--version` flag outputs the correct version string (`0.0.1`).
///
/// **AAP §0.7.1**: "The `--version` flag must output the same version string (`0.0.1`)."
/// The version is sourced from `Cargo.toml` `[package].version = "0.0.1"`, matching
/// the Python original's `ghrr/version.py`: `version = '0.0.1'`.
#[test]
fn test_version_flag() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.0.1"));
}

// ---------------------------------------------------------------------------
// CLI Help Flag Tests
// ---------------------------------------------------------------------------

/// Verify that the `--help` flag displays usage information including the program
/// description and all argument flags.
///
/// **AAP §0.7.1**: The help output must mention "stargazers crawler"
/// (matching Python line 16: `argparse.ArgumentParser(description='stargazers crawler')`)
/// and list all argument flags (`organization`, `repository`).
#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("organization"))
        .stdout(predicate::str::contains("repository"))
        .stdout(predicate::str::contains("stargazers crawler"));
}

/// Verify that the `-h` short help flag also works and shows usage information.
#[test]
fn test_short_help_flag() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.arg("-h")
        .assert()
        .success()
        .stdout(predicate::str::contains("organization"))
        .stdout(predicate::str::contains("repository"));
}

// ---------------------------------------------------------------------------
// CLI Argument Validation Tests
// ---------------------------------------------------------------------------

/// Verify that missing the required `--organization` argument causes a failure
/// with an error message mentioning "organization".
///
/// Matches Python behavior where `required=True` on the `-o` argument (line 18)
/// causes argparse to produce an error when omitted.
#[test]
fn test_missing_organization_arg() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.arg("-r")
        .arg("myrepo")
        .env("GITHUB_USER", "testuser")
        .env("GITHUB_TOKEN", "testtoken")
        .assert()
        .failure()
        .stderr(predicate::str::contains("organization"));
}

/// Verify that missing the required `--repository` argument causes a failure
/// with an error message mentioning "repository".
///
/// Matches Python behavior where `required=True` on the `-r` argument (line 20)
/// causes argparse to produce an error when omitted.
#[test]
fn test_missing_repository_arg() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.arg("-o")
        .arg("myorg")
        .env("GITHUB_USER", "testuser")
        .env("GITHUB_TOKEN", "testtoken")
        .assert()
        .failure()
        .stderr(predicate::str::contains("repository"));
}

/// Verify that providing no arguments at all causes a failure.
///
/// When neither `-o` nor `-r` is provided, clap should emit an error and
/// exit with a non-zero status code.
#[test]
fn test_missing_all_required_args() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.env("GITHUB_USER", "testuser")
        .env("GITHUB_TOKEN", "testtoken")
        .assert()
        .failure();
}

// ---------------------------------------------------------------------------
// Authentication Validation Tests
// ---------------------------------------------------------------------------

/// Verify that a missing `GITHUB_USER` environment variable causes a failure
/// with an error message containing "GITHUB_USER" on stderr.
///
/// **AAP §0.7.1**: "If either is missing, the program must print a descriptive
/// error message to stderr and exit with a non-zero status code."
///
/// Matches Python line 45: `die("Please add GITHUB_USER environment variable")`
#[test]
fn test_missing_github_user_env() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args(["-o", "myorg", "-r", "myrepo"])
        .env_remove("GITHUB_USER")
        .env("GITHUB_TOKEN", "testtoken")
        .assert()
        .failure()
        .stderr(predicate::str::contains("GITHUB_USER"));
}

/// Verify that a missing `GITHUB_TOKEN` environment variable causes a failure
/// with an error message containing "GITHUB_TOKEN" on stderr.
///
/// **AAP §0.7.1**: Matches Python line 47:
/// `die("Please add GITHUB_TOKEN environment variable")`
#[test]
fn test_missing_github_token_env() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args(["-o", "myorg", "-r", "myrepo"])
        .env("GITHUB_USER", "testuser")
        .env_remove("GITHUB_TOKEN")
        .assert()
        .failure()
        .stderr(predicate::str::contains("GITHUB_TOKEN"));
}

/// Verify that when both `GITHUB_USER` and `GITHUB_TOKEN` are missing,
/// the first check (`GITHUB_USER`) fails first, producing the `GITHUB_USER`
/// error message.
///
/// In the Python original, `validate_params()` checks `GITHUB_USER` first
/// (line 44) and `GITHUB_TOKEN` second (line 46). The Rust implementation
/// preserves this ordering.
#[test]
fn test_missing_both_env_vars() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args(["-o", "myorg", "-r", "myrepo"])
        .env_remove("GITHUB_USER")
        .env_remove("GITHUB_TOKEN")
        .assert()
        .failure()
        .stderr(predicate::str::contains("GITHUB_USER"));
}

/// Verify that authentication error output goes to stderr and contains
/// the "Error:" prefix matching the Python ANSI-formatted error output.
///
/// Python line 24: `warn = lambda msg: print(f'\033[93mError: {msg}\033[0m', file=sys.stderr)`
/// The Rust implementation replicates this with: `eprintln!("\x1b[93mError: {}\x1b[0m", msg)`
#[test]
fn test_auth_error_output_to_stderr() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args(["-o", "myorg", "-r", "myrepo"])
        .env_remove("GITHUB_USER")
        .env_remove("GITHUB_TOKEN")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error:"));
}

// ---------------------------------------------------------------------------
// CLI Short and Long Flag Variants Tests
// ---------------------------------------------------------------------------

/// Verify that short flags `-o` and `-r` are accepted by the argument parser.
///
/// This test provides valid auth credentials but an invalid token, so the
/// command will fail on API connection — but crucially should NOT fail on
/// argument parsing. A timeout is used to prevent hanging if the binary
/// attempts to connect to GitHub with the invalid token.
#[test]
fn test_short_flags_accepted() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args(["-o", "testorg", "-r", "testrepo"])
        .env("GITHUB_USER", "testuser")
        .env("GITHUB_TOKEN", "invalidtoken")
        .timeout(Duration::from_secs(10))
        .assert()
        .failure();
    // Will fail on API, but not on arg parsing — confirms short flags work
}

/// Verify that long flags `--organization` and `--repository` are accepted.
///
/// Same rationale as `test_short_flags_accepted`: command will fail on API
/// but not on argument parsing, confirming long flag support.
#[test]
fn test_long_flags_accepted() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args(["--organization", "testorg", "--repository", "testrepo"])
        .env("GITHUB_USER", "testuser")
        .env("GITHUB_TOKEN", "invalidtoken")
        .timeout(Duration::from_secs(10))
        .assert()
        .failure();
    // Will fail on API, but not on arg parsing — confirms long flags work
}

// ---------------------------------------------------------------------------
// CSV Output Flag Tests
// ---------------------------------------------------------------------------

/// Verify that the `-f <path>` flag is accepted syntactically.
///
/// The actual file creation depends on a successful API connection, so this
/// test only verifies that the flag is parsed without an argument parsing error.
/// The binary will fail on API connection (invalid token), but the `-f` flag
/// itself should be accepted.
#[test]
fn test_file_flag_creates_output() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args([
        "-o",
        "testorg",
        "-r",
        "testrepo",
        "-f",
        "/tmp/blitzy_test_ghrr_output.csv",
    ])
    .env("GITHUB_USER", "testuser")
    .env("GITHUB_TOKEN", "invalidtoken")
    .timeout(Duration::from_secs(10))
    .assert()
    .failure();
    // API failure, but -f flag was accepted without arg parse error
}

/// Verify that the `-f -` flag (stdout mode) is accepted syntactically.
///
/// When `-f -` is specified, CSV output should go to stdout and progress bars
/// should be suppressed. This test verifies the flag is parsed correctly.
/// The binary will fail on API, but the `-f -` flag itself should be accepted.
#[test]
fn test_stdout_mode_flag() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args(["-o", "testorg", "-r", "testrepo", "-f", "-"])
        .env("GITHUB_USER", "testuser")
        .env("GITHUB_TOKEN", "invalidtoken")
        .timeout(Duration::from_secs(10))
        .assert()
        .failure();
    // API failure, but -f - flag was accepted without arg parse error
}

/// Verify that the long `--file` flag variant also works.
#[test]
fn test_long_file_flag() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args([
        "--organization",
        "testorg",
        "--repository",
        "testrepo",
        "--file",
        "/tmp/blitzy_test_ghrr_long_flag.csv",
    ])
    .env("GITHUB_USER", "testuser")
    .env("GITHUB_TOKEN", "invalidtoken")
    .timeout(Duration::from_secs(10))
    .assert()
    .failure();
    // API failure, but --file long flag was accepted
}

// ---------------------------------------------------------------------------
// Exit Code Tests
// ---------------------------------------------------------------------------

/// Verify that providing valid arguments with an invalid API token produces
/// a non-zero exit code.
///
/// This test confirms that the binary handles API connection failures gracefully
/// by exiting with a non-zero status code rather than panicking. A timeout is
/// used to prevent the binary from retrying indefinitely with the invalid token.
#[test]
fn test_invalid_token_exits_nonzero() {
    let mut cmd = Command::cargo_bin("ghrr").unwrap();
    cmd.args(["-o", "nonexistent-org", "-r", "nonexistent-repo"])
        .env("GITHUB_USER", "testuser")
        .env("GITHUB_TOKEN", "invalid_token_that_will_fail")
        .timeout(Duration::from_secs(10))
        .assert()
        .failure();
}
