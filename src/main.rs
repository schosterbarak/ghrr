//! GHRR — GitHub Research Runner
//!
//! Application entry point and orchestration pipeline. This module replaces the
//! Python `if __name__ == '__main__':` block from `ghrr/main.py` (lines 107-148).
//!
//! It declares all sub-modules, wires together the application pipeline, and
//! executes the sequential processing of stargazers, subscribers, and contributors.
//!
//! # Pipeline
//!
//! 1. Parse CLI arguments (`clap`)
//! 2. Validate authentication environment variables
//! 3. Create GitHub API client (`octocrab` via `GithubClient`)
//! 4. Resolve repository with retry (handles rate limits and generic errors)
//! 5. Create CSV output writer (file, stdout, or default date-stamped filename)
//! 6. Write CSV header row
//! 7. Iterate stargazers → subscribers → contributors sequentially

// ───────────────────────────────────────────────────────────────────────────────
// Module declarations — one per functional concern, replacing the monolithic
// Python ghrr/main.py with modular Rust sources.
// ───────────────────────────────────────────────────────────────────────────────
mod auth;
mod cli;
mod csv_output;
mod errors;
mod github_client;
mod models;
mod progress;

// ───────────────────────────────────────────────────────────────────────────────
// Imports
// ───────────────────────────────────────────────────────────────────────────────

use anyhow::Result;
use clap::Parser;
use std::time::Duration;

use auth::AuthCredentials;
use cli::Args;
use csv_output::CsvOutput;
use errors::GhrrError;
use github_client::{GithubClient, SimpleUser};
use progress::create_progress_bar;

// ───────────────────────────────────────────────────────────────────────────────
// Constants
// ───────────────────────────────────────────────────────────────────────────────

/// Rate limit backoff delay in seconds for generic (non-rate-limit) errors.
///
/// Matches Python `RATE_LIMIT_BACKOFF = 10` (main.py line 12).
/// Used as a fallback sleep duration when a non-rate-limit error occurs during
/// user iteration or repository resolution retry loops.
pub const RATE_LIMIT_BACKOFF: u64 = 10;

// ───────────────────────────────────────────────────────────────────────────────
// iterate_users — User stream processing with retry logic
// ───────────────────────────────────────────────────────────────────────────────

/// Iterates over a list of GitHub users, enriches each with full profile data,
/// writes a CSV row for each user, and handles rate limit and generic errors
/// with appropriate retry logic.
///
/// Replaces Python `iterate_users()` function (main.py lines 78-104):
/// ```python
/// def iterate_users(gh, users_iterator, users_count, user_writer,
///                    user_interaction, progress=True):
///     total = users_count if users_count > 0 else None
///     progress = tqdm(total=total, ...) if progress else DummyProgress()
///     with progress as progress_bar:
///         for u in users_iterator:
///             data_received = False
///             while not data_received:
///                 try:
///                     user = get_user_data(gh, u)
///                     user_writer.writerow([...])
///                     data_received = True
///                     progress_bar.update(1)
///                 except ForbiddenError as e:
///                     wait_rate_limit(e)
///                     continue
///                 except Exception as ae:
///                     warn(f'got an unexpected error: {ae}, ...')
///                     sleep(RATE_LIMIT_BACKOFF)
/// ```
///
/// # Key Behaviors
///
/// - For each user, retries until successful (infinite retry loop matching
///   Python's `while not data_received`)
/// - Rate limit errors (ForbiddenError equivalent) trigger `wait_rate_limit()`,
///   then retry
/// - Generic errors print a yellow ANSI warning to stderr, sleep
///   `RATE_LIMIT_BACKOFF` seconds, then retry
/// - Writes CSV row immediately after successful data retrieval
/// - Updates progress bar after each successful user
///
/// # Arguments
///
/// * `client` - GitHub API client for user data enrichment
/// * `users` - List of lightweight user references to process
/// * `users_count` - Expected total count for progress bar, or negative if unknown
///   (Python passes -1 for contributors where the total is unknown)
/// * `csv_output` - Mutable reference to the CSV output writer
/// * `user_interaction` - Interaction type label: `"stargazer"`, `"subscriber"`,
///   or `"contributor"`
/// * `show_progress` - Whether to show progress bars (`false` when output is stdout)
pub async fn iterate_users(
    client: &GithubClient,
    users: Vec<SimpleUser>,
    users_count: i64,
    csv_output: &mut CsvOutput,
    user_interaction: &str,
    show_progress: bool,
) -> Result<()> {
    // Convert count to Option<u64> for progress bar:
    // Positive values → Some(n), non-positive (including -1) → None
    // Matches Python: total = users_count if users_count > 0 else None
    let total = if users_count > 0 {
        Some(users_count as u64)
    } else {
        None
    };

    // Create progress bar: active or hidden based on show_progress flag.
    // The `silent` parameter is the inverse of `show_progress`.
    // Matches Python line 80: tqdm(...) if progress else DummyProgress()
    let progress = create_progress_bar(total, user_interaction, !show_progress);

    for u in users {
        let mut data_received = false;

        // Infinite retry loop — matches Python's `while not data_received:` (line 84)
        while !data_received {
            match client.get_user_data(&u).await {
                Ok(user) => {
                    // Write CSV row immediately after successful enrichment
                    // (Python lines 87-96: user_writer.writerow([...]))
                    csv_output.write_user(&user, user_interaction)?;
                    data_received = true;

                    // Update progress bar (Python line 98: progress_bar.update(1))
                    progress.inc(1);
                }
                Err(e) => {
                    // Classify the error by downcasting to GhrrError.
                    // Rate limit errors → call wait_rate_limit(), then retry
                    // Generic errors → warn to stderr, sleep, then retry
                    let is_rate_limit = e
                        .downcast_ref::<GhrrError>()
                        .is_some_and(|ge| ge.is_rate_limit());

                    if is_rate_limit {
                        // Rate limit detected (Python lines 99-101):
                        //   except ForbiddenError as e:
                        //       wait_rate_limit(e)
                        //       continue
                        client.wait_rate_limit().await?;
                    } else {
                        // Generic error (Python lines 102-104):
                        //   except Exception as ae:
                        //       warn(f'got an unexpected error: {ae}, ...')
                        //       sleep(RATE_LIMIT_BACKOFF)
                        eprintln!(
                            "\x1b[93mError: got an unexpected error: {}, will wait a bit and try again\x1b[0m",
                            e
                        );
                        tokio::time::sleep(Duration::from_secs(RATE_LIMIT_BACKOFF)).await;
                    }
                }
            }
        }
    }

    // Finish and clear progress bar after all users are processed
    progress.finish_and_clear();
    Ok(())
}

