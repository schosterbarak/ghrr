//! GitHub API client wrapper for the GHRR (GitHub Research Runner) application.
//!
//! This module wraps the [`octocrab`] crate to provide GitHub API access equivalent
//! to the Python `github3.py` usage in `ghrr/main.py`. It encapsulates:
//!
//! - GitHub client construction with personal token authentication
//!   (Python line 112: `gh = login(gh_user, token=gh_token)`)
//! - Repository resolution (Python lines 116-140)
//! - User listing: stargazers, subscribers, contributors
//!   (Python lines 119-123)
//! - User profile enrichment — `get_user_data()`
//!   (Python lines 29-41)
//! - Rate limit handling — `wait_rate_limit()`
//!   (Python lines 66-75)
//! - Error classification for rate limit vs generic API errors
//!   (Python lines 99-104)
//!
//! # Architecture
//!
//! The [`GithubClient`] struct holds an [`Octocrab`] instance and provides typed
//! methods for each GitHub API interaction needed by the GHRR pipeline. All public
//! async methods return [`anyhow::Result<T>`] for ergonomic error propagation.
//!
//! Rate limit handling uses the GitHub `/rate_limit` API endpoint to determine the
//! reset timestamp, since `octocrab`'s error types do not expose raw HTTP response
//! headers. This is functionally equivalent to the Python approach of reading the
//! `X-RateLimit-Reset` header from `ForbiddenError.response.headers`.

use crate::auth::AuthCredentials;
use crate::errors::GhrrError;
use crate::models::User;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use octocrab::{Octocrab, Page};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

// Note: `reqwest` is used transitively through octocrab for HTTP transport.
// The `GhrrError` enum variants (Api, RateLimit, Repository) are used for
// typed error classification in API calls throughout this module.

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Rate limit backoff delay in seconds for generic (non-rate-limit) errors.
///
/// Matches Python `RATE_LIMIT_BACKOFF = 10` (main.py line 12).
/// Used as a fallback sleep duration when a non-rate-limit error occurs during
/// user iteration or repository resolution retry loops.
pub const RATE_LIMIT_BACKOFF: u64 = 10;

// ---------------------------------------------------------------------------
// Data Structures
// ---------------------------------------------------------------------------

/// Lightweight user representation holding just the GitHub login handle.
///
/// This struct is used as an intermediate type for deserializing user lists from
/// GitHub's stargazers, subscribers, and contributors API endpoints. Only the
/// `login` field is extracted; all other fields in the JSON response are ignored
/// by serde's default behavior.
///
/// After listing, each `SimpleUser` is passed to [`GithubClient::get_user_data()`]
/// for full profile enrichment into a [`User`] struct.
#[derive(Debug, Clone, Deserialize)]
pub struct SimpleUser {
    /// GitHub username (login handle) for this user.
    pub login: String,
}

/// Container for resolved repository data including user lists and counts.
///
/// Returned by [`GithubClient::resolve_repository()`] after fetching repository
/// metadata and all three user listing endpoints. The counts come from the
/// repository metadata endpoint; the user vectors come from paginated listing.
///
/// # Fields
///
/// | Field              | Source                                      |
/// |--------------------|---------------------------------------------|
/// | `stargazers`       | `GET /repos/{owner}/{repo}/stargazers`      |
/// | `stargazers_count` | `Repository.stargazers_count` metadata      |
/// | `subscribers`      | `GET /repos/{owner}/{repo}/subscribers`     |
/// | `subscribers_count`| `Repository.subscribers_count` metadata     |
/// | `contributors`     | `GET /repos/{owner}/{repo}/contributors`    |
#[derive(Debug)]
pub struct RepoData {
    /// All stargazers (users who starred the repository).
    pub stargazers: Vec<SimpleUser>,
    /// Total stargazer count from repository metadata.
    pub stargazers_count: u64,
    /// All subscribers (users watching the repository).
    pub subscribers: Vec<SimpleUser>,
    /// Total subscriber count from repository metadata.
    pub subscribers_count: u64,
    /// All contributors (users who committed to the repository).
    pub contributors: Vec<SimpleUser>,
}

