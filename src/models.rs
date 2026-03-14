// src/models.rs — Data Structures for GHRR Application
//
// This module defines the core data model types for the GHRR (GitHub Research Runner)
// application, replacing the Python `User` namedtuple and associated data enrichment
// logic from `ghrr/main.py`.
//
// Exported items:
//   - `User` struct: 7-field record representing an enriched GitHub user profile
//   - `normalize_company()`: Strips all '@' characters from company names
//   - `join_organizations()`: Joins a list of organization names with ", " separator

use serde::Serialize;

/// Represents an enriched GitHub user profile with all collected data fields.
///
/// This struct replaces the Python `User` namedtuple defined at line 27 of `ghrr/main.py`:
///   `User = namedtuple('User', ['email', 'location', 'username', 'company', 'followers', 'repos', 'organizations'])`
///
/// The `Serialize` derive enables direct CSV serialization via the `csv` crate's serde
/// integration.
///
/// # Field Mapping from Python
///
/// Fields are ordered to match the required CSV column header order per AAP §0.7.1:
/// `username, company, organizations, email, location, followers_count, public_repos_count, user_interaction`
/// (The `user_interaction` column is a per-stream property — stargazer, subscriber, or contributor —
/// and is added during CSV writing in `csv_output.rs`, not stored on the User struct.)
///
/// | CSV Column           | Rust Field      | Serde Name             | Source in get_user_data()              |
/// |----------------------|-----------------|------------------------|----------------------------------------|
/// | `username`           | `username`      | `username`             | `u.login`                              |
/// | `company`            | `company`       | `company`              | `gh_user.company` with '@' stripped     |
/// | `organizations`      | `organizations` | `organizations`        | Joined org login names (", " separator)|
/// | `email`              | `email`         | `email`                | `gh_user.email` (may be None → "")     |
/// | `location`           | `location`      | `location`             | `gh_user.location` (may be None → "")  |
/// | `followers_count`    | `followers`     | `followers_count`      | `gh_user.followers_count`              |
/// | `public_repos_count` | `repos`         | `public_repos_count`   | `gh_user.public_repos_count`           |
#[derive(Debug, Clone, Serialize)]
pub struct User {
    /// GitHub username (login handle).
    pub username: String,

    /// GitHub user's company affiliation with all '@' characters removed.
    /// For example, "@Microsoft" becomes "Microsoft".
    pub company: String,

    /// Comma-separated list of organization names the user belongs to.
    /// For example, "org1, org2, org3". Empty string if user has no organizations.
    pub organizations: String,

    /// GitHub user's email address. Empty string if not publicly available.
    pub email: String,

    /// GitHub user's listed location. Empty string if not set.
    pub location: String,

    /// Number of followers the user has on GitHub. Always non-negative.
    /// Serialized as `followers_count` to match the required CSV column header.
    #[serde(rename = "followers_count")]
    pub followers: u64,

    /// Number of public repositories the user owns. Always non-negative.
    /// Serialized as `public_repos_count` to match the required CSV column header.
    #[serde(rename = "public_repos_count")]
    pub repos: u64,
}

impl User {
    /// Constructs a new `User` with automatic normalization of optional fields.
    ///
    /// This constructor encapsulates the data enrichment logic from Python's
    /// `get_user_data()` function (lines 29-41 of `ghrr/main.py`), specifically:
    ///   - Converting `None` email/location to empty strings
    ///   - Stripping all '@' characters from the company name
    ///   - Joining organization names with ", " separator
    ///
    /// # Arguments
    ///
    /// * `email` - User's email, or `None` if not publicly available
    /// * `location` - User's location, or `None` if not set
    /// * `username` - GitHub login handle (always present)
    /// * `company` - User's company affiliation, or `None` if not set
    /// * `followers` - Number of followers
    /// * `repos` - Number of public repositories
    /// * `organizations` - List of organization login names the user belongs to
    ///
    /// # Examples
    ///
    /// ```
    /// use ghrr::models::User;
    ///
    /// let user = User::new(
    ///     Some("user@example.com".to_string()),
    ///     Some("San Francisco".to_string()),
    ///     "octocat".to_string(),
    ///     Some("@GitHub".to_string()),
    ///     1000,
    ///     42,
    ///     vec!["github".to_string(), "rust-lang".to_string()],
    /// );
    /// assert_eq!(user.company, "GitHub");
    /// assert_eq!(user.organizations, "github, rust-lang");
    /// ```
    pub fn new(
        email: Option<String>,
        location: Option<String>,
        username: String,
        company: Option<String>,
        followers: u64,
        repos: u64,
        organizations: Vec<String>,
    ) -> Self {
        Self {
            email: email.unwrap_or_default(),
            location: location.unwrap_or_default(),
            username,
            company: normalize_company(company.as_deref()),
            followers,
            repos,
            organizations: join_organizations(&organizations),
        }
    }
}