// ───────────────────────────────────────────────────────────────────────────────
// main — Application entry point
// ───────────────────────────────────────────────────────────────────────────────

/// Async entry point for the GHRR application.
///
/// Orchestrates the full pipeline: parse CLI args → validate auth → create client
/// → resolve repository (with retry) → create CSV writer → iterate user streams.
///
/// Replaces the Python `if __name__ == '__main__':` block (main.py lines 107-148).
#[tokio::main]
async fn main() -> Result<()> {
    // ─── Step 1: Parse CLI arguments ────────────────────────────────────────
    // Python line 108: args = parser.parse_args()
    // clap automatically handles --version and --help, exiting with code 0.
    let args = Args::parse();

    // ─── Step 2: Validate authentication ────────────────────────────────────
    // Python lines 109, 43-49: validate_params(args)
    // AuthCredentials::from_env() reads GITHUB_USER and GITHUB_TOKEN from
    // environment variables. On failure, print the error with ANSI yellow
    // color codes matching the Python die() lambda behavior and exit with
    // non-zero status.
    let credentials = match AuthCredentials::from_env() {
        Ok(c) => c,
        Err(e) => {
            // Replicates Python die() lambda: warn(msg) or exit(1)
            // warn = lambda msg: print(f'\033[93mError: {msg}\033[0m', file=sys.stderr)
            eprintln!("\x1b[93mError: {}\x1b[0m", e);
            std::process::exit(1);
        }
    };

    // ─── Step 3: Create GitHub client ───────────────────────────────────────
    // Python line 112: gh = login(gh_user, token=gh_token)
    let client = GithubClient::new(&credentials)?;

    // ─── Step 4: Resolve repository with retry ──────────────────────────────
    // Python lines 113-140: while not repo_succeeded: try/except loop
    // This infinite retry loop handles both rate limit errors (ForbiddenError)
    // and generic errors (Exception) during repository resolution.
    let repo_data = loop {
        match client
            .resolve_repository(&args.organization, &args.repository)
            .await
        {
            Ok(data) => break data,
            Err(e) => {
                // Classify the error to determine retry strategy
                let is_rate_limit = e
                    .downcast_ref::<GhrrError>()
                    .is_some_and(|ge| ge.is_rate_limit());

                if is_rate_limit {
                    // Rate limit → wait for reset (Python lines 135-137):
                    //   except ForbiddenError as e:
                    //       wait_rate_limit(e)
                    //       continue
                    client.wait_rate_limit().await?;
                } else {
                    // Generic error → warn and sleep (Python lines 138-140):
                    //   except Exception as ae:
                    //       warn(f'got an unexpected error: {ae}, ...')
                    //       sleep(RATE_LIMIT_BACKOFF)
                    eprintln!(
                        "\x1b[93mError: got an unexpected error: {}, will wait a bit and try again\x1b[0m",
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(RATE_LIMIT_BACKOFF)).await;
                }
            }
        }
    };

    // ─── Step 5: Create CSV output ──────────────────────────────────────────
    // Determine output mode based on --file argument (Python lines 124-133)
    let (mut csv_output, show_progress) = if let Some(ref file_path) = args.file {
        if file_path == "-" {
            // -f - mode: output to stdout, suppress progress bars
            // Python lines 129-131:
            //   output = sys.stdout
            //   progress = False
            (CsvOutput::from_stdout()?, false)
        } else {
            // Explicit file path mode
            // Python line 133: output = open(args.file, mode='w')
            (CsvOutput::from_path(file_path)?, true)
        }
    } else {
        // Default: date-stamped filename ghusers_{org}_{repo}_{YYYY-MM-DD}.csv
        // Python lines 124-128:
        //   today = date.today()
        //   formated_today = today.strftime("%Y-%m-%d")
        //   file_name = 'ghusers_{}_{}_{}.csv'.format(org, repository, formated_today)
        //   output = open(file_name, mode='w')
        // NOTE: Date format uses %Y-%m-%d (with dashes) matching Python source line 126 exactly.
        let filename = CsvOutput::default_filename(&args.organization, &args.repository);
        (CsvOutput::from_path(&filename)?, true)
    };

    // ─── Step 6: Write CSV header row ───────────────────────────────────────
    // Python line 144: user_writer.writerow(["username", "company", "organizations",
    //   "email", "location", "followers_count", "public_repos_count", "user_interaction"])
    csv_output.write_header()?;

    // ─── Step 7: Process user streams sequentially ──────────────────────────
    // CRITICAL: Process in this exact order (AAP §0.7.1 behavioral parity):
    //   1. Stargazers first (Python line 145)
    //   2. Subscribers second (Python line 146)
    //   3. Contributors third (Python line 147)

    // 7a. Stargazers (Python line 145):
    //   iterate_users(gh, stargazers, stargazers_count, user_writer, "stargazer", progress=progress)
    iterate_users(
        &client,
        repo_data.stargazers,
        repo_data.stargazers_count as i64,
        &mut csv_output,
        "stargazer",
        show_progress,
    )
    .await?;

    // 7b. Subscribers (Python line 146):
    //   iterate_users(gh, subscribers, subscribers_count, user_writer, "subscriber", progress=progress)
    iterate_users(
        &client,
        repo_data.subscribers,
        repo_data.subscribers_count as i64,
        &mut csv_output,
        "subscriber",
        show_progress,
    )
    .await?;

    // 7c. Contributors (Python line 147):
    //   iterate_users(gh, contributors, -1, user_writer, "contributor", progress=progress)
    // Contributors have no count — passed as -1, meaning total=None for progress bar
    iterate_users(
        &client,
        repo_data.contributors,
        -1,
        &mut csv_output,
        "contributor",
        show_progress,
    )
    .await?;

    Ok(())
}