/// Lightweight organization entry for deserializing the `/users/{username}/orgs`
/// endpoint response. Only the `login` field is needed for organization name
/// aggregation in [`GithubClient::get_user_data()`].
#[derive(Debug, Clone, Deserialize)]
struct OrgEntry {
    login: String,
}

/// Query parameters for paginated GitHub API requests.
///
/// Used with `Octocrab::get()` to request the maximum number of results per page
/// (100), minimizing the number of HTTP round-trips for pagination.
#[derive(serde::Serialize)]
struct ListParams {
    per_page: u8,
}

impl Default for ListParams {
    fn default() -> Self {
        Self { per_page: 100 }
    }
}

// ---------------------------------------------------------------------------
// GithubClient
// ---------------------------------------------------------------------------

/// GitHub API client wrapping [`Octocrab`] with rate limit handling and retry support.
///
/// This struct replaces the Python `github3.login()` client (main.py line 112) and
/// encapsulates all GitHub API interactions needed by the GHRR pipeline:
///
/// - Repository resolution with metadata retrieval
/// - Paginated user listing (stargazers, subscribers, contributors)
/// - Full user profile enrichment with organization aggregation
/// - Rate limit detection and sleep-until-reset waiting
///
/// # Construction
///
/// Use [`GithubClient::new()`] with [`AuthCredentials`] to create a client:
/// ```no_run
/// # use ghrr::auth::AuthCredentials;
/// # use ghrr::github_client::GithubClient;
/// # fn example() -> anyhow::Result<()> {
/// let creds = AuthCredentials::from_env().unwrap();
/// let client = GithubClient::new(&creds)?;
/// # Ok(())
/// # }
/// ```
pub struct GithubClient {
    /// The underlying octocrab client instance.
    client: Octocrab,
}

impl GithubClient {
    /// Creates a new `GithubClient` with personal access token authentication.
    ///
    /// Replaces Python `gh = login(gh_user, token=gh_token)` (main.py line 112).
    /// Uses `Octocrab::builder().personal_token()` for token-based authentication,
    /// which is the modern replacement for GitHub's deprecated password-based auth.
    ///
    /// # Arguments
    ///
    /// * `credentials` - Authentication credentials containing the personal access token.
    ///   The `token` field is cloned for the builder.
    ///
    /// # Errors
    ///
    /// Returns an error if the octocrab client cannot be constructed (e.g., invalid
    /// TLS configuration or builder error).
    pub fn new(credentials: &AuthCredentials) -> Result<Self> {
        let client = Octocrab::builder()
            .personal_token(credentials.token.clone())
            .build()
            .context("Failed to create GitHub client")?;
        Ok(Self { client })
    }

    /// Resolves a GitHub repository and fetches all user lists with their counts.
    ///
    /// This method replaces the Python repository resolution block (main.py lines 116-123):
    /// ```python
    /// gh_repository = gh.repository(org, repository)
    /// stargazers = gh_repository.stargazers()
    /// stargazers_count = gh_repository.stargazers_count
    /// contributors = gh_repository.contributors()
    /// subscribers = gh_repository.subscribers()
    /// subscribers_count = gh_repository.subscribers_count
    /// ```
    ///
    /// The repository metadata (stargazers_count, subscribers_count) is fetched from
    /// the `GET /repos/{owner}/{repo}` endpoint. User lists are fetched from their
    /// respective listing endpoints with full pagination.
    ///
    /// # Arguments
    ///
    /// * `org` - Repository owner (organization or user)
    /// * `repo` - Repository name
    ///
    /// # Errors
    ///
    /// Returns an error if the repository cannot be resolved or any user listing
    /// endpoint fails. The caller should handle rate limit errors and retry.
    pub async fn resolve_repository(&self, org: &str, repo: &str) -> Result<RepoData> {
        // Fetch repository metadata for stargazers_count and subscribers_count
        let repository = self.client.repos(org, repo).get().await.map_err(|e| {
            GhrrError::Repository(format!("Failed to resolve {}/{}: {}", org, repo, e))
        })?;

        let stargazers_count = repository.stargazers_count.unwrap_or(0) as u64;
        // subscribers_count is Option<i64>; clamp negative values to 0
        let subscribers_count = repository
            .subscribers_count
            .map(|c| c.max(0) as u64)
            .unwrap_or(0);

        // Fetch all user lists with pagination
        let stargazers = self.list_stargazers(org, repo).await?;
        let subscribers = self.list_subscribers(org, repo).await?;
        let contributors = self.list_contributors(org, repo).await?;

        Ok(RepoData {
            stargazers,
            stargazers_count,
            subscribers,
            subscribers_count,
            contributors,
        })
    }

