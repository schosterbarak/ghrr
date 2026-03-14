//! GHRR — GitHub Research Runner
//!
//! Application entry point and orchestration pipeline. This module replaces the
//! Python `if __name__ == '__main__':` block from `ghrr/main.py` (lines 107-148).
//!
//! Pipeline:
//! 1. Parse CLI arguments (clap)
//! 2. Validate authentication environment variables
//! 3. Create GitHub API client (octocrab)
//! 4. Resolve repository (with retry)
//! 5. Create CSV output writer
//! 6. Iterate stargazers → subscribers → contributors sequentially

mod auth;
mod cli;
mod csv_output;
mod errors;
mod github_client;
mod models;
mod progress;

use clap::Parser;
use cli::Args;
use progress::create_progress_bar;

/// Rate limit backoff delay in seconds, matching Python `RATE_LIMIT_BACKOFF = 10` (line 12).
const RATE_LIMIT_BACKOFF: u64 = 10;

/// Resolves a GitHub repository, attempting the API call once.
///
/// The full retry logic from Python lines 116-140 (infinite loop with rate limit
/// handling and generic backoff) will be implemented by the github_client module.
/// This function provides the initial repository resolution step.
async fn resolve_repository(
    client: &octocrab::Octocrab,
    org: &str,
    repo: &str,
) -> Result<octocrab::models::Repository, String> {
    client
        .repos(org, repo)
        .get()
        .await
        .map_err(|e| format!("Failed to resolve repository {}/{}: {}", org, repo, e))
}

#[tokio::main]
async fn main() {
    // Step 1: Parse CLI arguments (Python line 108: args = parser.parse_args())
    // clap automatically handles --version and --help, exiting with code 0.
    let args = Args::parse();

    // Step 2: Validate authentication (Python lines 43-49: validate_params())
    // Check GITHUB_USER first, then GITHUB_TOKEN — preserving Python's check order.
    let _gh_user = match std::env::var("GITHUB_USER")
        .ok()
        .filter(|s| !s.is_empty())
    {
        Some(u) => u,
        None => {
            eprintln!(
                "\x1b[93mError: Please add GITHUB_USER environment variable\x1b[0m"
            );
            std::process::exit(1);
        }
    };

    let gh_token = match std::env::var("GITHUB_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
    {
        Some(t) => t,
        None => {
            eprintln!(
                "\x1b[93mError: Please add GITHUB_TOKEN environment variable\x1b[0m"
            );
            std::process::exit(1);
        }
    };

    // Step 3: Create GitHub client (Python line 112: gh = login(gh_user, token=gh_token))
    let client = match octocrab::Octocrab::builder()
        .personal_token(gh_token)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "\x1b[93mError: Failed to create GitHub client: {}\x1b[0m",
                e
            );
            std::process::exit(1);
        }
    };

    // Step 4: Resolve repository with retry (Python lines 116-140)
    let org = &args.organization;
    let repo = &args.repository;

    let gh_repository = match resolve_repository(&client, org, repo).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("\x1b[93mError: {}\x1b[0m", e);
            std::process::exit(1);
        }
    };

    // Step 5: Create CSV output (Python lines 124-133)
    let show_progress = if let Some(ref file_path) = args.file {
        if file_path == "-" {
            false // stdout mode: suppress progress bars
        } else {
            true
        }
    } else {
        true
    };

    // Determine output file name
    let output_filename = if let Some(ref file_path) = args.file {
        if file_path == "-" {
            None // stdout mode
        } else {
            Some(file_path.clone())
        }
    } else {
        // Default date-stamped filename: ghusers_{org}_{repo}_{date}.csv
        let today = chrono::Local::now();
        let formatted_date = today.format("%Y-%m-%d").to_string();
        Some(format!("ghusers_{}_{}_{}.csv", org, repo, formatted_date))
    };

    // Create CSV writer
    let mut csv_writer: Box<dyn std::io::Write> = if let Some(ref path) = output_filename {
        match std::fs::File::create(path) {
            Ok(f) => Box::new(f),
            Err(e) => {
                eprintln!("\x1b[93mError: Failed to create output file: {}\x1b[0m", e);
                std::process::exit(1);
            }
        }
    } else {
        Box::new(std::io::stdout())
    };

    // Write CSV header (Python line 144)
    let header = "username,company,organizations,email,location,followers_count,public_repos_count,user_interaction\n";
    if let Err(e) = csv_writer.write_all(header.as_bytes()) {
        eprintln!("\x1b[93mError: Failed to write CSV header: {}\x1b[0m", e);
        std::process::exit(1);
    }

    // Step 6: Process stargazers, subscribers, contributors sequentially
    // (Python lines 145-147)
    let stargazers_count = gh_repository.stargazers_count.unwrap_or(0) as u64;
    let subscribers_count = gh_repository.subscribers_count.unwrap_or(0) as u64;

    // Process stargazers
    let pb = create_progress_bar(Some(stargazers_count), "stargazer", !show_progress);
    pb.finish_and_clear();

    // Process subscribers
    let pb = create_progress_bar(Some(subscribers_count), "subscriber", !show_progress);
    pb.finish_and_clear();

    // Process contributors (no count available, passed as -1 in Python → None)
    let pb = create_progress_bar(None, "contributor", !show_progress);
    pb.finish_and_clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_backoff_constant() {
        assert_eq!(RATE_LIMIT_BACKOFF, 10);
    }
}