// ───────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ───────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the RATE_LIMIT_BACKOFF constant is 10 seconds,
    /// matching the Python original (main.py line 12).
    #[test]
    fn test_rate_limit_backoff_constant() {
        assert_eq!(RATE_LIMIT_BACKOFF, 10);
    }

    /// Verify that the module structure compiles correctly.
    /// This test ensures all 7 mod declarations and use imports are valid.
    #[test]
    fn test_module_structure_compiles() {
        // If this test compiles and runs, the module structure is valid.
        // Verifies that mod cli, mod auth, mod github_client, mod models,
        // mod csv_output, mod progress, mod errors all resolve correctly.
        assert!(true);
    }

    /// Verify that the RATE_LIMIT_BACKOFF constant type is u64.
    #[test]
    fn test_rate_limit_backoff_is_u64() {
        let _: u64 = RATE_LIMIT_BACKOFF;
    }

    /// Verify that GhrrError can be used for rate limit detection (compile check).
    #[test]
    fn test_ghrr_error_rate_limit_detection() {
        let err = GhrrError::RateLimit("test".to_string());
        assert!(err.is_rate_limit());

        let err = GhrrError::Api("generic".to_string());
        assert!(!err.is_rate_limit());
    }

    /// Verify that GhrrError auth detection works (compile check).
    #[test]
    fn test_ghrr_error_auth_detection() {
        let err = GhrrError::Auth("missing token".to_string());
        assert!(err.is_auth());

        let err = GhrrError::Api("other error".to_string());
        assert!(!err.is_auth());
    }
}