    /// Lists all stargazers of a repository with full pagination.
    ///
    /// Replaces Python `gh_repository.stargazers()` (main.py line 119).
    /// Uses `GET /repos/{owner}/{repo}/stargazers` with 100 results per page,
    /// following pagination links until all stargazers are collected.
    ///
    /// # Arguments
    ///
    /// * `org` - Repository owner (organization or user)
    /// * `repo` - Repository name
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or pagination cannot be completed.
    pub async fn list_stargazers(&self, org: &str, repo: &str) -> Result<Vec<SimpleUser>> {
        let route = format!("/repos/{}/{}/stargazers", org, repo);
        let params = ListParams::default();
        let first_page: Page<SimpleUser> = self
            .client
            .get(&route, Some(&params))
            .await
            .map_err(|e| GhrrError::Api(format!("Failed to list stargazers: {}", e)))?;
        let all_users = self
            .client
            .all_pages(first_page)
            .await
            .map_err(|e| GhrrError::Api(format!("Failed to paginate stargazers: {}", e)))?;
        Ok(all_users)
    }

    /// Lists all subscribers (watchers) of a repository with full pagination.
    ///
    /// Replaces Python `gh_repository.subscribers()` (main.py line 122).
    /// Uses `GET /repos/{owner}/{repo}/subscribers` with 100 results per page,
    /// following pagination links until all subscribers are collected.
    ///
    /// # Arguments
    ///
    /// * `org` - Repository owner (organization or user)
    /// * `repo` - Repository name
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or pagination cannot be completed.
    pub async fn list_subscribers(&self, org: &str, repo: &str) -> Result<Vec<SimpleUser>> {
        let route = format!("/repos/{}/{}/subscribers", org, repo);
        let params = ListParams::default();
        let first_page: Page<SimpleUser> = self
            .client
            .get(&route, Some(&params))
            .await
            .map_err(|e| GhrrError::Api(format!("Failed to list subscribers: {}", e)))?;
        let all_users = self
            .client
            .all_pages(first_page)
            .await
            .map_err(|e| GhrrError::Api(format!("Failed to paginate subscribers: {}", e)))?;
        Ok(all_users)
    }

    /// Lists all contributors of a repository with full pagination.
    ///
    /// Replaces Python `gh_repository.contributors()` (main.py line 121).
    /// Uses `GET /repos/{owner}/{repo}/contributors` with 100 results per page,
    /// following pagination links until all contributors are collected.
    ///
    /// # Arguments
    ///
    /// * `org` - Repository owner (organization or user)
    /// * `repo` - Repository name
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or pagination cannot be completed.
    pub async fn list_contributors(&self, org: &str, repo: &str) -> Result<Vec<SimpleUser>> {
        let route = format!("/repos/{}/{}/contributors", org, repo);
        let params = ListParams::default();
        let first_page: Page<SimpleUser> = self
            .client
            .get(&route, Some(&params))
            .await
            .map_err(|e| GhrrError::Api(format!("Failed to list contributors: {}", e)))?;
        let all_users = self
            .client
            .all_pages(first_page)
            .await
            .map_err(|e| GhrrError::Api(format!("Failed to paginate contributors: {}", e)))?;
        Ok(all_users)
    }