/// Normalizes a company name by removing all '@' characters.
///
/// This replicates the Python logic from `ghrr/main.py` lines 35-36:
/// ```python
/// if company:
///     company = company.replace("@", "")
/// ```
///
/// Note: Python's `str.replace("@", "")` removes ALL occurrences of '@',
/// not just a leading one. The Rust `str::replace` method has identical behavior.
///
/// # Arguments
///
/// * `company` - The raw company name from GitHub's API, or `None` if not set
///
/// # Returns
///
/// A `String` with all '@' characters removed, or an empty string if `None`.
///
/// # Examples
///
/// ```
/// use ghrr::models::normalize_company;
///
/// assert_eq!(normalize_company(Some("@Microsoft")), "Microsoft");
/// assert_eq!(normalize_company(Some("@org@name")), "orgname");
/// assert_eq!(normalize_company(Some("Google")), "Google");
/// assert_eq!(normalize_company(None), "");
/// ```
pub fn normalize_company(company: Option<&str>) -> String {
    match company {
        Some(c) => c.replace('@', ""),
        None => String::new(),
    }
}

/// Joins a slice of organization names into a single comma-separated string.
///
/// This replicates the Python logic from `ghrr/main.py` lines 37-40 where
/// organization login names are collected into a list. The Rust version
/// pre-joins them with ", " (comma + space) for clean CSV output.
///
/// # Arguments
///
/// * `orgs` - Slice of organization login names
///
/// # Returns
///
/// A single `String` with organization names separated by ", ".
/// Returns an empty string if the slice is empty.
///
/// # Examples
///
/// ```
/// use ghrr::models::join_organizations;
///
/// let orgs = vec!["org1".to_string(), "org2".to_string()];
/// assert_eq!(join_organizations(&orgs), "org1, org2");
///
/// let empty: Vec<String> = vec![];
/// assert_eq!(join_organizations(&empty), "");
/// ```
pub fn join_organizations(orgs: &[String]) -> String {
    orgs.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalize_company tests ---

    #[test]
    fn test_normalize_company_with_leading_at() {
        // Python behavior: "@Microsoft" -> "Microsoft"
        assert_eq!(normalize_company(Some("@Microsoft")), "Microsoft");
    }

    #[test]
    fn test_normalize_company_multiple_at_symbols() {
        // Python behavior: "@org@name" -> "orgname" (all @ removed)
        assert_eq!(normalize_company(Some("@org@name")), "orgname");
    }

    #[test]
    fn test_normalize_company_none_input() {
        // Python behavior: None -> "" (empty string for csv output)
        assert_eq!(normalize_company(None), "");
    }

    #[test]
    fn test_normalize_company_no_at_symbol() {
        // Company without @ should remain unchanged
        assert_eq!(normalize_company(Some("Google")), "Google");
    }

    #[test]
    fn test_normalize_company_empty_string() {
        // Empty string input returns empty string
        assert_eq!(normalize_company(Some("")), "");
    }

    #[test]
    fn test_normalize_company_only_at_symbol() {
        // A company name that is just "@" should become empty
        assert_eq!(normalize_company(Some("@")), "");
    }

    #[test]
    fn test_normalize_company_at_in_middle() {
        // "@" in the middle of company name: "My@Corp" -> "MyCorp"
        assert_eq!(normalize_company(Some("My@Corp")), "MyCorp");
    }

    #[test]
    fn test_normalize_company_whitespace_preserved() {
        // Whitespace should be preserved; only @ is removed
        assert_eq!(normalize_company(Some("@ My Company")), " My Company");
    }

    // --- join_organizations tests ---

    #[test]
    fn test_join_organizations_multiple() {
        let orgs = vec![
            "org1".to_string(),
            "org2".to_string(),
            "org3".to_string(),
        ];
        assert_eq!(join_organizations(&orgs), "org1, org2, org3");
    }

    #[test]
    fn test_join_organizations_single() {
        let orgs = vec!["org1".to_string()];
        assert_eq!(join_organizations(&orgs), "org1");
    }

    #[test]
    fn test_join_organizations_empty() {
        let orgs: Vec<String> = vec![];
        assert_eq!(join_organizations(&orgs), "");
    }

    #[test]
    fn test_join_organizations_two() {
        let orgs = vec!["alpha".to_string(), "beta".to_string()];
        assert_eq!(join_organizations(&orgs), "alpha, beta");
    }

    #[test]
    fn test_join_organizations_with_special_chars() {
        // Organization names may contain hyphens, dots, etc.
        let orgs = vec!["rust-lang".to_string(), "org.name".to_string()];
        assert_eq!(join_organizations(&orgs), "rust-lang, org.name");
    }

    // --- User::new() tests ---

    #[test]
    fn test_user_new_with_all_fields() {
        let user = User::new(
            Some("test@email.com".to_string()),
            Some("New York".to_string()),
            "testuser".to_string(),
            Some("@TestCorp".to_string()),
            100,
            50,
            vec!["org1".to_string(), "org2".to_string()],
        );
        assert_eq!(user.email, "test@email.com");
        assert_eq!(user.location, "New York");
        assert_eq!(user.username, "testuser");
        assert_eq!(user.company, "TestCorp"); // '@' stripped by normalize_company
        assert_eq!(user.followers, 100);
        assert_eq!(user.repos, 50);
        assert_eq!(user.organizations, "org1, org2");
    }

    #[test]
    fn test_user_new_with_none_optional_fields() {
        let user = User::new(
            None,
            None,
            "testuser".to_string(),
            None,
            0,
            0,
            vec![],
        );
        assert_eq!(user.email, "");
        assert_eq!(user.location, "");
        assert_eq!(user.username, "testuser");
        assert_eq!(user.company, "");
        assert_eq!(user.followers, 0);
        assert_eq!(user.repos, 0);
        assert_eq!(user.organizations, "");
    }

    #[test]
    fn test_user_new_normalizes_company_automatically() {
        // Verify that User::new applies normalize_company internally
        let user = User::new(
            None,
            None,
            "dev".to_string(),
            Some("@BigCorp@Inc".to_string()),
            10,
            5,
            vec![],
        );
        assert_eq!(user.company, "BigCorpInc"); // Both '@' removed
    }

    #[test]
    fn test_user_new_joins_organizations_automatically() {
        // Verify that User::new applies join_organizations internally
        let user = User::new(
            None,
            None,
            "dev".to_string(),
            None,
            0,
            0,
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );
        assert_eq!(user.organizations, "a, b, c");
    }

    #[test]
    fn test_user_new_email_at_preserved() {
        // '@' in email should NOT be affected (normalize_company only touches company)
        let user = User::new(
            Some("user@example.com".to_string()),
            None,
            "user".to_string(),
            None,
            0,
            0,
            vec![],
        );
        assert_eq!(user.email, "user@example.com");
    }

    #[test]
    fn test_user_new_large_counts() {
        // Verify u64 handles large follower/repo counts
        let user = User::new(
            None,
            None,
            "popular_dev".to_string(),
            None,
            1_000_000,
            10_000,
            vec![],
        );
        assert_eq!(user.followers, 1_000_000);
        assert_eq!(user.repos, 10_000);
    }

    // --- Trait derivation tests ---

    #[test]
    fn test_user_debug_trait() {
        // Verify Debug derive works (used for logging/error messages)
        let user = User::new(
            None,
            None,
            "debuguser".to_string(),
            None,
            0,
            0,
            vec![],
        );
        let debug_output = format!("{:?}", user);
        assert!(debug_output.contains("debuguser"));
    }

    #[test]
    fn test_user_clone_trait() {
        // Verify Clone derive works
        let user = User::new(
            Some("email@test.com".to_string()),
            Some("London".to_string()),
            "cloneuser".to_string(),
            Some("@CloneCorp".to_string()),
            42,
            7,
            vec!["org1".to_string()],
        );
        let cloned = user.clone();
        assert_eq!(cloned.email, user.email);
        assert_eq!(cloned.location, user.location);
        assert_eq!(cloned.username, user.username);
        assert_eq!(cloned.company, user.company);
        assert_eq!(cloned.followers, user.followers);
        assert_eq!(cloned.repos, user.repos);
        assert_eq!(cloned.organizations, user.organizations);
    }

    #[test]
    fn test_user_serialize_trait() {
        // Verify Serialize derive works by serializing to JSON
        // (This validates the serde::Serialize derive is functioning)
        let user = User::new(
            Some("ser@test.com".to_string()),
            Some("Berlin".to_string()),
            "seruser".to_string(),
            Some("@SerCo".to_string()),
            5,
            3,
            vec!["serorg".to_string()],
        );
        let json = serde_json::to_string(&user).expect("Serialize to JSON should succeed");
        assert!(json.contains("\"username\":\"seruser\""));
        assert!(json.contains("\"company\":\"SerCo\""));
        assert!(json.contains("\"organizations\":\"serorg\""));
    }
}