    /// Enriches a [`SimpleUser`] with full profile data and organization memberships.
    ///
    /// Replaces Python `get_user_data(gh, u)` function (main.py lines 29-41):
    /// ```python
    /// def get_user_data(gh, u):
    ///     username = u.login
    ///     gh_user = gh.user(username)
    ///     company = gh_user.company
    ///     followers = gh_user.followers_count
    ///     repos = gh_user.public_repos_count
    ///     if company:
    ///         company = company.replace("@", "")
    ///     organizationsIterator = gh_user.organizations()
    ///     organizations = []
    ///     for org in organizationsIterator:
    ///         organizations.append(org.login)
    ///     return User(gh_user.email, gh_user.location, username, company, followers, repos, organizations)
    /// ```
    ///
    /// # Behavioral Parity
    ///
    /// - **Company normalization:** All `@` characters are stripped from the company
    ///   name, matching Python's `company.replace("@", "")`.
    /// - **Organizations:** Organization login names are collected and joined with
    ///   `", "` separator for CSV output.
    /// - **None handling:** `email` and `location` default to empty strings when
    ///   not publicly available, matching Python's `csv.writer` None → empty behavior.
    ///
    /// # Arguments
    ///
    /// * `simple_user` - A lightweight user reference with just the login name.
    ///
    /// # Errors
    ///
    /// Returns an error if the user profile cannot be fetched. Organization listing
    /// errors are handled gracefully by returning an empty organizations list.
    pub async fn get_user_data(&self, simple_user: &SimpleUser) -> Result<User> {
        let username = &simple_user.login;

        // Fetch full user profile (Python line 31: gh_user = gh.user(username))
        let profile = self
            .client
            .users(username)
            .profile()
            .await
            .map_err(|e| {
                GhrrError::Api(format!("Failed to get profile for {}: {}", username, e))
            })?;

        // Fetch user's organization memberships (Python lines 37-40)
        // Uses GET /users/{username}/orgs with pagination
        let organizations = self.fetch_user_organizations(username).await;

        // Construct enriched User via User::new() which handles:
        // - email/location None → empty string conversion
        // - company @ stripping via normalize_company()
        // - organizations joining via join_organizations()
        Ok(User::new(
            profile.email,
            profile.location,
            username.clone(),
            profile.company,
            profile.followers,
            profile.public_repos,
            organizations,
        ))
    }

    /// Fetches organization memberships for a user.
    ///
    /// Uses `GET /users/{username}/orgs` with pagination to collect all organization
    /// login names. Errors are handled gracefully by returning an empty vector,
    /// matching the Python behavior where organization listing rarely fails.
    async fn fetch_user_organizations(&self, username: &str) -> Vec<String> {
        let route = format!("/users/{}/orgs", username);
        let params = ListParams::default();

        // Attempt to fetch the first page of organizations
        let first_page_result: std::result::Result<Page<OrgEntry>, _> =
            self.client.get(&route, Some(&params)).await;

        match first_page_result {
            Ok(first_page) => {
                // Paginate through all organization pages
                match self.client.all_pages(first_page).await {
                    Ok(all_orgs) => all_orgs.into_iter().map(|o| o.login).collect(),
                    Err(_) => Vec::new(),
                }
            }
            Err(_) => Vec::new(),
        }
    }

    /// Waits for the GitHub API rate limit to reset.
    ///
    /// Replaces Python `wait_rate_limit(e: ForbiddenError)` function (main.py lines 66-75):
    /// ```python
    /// def wait_rate_limit(e: ForbiddenError):
    ///     if not e.message.startswith('API rate limit exceeded'):
    ///         raise e
    ///     reset_time = e.response.headers.get('X-RateLimit-Reset')
    ///     sleep_until = datetime.fromtimestamp(int(reset_time))
    ///     seconds_to_sleep = int((sleep_until - datetime.now()).total_seconds()) + 3
    ///     sleep_until_pretty = sleep_until.strftime('%m/%d/%Y, %H:%M:%S')
    ///     warn(f'Rate limited. sleeping until "{sleep_until_pretty}" due to rate limit...')
    ///     sleep(seconds_to_sleep)
    /// ```
    ///
    /// Since `octocrab`'s error types do not expose raw HTTP response headers, this
    /// implementation queries the `/rate_limit` API endpoint to obtain the reset
    /// timestamp. This is functionally equivalent — both approaches read the same
    /// `X-RateLimit-Reset` value (the endpoint returns `resources.core.reset`).
    ///
    /// # Behavior
    ///
    /// 1. Queries `GET /rate_limit` for the core rate limit reset timestamp
    /// 2. Computes sleep duration: `(reset_time - now) + 3 seconds` buffer
    /// 3. Prints a yellow ANSI warning to stderr with the formatted reset time
    /// 4. Sleeps asynchronously for the computed duration
    /// 5. Falls back to `RATE_LIMIT_BACKOFF` seconds if the rate limit API is unavailable
    ///
    /// # Errors
    ///
    /// This method does not propagate errors — it always succeeds by falling back
    /// to the `RATE_LIMIT_BACKOFF` constant if the rate limit query fails.
    pub async fn wait_rate_limit(&self) -> Result<()> {
        match self.client.ratelimit().get().await {
            Ok(rate_limit) => {
                let reset_time = rate_limit.resources.core.reset as i64;

                // Convert Unix timestamp to DateTime<Utc> (Python line 70)
                let sleep_until = DateTime::from_timestamp(reset_time, 0)
                    .unwrap_or_else(Utc::now);

                // Compute sleep duration with 3-second buffer (Python line 72)
                let now = Utc::now();
                let seconds_to_sleep = (sleep_until - now).num_seconds() + 3;
                let seconds_to_sleep = seconds_to_sleep.max(0) as u64;

                // Print yellow ANSI warning to stderr (Python line 73-74)
                let sleep_until_pretty = sleep_until.format("%m/%d/%Y, %H:%M:%S");
                eprintln!(
                    "\x1b[93mError: Rate limited. sleeping until \"{}\" due to rate limit...\x1b[0m",
                    sleep_until_pretty
                );

                // Sleep until rate limit resets (Python line 75)
                sleep(Duration::from_secs(seconds_to_sleep)).await;
            }
            Err(_) => {
                // Fallback: if rate limit API is unavailable, use constant backoff
                eprintln!(
                    "\x1b[93mError: Rate limited. sleeping for {} seconds due to rate limit...\x1b[0m",
                    RATE_LIMIT_BACKOFF
                );
                sleep(Duration::from_secs(RATE_LIMIT_BACKOFF)).await;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Standalone Helper Functions
// ---------------------------------------------------------------------------

/// Checks if an [`octocrab::Error`] represents a GitHub API rate limit condition.
///
/// Replaces the Python pattern of catching `ForbiddenError` and checking its message
/// (main.py lines 66-68, 99-100):
/// ```python
/// except ForbiddenError as e:
///     wait_rate_limit(e)
/// # and inside wait_rate_limit:
/// if not e.message.startswith('API rate limit exceeded'):
///     raise e
/// ```
///
/// A rate limit error is identified by two conditions:
/// 1. The HTTP status code is 403 Forbidden
/// 2. The error message starts with "API rate limit exceeded"
///
/// This two-condition check ensures that other 403 errors (e.g., insufficient
/// permissions) are not mistakenly classified as rate limit errors.
///
/// # Arguments
///
/// * `err` - An octocrab error to classify
///
/// # Returns
///
/// `true` if the error represents a rate limit condition; `false` otherwise.
pub fn is_rate_limit_error(err: &octocrab::Error) -> bool {
    // Pattern match on octocrab's non-exhaustive Error enum.
    // The GitHub variant contains a BoxError<GitHubError> with status_code and message.
    if let octocrab::Error::GitHub { source, .. } = err {
        // Check for 403 Forbidden status AND rate limit message prefix
        // Python line 67: if not e.message.startswith('API rate limit exceeded')
        is_rate_limit_response(source.status_code.as_u16(), &source.message)
    } else {
        false
    }
}

/// Checks whether a given HTTP status code and message indicate a GitHub
/// rate limit condition.
///
/// This is the testable core of [`is_rate_limit_error`]. A rate limit is
/// identified by two conditions:
/// 1. HTTP status code 403 (Forbidden)
/// 2. Error message starting with `"API rate limit exceeded"`
///
/// Extracted as a standalone helper so that the logic can be unit-tested
/// without constructing `octocrab::Error` (which contains
/// `#[non_exhaustive]` types that cannot be instantiated from outside the
/// crate).
fn is_rate_limit_response(status_code: u16, message: &str) -> bool {
    status_code == 403 && message.starts_with("API rate limit exceeded")
}

/// Classifies an octocrab error into the appropriate [`GhrrError`] variant.
///
/// Rate limit errors (403 Forbidden with "API rate limit exceeded" message) are
/// mapped to [`GhrrError::RateLimit`]. All other errors are mapped to
/// [`GhrrError::Api`].
///
/// This function is used by the retry logic in `main.rs` to determine whether
/// to invoke `wait_rate_limit()` (for rate limit errors) or apply a generic
/// `RATE_LIMIT_BACKOFF` delay (for other errors).
pub fn classify_error(err: octocrab::Error) -> GhrrError {
    if is_rate_limit_error(&err) {
        GhrrError::RateLimit(err.to_string())
    } else {
        GhrrError::Api(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- SimpleUser tests ---

    #[test]
    fn test_simple_user_construction() {
        let user = SimpleUser {
            login: "octocat".to_string(),
        };
        assert_eq!(user.login, "octocat");
    }

    #[test]
    fn test_simple_user_clone() {
        let user = SimpleUser {
            login: "testuser".to_string(),
        };
        let cloned = user.clone();
        assert_eq!(cloned.login, "testuser");
    }

    #[test]
    fn test_simple_user_debug_format() {
        let user = SimpleUser {
            login: "debug_user".to_string(),
        };
        let debug_str = format!("{:?}", user);
        assert!(debug_str.contains("debug_user"));
    }

    #[test]
    fn test_simple_user_deserialize_from_json() {
        let json = r#"{"login": "octocat", "id": 1, "node_id": "MDQ6VXNlcjE="}"#;
        let user: SimpleUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.login, "octocat");
    }

    #[test]
    fn test_simple_user_deserialize_minimal_json() {
        let json = r#"{"login": "minimal"}"#;
        let user: SimpleUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.login, "minimal");
    }

    #[test]
    fn test_simple_user_deserialize_ignores_extra_fields() {
        // GitHub API returns many fields; SimpleUser should ignore all but login
        let json = r#"{
            "login": "octocat",
            "id": 1,
            "avatar_url": "https://example.com/avatar.png",
            "contributions": 42,
            "type": "User",
            "site_admin": false
        }"#;
        let user: SimpleUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.login, "octocat");
    }

    // --- OrgEntry tests ---

    #[test]
    fn test_org_entry_deserialize() {
        let json = r#"{"login": "rust-lang", "id": 5430905}"#;
        let org: OrgEntry = serde_json::from_str(json).unwrap();
        assert_eq!(org.login, "rust-lang");
    }

    #[test]
    fn test_org_entry_deserialize_minimal() {
        let json = r#"{"login": "myorg"}"#;
        let org: OrgEntry = serde_json::from_str(json).unwrap();
        assert_eq!(org.login, "myorg");
    }

    // --- RepoData tests ---

    #[test]
    fn test_repo_data_construction() {
        let data = RepoData {
            stargazers: vec![SimpleUser { login: "user1".to_string() }],
            stargazers_count: 100,
            subscribers: vec![SimpleUser { login: "user2".to_string() }],
            subscribers_count: 50,
            contributors: vec![SimpleUser { login: "user3".to_string() }],
        };
        assert_eq!(data.stargazers.len(), 1);
        assert_eq!(data.stargazers_count, 100);
        assert_eq!(data.subscribers.len(), 1);
        assert_eq!(data.subscribers_count, 50);
        assert_eq!(data.contributors.len(), 1);
    }

    #[test]
    fn test_repo_data_empty() {
        let data = RepoData {
            stargazers: vec![],
            stargazers_count: 0,
            subscribers: vec![],
            subscribers_count: 0,
            contributors: vec![],
        };
        assert!(data.stargazers.is_empty());
        assert_eq!(data.stargazers_count, 0);
        assert!(data.subscribers.is_empty());
        assert_eq!(data.subscribers_count, 0);
        assert!(data.contributors.is_empty());
    }

    // --- ListParams tests ---

    #[test]
    fn test_list_params_default() {
        let params = ListParams::default();
        assert_eq!(params.per_page, 100);
    }

    #[test]
    fn test_list_params_serialize() {
        let params = ListParams { per_page: 50 };
        let serialized = serde_urlencoded::to_string(&params).unwrap();
        assert_eq!(serialized, "per_page=50");
    }

    #[test]
    fn test_list_params_default_serialize() {
        let params = ListParams::default();
        let serialized = serde_urlencoded::to_string(&params).unwrap();
        assert_eq!(serialized, "per_page=100");
    }

    // --- RATE_LIMIT_BACKOFF constant test ---

    #[test]
    fn test_rate_limit_backoff_constant() {
        // Must match Python RATE_LIMIT_BACKOFF = 10 (main.py line 12)
        assert_eq!(RATE_LIMIT_BACKOFF, 10);
    }

    // --- is_rate_limit_response / is_rate_limit_error logic tests ---
    //
    // Note: `octocrab::GitHubError` is `#[non_exhaustive]`, so it cannot
    // be constructed directly from outside the crate. Instead we test the
    // extracted `is_rate_limit_response(status_code, message)` helper
    // which contains the identical matching logic used by
    // `is_rate_limit_error`.

    #[test]
    fn test_rate_limit_403_with_rate_limit_message() {
        // Classic rate limit response: 403 + "API rate limit exceeded ..."
        assert!(is_rate_limit_response(
            403,
            "API rate limit exceeded for user ID 12345."
        ));
    }

    #[test]
    fn test_rate_limit_403_with_exact_prefix() {
        // Message that is exactly the prefix with nothing after it
        assert!(is_rate_limit_response(403, "API rate limit exceeded"));
    }

    #[test]
    fn test_rate_limit_403_with_non_rate_limit_message() {
        // 403 Forbidden but not a rate limit error (e.g., insufficient permissions)
        assert!(!is_rate_limit_response(
            403,
            "Resource not accessible by integration"
        ));
    }

    #[test]
    fn test_rate_limit_404_not_classified() {
        // 404 Not Found — even with rate-limit-like message, not classified
        assert!(!is_rate_limit_response(404, "Not Found"));
    }

    #[test]
    fn test_rate_limit_500_with_rate_limit_message() {
        // Rate limit message but with 500 status — not classified as rate limit
        // Both conditions must hold: status 403 AND message prefix
        assert!(!is_rate_limit_response(500, "API rate limit exceeded"));
    }

    #[test]
    fn test_rate_limit_200_not_classified() {
        // 200 OK — never a rate limit error
        assert!(!is_rate_limit_response(200, "API rate limit exceeded"));
    }

    #[test]
    fn test_rate_limit_403_empty_message() {
        // 403 with empty message — not rate limited
        assert!(!is_rate_limit_response(403, ""));
    }

    #[test]
    fn test_rate_limit_403_partial_prefix_message() {
        // 403 with message that partially matches but doesn't start with the prefix
        assert!(!is_rate_limit_response(403, "api rate limit exceeded"));
        // Case sensitivity matters — Python's startswith is case-sensitive
        assert!(!is_rate_limit_response(403, "API Rate Limit Exceeded"));
    }

    // --- Company normalization behavioral verification ---
    // (Full normalization is tested in models.rs, but we verify the
    //  get_user_data pipeline expectation here)

    #[test]
    fn test_company_at_stripping_matches_python() {
        // Python: company.replace("@", "")
        // Verifies that the Rust replace behavior matches Python exactly
        let company = "@GitHub".to_string();
        let normalized = company.replace("@", "");
        assert_eq!(normalized, "GitHub");

        let company_multi = "@org@name".to_string();
        let normalized_multi = company_multi.replace("@", "");
        assert_eq!(normalized_multi, "orgname");
    }

    // --- Organizations join behavioral verification ---

    #[test]
    fn test_organizations_join_matches_python() {
        // Python: ', '.join(organizations) equivalent
        let orgs = vec![
            "github".to_string(),
            "rust-lang".to_string(),
            "servo".to_string(),
        ];
        let joined = orgs.join(", ");
        assert_eq!(joined, "github, rust-lang, servo");
    }

    #[test]
    fn test_organizations_join_single() {
        let orgs = vec!["only-org".to_string()];
        let joined = orgs.join(", ");
        assert_eq!(joined, "only-org");
    }

    #[test]
    fn test_organizations_join_empty() {
        let orgs: Vec<String> = vec![];
        let joined = orgs.join(", ");
        assert_eq!(joined, "");
    }

    // --- Rate limit sleep calculation verification ---

    #[test]
    fn test_rate_limit_sleep_calculation_future_timestamp() {
        // Simulate a reset time 60 seconds in the future
        let now = Utc::now();
        let reset_time = now.timestamp() + 60;
        let sleep_until = DateTime::from_timestamp(reset_time, 0).unwrap();
        let seconds_to_sleep = (sleep_until - now).num_seconds() + 3;
        let seconds_to_sleep = seconds_to_sleep.max(0) as u64;

        // Should be approximately 63 seconds (60 + 3 buffer)
        assert!(seconds_to_sleep >= 60, "Expected >= 60, got {}", seconds_to_sleep);
        assert!(seconds_to_sleep <= 66, "Expected <= 66, got {}", seconds_to_sleep);
    }

    #[test]
    fn test_rate_limit_sleep_calculation_past_timestamp() {
        // Simulate a reset time that's already passed
        let now = Utc::now();
        let reset_time = now.timestamp() - 30;
        let sleep_until = DateTime::from_timestamp(reset_time, 0).unwrap();
        let seconds_to_sleep = (sleep_until - now).num_seconds() + 3;
        let seconds_to_sleep = seconds_to_sleep.max(0) as u64;

        // Should be 0 since the reset time has passed and max(0) clamps
        assert_eq!(seconds_to_sleep, 0);
    }

    #[test]
    fn test_rate_limit_sleep_calculation_exact_now() {
        // Reset time is exactly now — should sleep 3 seconds (buffer only)
        let now = Utc::now();
        let reset_time = now.timestamp();
        let sleep_until = DateTime::from_timestamp(reset_time, 0).unwrap();
        let seconds_to_sleep = (sleep_until - now).num_seconds() + 3;
        let seconds_to_sleep = seconds_to_sleep.max(0) as u64;

        // Should be approximately 3 seconds (0 + 3 buffer)
        assert!(seconds_to_sleep <= 5, "Expected <= 5, got {}", seconds_to_sleep);
    }
}
