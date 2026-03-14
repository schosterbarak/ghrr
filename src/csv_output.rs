// src/csv_output.rs — CSV Writer and Output Handling for GHRR Application
//
// This module handles CSV output creation and writing, replacing the CSV-related
// logic from `ghrr/main.py`. It extracts:
//   - CSV writer creation logic (Python lines 124-133, 142-143)
//   - CSV header row writing (Python line 144)
//   - CSV data row writing per user (Python lines 87-96 inside `iterate_users()`)
//   - Date-stamped default filename generation (Python lines 125-127)
//
// Architecture:
//   The `CsvOutput` enum implements the Strategy pattern: output mode selection
//   (file vs. stdout) determines which `csv::Writer` variant is constructed.
//   Both variants share the same public API through `match` dispatch.
//
// Exported items:
//   - `CsvOutput` enum: CSV writer with `File` and `Stdout` variants
//     - `CsvOutput::from_path(path)` — create file-based writer
//     - `CsvOutput::from_stdout()` — create stdout-based writer
//     - `CsvOutput::default_filename(org, repo)` — generate date-stamped filename
//     - `CsvOutput::write_header()` — write CSV header row
//     - `CsvOutput::write_user(user, interaction)` — write user data row
//     - `CsvOutput::flush()` — flush underlying writer

use crate::models::User;
use anyhow::{Context, Result};
use chrono::Local;
use csv::Writer;
use std::fs::File;
use std::io::{self, Stdout};

/// CSV output handler with File and Stdout variants.
///
/// This enum replaces the Python output mode selection logic from `ghrr/main.py`
/// lines 124-133:
/// ```python
/// if not args.file:
///     output = open(file_name, mode='w')     # File variant
/// elif args.file == '-':
///     output = sys.stdout                     # Stdout variant
/// else:
///     output = open(args.file, mode='w')      # File variant
/// ```
///
/// The enum wraps a `csv::Writer<W>` specialized for each output target,
/// providing a unified interface for header writing, user data writing,
/// and flushing regardless of the output destination.
pub enum CsvOutput {
    /// File-based CSV output. The inner `Writer<File>` writes to a filesystem
    /// path, either the user-specified `--file` path or the auto-generated
    /// date-stamped filename (`ghusers_{org}_{repo}_{date}.csv`).
    File(Writer<File>),

    /// Stdout-based CSV output. The inner `Writer<Stdout>` writes directly to
    /// standard output, activated when the user passes `--file -` (or `-f -`).
    /// When this variant is active, progress bars must be suppressed (hidden)
    /// to avoid interleaving progress output with CSV data on the terminal.
    Stdout(Writer<Stdout>),
}

impl CsvOutput {
    /// Creates a new `CsvOutput::File` variant by opening a file at the given path.
    ///
    /// This replaces the Python logic at lines 128 and 133:
    /// ```python
    /// output = open(file_name, mode='w')   # default date-stamped filename
    /// output = open(args.file, mode='w')   # user-specified filename
    /// ```
    ///
    /// The `csv::Writer::from_path()` creates the file and wraps it in a
    /// buffered CSV writer. If the file cannot be created (e.g., permission
    /// denied, invalid path), an error with descriptive context is returned.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path for the output CSV file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created at the specified path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ghrr::csv_output::CsvOutput;
    /// let output = CsvOutput::from_path("output.csv").unwrap();
    /// ```
    pub fn from_path(path: &str) -> Result<Self> {
        let writer = Writer::from_path(path)
            .context(format!("Failed to create CSV file: {}", path))?;
        Ok(CsvOutput::File(writer))
    }

    /// Creates a new `CsvOutput::Stdout` variant that writes CSV data to stdout.
    ///
    /// This replaces the Python logic at line 130:
    /// ```python
    /// output = sys.stdout
    /// ```
    ///
    /// When using stdout mode (triggered by `--file -`), progress bars should
    /// be completely suppressed to avoid mixing progress output with CSV data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ghrr::csv_output::CsvOutput;
    /// let output = CsvOutput::from_stdout().unwrap();
    /// ```
    pub fn from_stdout() -> Result<Self> {
        let writer = Writer::from_writer(io::stdout());
        Ok(CsvOutput::Stdout(writer))
    }

    /// Generates a default date-stamped CSV filename.
    ///
    /// This replaces the Python filename generation logic at lines 125-127:
    /// ```python
    /// today = date.today()
    /// formated_today = today.strftime("%Y-%m-%d")  # YY-MM-DD
    /// file_name = 'ghusers_{}_{}_{}.csv'.format(org, repository, formated_today)
    /// ```
    ///
    /// The date format uses `%Y-%m-%d` (with dashes) to match the Python source
    /// exactly. Despite the Python comment saying "YY-MM-DD", the actual format
    /// string `"%Y-%m-%d"` produces `YYYY-MM-DD` (4-digit year with dashes).
    ///
    /// # Arguments
    ///
    /// * `org` - GitHub organization name
    /// * `repo` - GitHub repository name
    ///
    /// # Returns
    ///
    /// A filename string in the pattern `ghusers_{org}_{repo}_{YYYY-MM-DD}.csv`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ghrr::csv_output::CsvOutput;
    /// let filename = CsvOutput::default_filename("bridgecrewio", "checkov");
    /// assert!(filename.starts_with("ghusers_bridgecrewio_checkov_"));
    /// assert!(filename.ends_with(".csv"));
    /// // Example output: "ghusers_bridgecrewio_checkov_2020-09-21.csv"
    /// ```
    pub fn default_filename(org: &str, repo: &str) -> String {
        let today = Local::now();
        let formatted_date = today.format("%Y-%m-%d").to_string();
        format!("ghusers_{}_{}_{}.csv", org, repo, formatted_date)
    }

    /// Writes the CSV header row with the exact column names in the required order.
    ///
    /// This replaces the Python header writing at line 144:
    /// ```python
    /// user_writer.writerow(["username", "company", "organizations", "email",
    ///                        "location", "followers_count", "public_repos_count",
    ///                        "user_interaction"])
    /// ```
    ///
    /// The column headers MUST be in this exact order to maintain behavioral
    /// parity with the Python original:
    /// 1. `username`
    /// 2. `company`
    /// 3. `organizations`
    /// 4. `email`
    /// 5. `location`
    /// 6. `followers_count`
    /// 7. `public_repos_count`
    /// 8. `user_interaction`
    ///
    /// After writing the header, the writer is immediately flushed to ensure
    /// the data is written to the output target.
    ///
    /// # Errors
    ///
    /// Returns an error if the header row cannot be written or flushed.
    pub fn write_header(&mut self) -> Result<()> {
        let header = [
            "username",
            "company",
            "organizations",
            "email",
            "location",
            "followers_count",
            "public_repos_count",
            "user_interaction",
        ];
        match self {
            CsvOutput::File(w) => w
                .write_record(header)
                .context("Failed to write CSV header to file")?,
            CsvOutput::Stdout(w) => w
                .write_record(header)
                .context("Failed to write CSV header to stdout")?,
        }
        self.flush()?;
        Ok(())
    }

    /// Writes a single user data row to the CSV output.
    ///
    /// This replaces the Python row writing logic at lines 87-96 inside
    /// `iterate_users()`:
    /// ```python
    /// user_writer.writerow([
    ///     user.username,        # field 1
    ///     user.company,         # field 2
    ///     user.organizations,   # field 3
    ///     user.email,           # field 4
    ///     user.location,        # field 5
    ///     user.followers,       # field 6
    ///     user.repos,           # field 7
    ///     user_interaction      # field 8
    /// ])
    /// ```
    ///
    /// The `user_interaction` parameter is the stream type identifier
    /// ("stargazer", "subscriber", or "contributor") that is NOT part of the
    /// `User` struct but is passed separately per the sequential processing
    /// in `main.rs`.
    ///
    /// The `followers` and `repos` fields are `u64` integers on the `User`
    /// struct and are converted to strings for CSV output.
    ///
    /// After writing the row, the writer is immediately flushed to ensure
    /// data is written promptly (matching Python's `csv.writer.writerow()`
    /// behavior which writes directly to the underlying file object).
    ///
    /// # Arguments
    ///
    /// * `user` - Reference to the enriched user profile data
    /// * `user_interaction` - The interaction type: "stargazer", "subscriber",
    ///   or "contributor"
    ///
    /// # Errors
    ///
    /// Returns an error if the record cannot be written or flushed.
    pub fn write_user(&mut self, user: &User, user_interaction: &str) -> Result<()> {
        // Convert numeric fields to strings for CSV output.
        // These temporaries are needed so the record array can hold references
        // to String values with uniform lifetimes.
        let followers_str = user.followers.to_string();
        let repos_str = user.repos.to_string();

        let record = [
            user.username.as_str(),
            user.company.as_str(),
            user.organizations.as_str(),
            user.email.as_str(),
            user.location.as_str(),
            followers_str.as_str(),
            repos_str.as_str(),
            user_interaction,
        ];
        match self {
            CsvOutput::File(w) => w
                .write_record(record)
                .context("Failed to write user record to CSV file")?,
            CsvOutput::Stdout(w) => w
                .write_record(record)
                .context("Failed to write user record to CSV stdout")?,
        }
        self.flush()?;
        Ok(())
    }

    /// Flushes the underlying CSV writer to ensure all buffered data is written.
    ///
    /// This is called after every `write_header()` and `write_user()` call to
    /// match the Python behavior where `csv.writer.writerow()` writes data
    /// immediately to the underlying file object.
    ///
    /// For the `File` variant, this flushes to disk.
    /// For the `Stdout` variant, this flushes the stdout buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the flush operation fails (e.g., broken pipe,
    /// disk full).
    pub fn flush(&mut self) -> Result<()> {
        match self {
            CsvOutput::File(w) => w.flush().context("Failed to flush CSV file")?,
            CsvOutput::Stdout(w) => w.flush().context("Failed to flush CSV stdout")?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// Helper: create a test User with known values for predictable CSV output.
    fn make_test_user() -> User {
        User {
            username: "octocat".to_string(),
            company: "GitHub".to_string(),
            organizations: "github, rust-lang".to_string(),
            email: "octocat@github.com".to_string(),
            location: "San Francisco".to_string(),
            followers: 1000,
            repos: 42,
        }
    }

    // --- default_filename tests ---

    #[test]
    fn test_default_filename_format() {
        let filename = CsvOutput::default_filename("myorg", "myrepo");
        assert!(
            filename.starts_with("ghusers_myorg_myrepo_"),
            "Filename should start with 'ghusers_myorg_myrepo_', got: {}",
            filename
        );
        assert!(
            filename.ends_with(".csv"),
            "Filename should end with '.csv', got: {}",
            filename
        );
    }

    #[test]
    fn test_default_filename_contains_date_with_dashes() {
        let filename = CsvOutput::default_filename("org", "repo");
        // Extract the date portion: between last underscore and .csv
        let without_prefix = filename.strip_prefix("ghusers_org_repo_").unwrap();
        let date_part = without_prefix.strip_suffix(".csv").unwrap();
        // Date format should be YYYY-MM-DD (10 chars with dashes)
        assert_eq!(
            date_part.len(),
            10,
            "Date part should be 10 chars (YYYY-MM-DD), got: '{}'",
            date_part
        );
        assert_eq!(
            date_part.chars().nth(4),
            Some('-'),
            "5th char should be dash, got: '{}'",
            date_part
        );
        assert_eq!(
            date_part.chars().nth(7),
            Some('-'),
            "8th char should be dash, got: '{}'",
            date_part
        );
    }

    #[test]
    fn test_default_filename_uses_current_date() {
        let filename = CsvOutput::default_filename("testorg", "testrepo");
        let today = Local::now().format("%Y-%m-%d").to_string();
        let expected = format!("ghusers_testorg_testrepo_{}.csv", today);
        assert_eq!(filename, expected);
    }

    #[test]
    fn test_default_filename_special_characters_in_names() {
        // Organization and repo names with special characters should be
        // passed through as-is (URL encoding is not our responsibility)
        let filename = CsvOutput::default_filename("my-org", "my_repo");
        assert!(filename.starts_with("ghusers_my-org_my_repo_"));
    }

    // --- from_path tests ---

    #[test]
    fn test_from_path_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_output.csv");
        let path_str = path.to_str().unwrap();

        let result = CsvOutput::from_path(path_str);
        assert!(result.is_ok(), "from_path should succeed for valid path");
        // Verify the file was created
        assert!(path.exists(), "CSV file should exist after from_path");
    }

    #[test]
    fn test_from_path_invalid_directory() {
        let result = CsvOutput::from_path("/nonexistent/directory/file.csv");
        assert!(result.is_err(), "from_path should fail for invalid directory");
    }

    // --- from_stdout tests ---

    #[test]
    fn test_from_stdout_succeeds() {
        let result = CsvOutput::from_stdout();
        assert!(result.is_ok(), "from_stdout should always succeed");
    }

    // --- write_header tests ---

    #[test]
    fn test_write_header_column_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("header_test.csv");
        let path_str = path.to_str().unwrap();

        let mut output = CsvOutput::from_path(path_str).unwrap();
        output.write_header().unwrap();
        // Drop the writer to ensure all data is flushed
        drop(output);

        // Read back and verify
        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        let expected_header = "username,company,organizations,email,location,followers_count,public_repos_count,user_interaction\n";
        assert_eq!(
            content, expected_header,
            "Header row should match exact Python column order"
        );
    }

    #[test]
    fn test_write_header_has_exactly_8_columns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("header_count_test.csv");
        let path_str = path.to_str().unwrap();

        let mut output = CsvOutput::from_path(path_str).unwrap();
        output.write_header().unwrap();
        drop(output);

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        let header_line = content.trim();
        let columns: Vec<&str> = header_line.split(',').collect();
        assert_eq!(columns.len(), 8, "Header should have exactly 8 columns");
    }

    // --- write_user tests ---

    #[test]
    fn test_write_user_field_order() {
        // Use a user with a single organization (no commas) so naive
        // field inspection is reliable for a quick structural check.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user_test.csv");
        let path_str = path.to_str().unwrap();

        let user = User {
            username: "octocat".to_string(),
            company: "GitHub".to_string(),
            organizations: "github".to_string(),
            email: "octocat@github.com".to_string(),
            location: "San Francisco".to_string(),
            followers: 1000,
            repos: 42,
        };

        let mut output = CsvOutput::from_path(path_str).unwrap();
        output.write_user(&user, "stargazer").unwrap();
        drop(output);

        // Use csv reader for proper parsing (handles quoting correctly)
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(&path)
            .unwrap();

        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(record.len(), 8, "User record should have exactly 8 fields");
        assert_eq!(&record[0], "octocat", "Field 1: username");
        assert_eq!(&record[1], "GitHub", "Field 2: company");
        assert_eq!(&record[2], "github", "Field 3: organizations");
        assert_eq!(&record[3], "octocat@github.com", "Field 4: email");
        assert_eq!(&record[4], "San Francisco", "Field 5: location");
        assert_eq!(&record[5], "1000", "Field 6: followers_count");
        assert_eq!(&record[6], "42", "Field 7: public_repos_count");
        assert_eq!(&record[7], "stargazer", "Field 8: user_interaction");
    }

    #[test]
    fn test_write_user_csv_parsed_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user_csv_test.csv");
        let path_str = path.to_str().unwrap();

        let mut output = CsvOutput::from_path(path_str).unwrap();
        let user = make_test_user();
        output.write_user(&user, "stargazer").unwrap();
        drop(output);

        // Use csv reader to properly parse the output
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(&path)
            .unwrap();

        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(record.len(), 8, "Record should have 8 fields");
        assert_eq!(&record[0], "octocat", "Field 1: username");
        assert_eq!(&record[1], "GitHub", "Field 2: company");
        assert_eq!(
            &record[2], "github, rust-lang",
            "Field 3: organizations"
        );
        assert_eq!(&record[3], "octocat@github.com", "Field 4: email");
        assert_eq!(&record[4], "San Francisco", "Field 5: location");
        assert_eq!(&record[5], "1000", "Field 6: followers_count");
        assert_eq!(&record[6], "42", "Field 7: public_repos_count");
        assert_eq!(&record[7], "stargazer", "Field 8: user_interaction");
    }

    #[test]
    fn test_write_user_different_interactions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("interactions_test.csv");
        let path_str = path.to_str().unwrap();

        let mut output = CsvOutput::from_path(path_str).unwrap();
        let user = make_test_user();

        output.write_user(&user, "stargazer").unwrap();
        output.write_user(&user, "subscriber").unwrap();
        output.write_user(&user, "contributor").unwrap();
        drop(output);

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(&path)
            .unwrap();

        let records: Vec<csv::StringRecord> =
            reader.records().map(|r| r.unwrap()).collect();
        assert_eq!(records.len(), 3, "Should have 3 records");
        assert_eq!(&records[0][7], "stargazer");
        assert_eq!(&records[1][7], "subscriber");
        assert_eq!(&records[2][7], "contributor");
    }

    #[test]
    fn test_write_user_with_empty_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty_fields_test.csv");
        let path_str = path.to_str().unwrap();

        let user = User {
            username: "ghost".to_string(),
            company: String::new(),
            organizations: String::new(),
            email: String::new(),
            location: String::new(),
            followers: 0,
            repos: 0,
        };

        let mut output = CsvOutput::from_path(path_str).unwrap();
        output.write_user(&user, "stargazer").unwrap();
        drop(output);

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(&path)
            .unwrap();

        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(&record[0], "ghost", "Username should be present");
        assert_eq!(&record[1], "", "Empty company");
        assert_eq!(&record[2], "", "Empty organizations");
        assert_eq!(&record[3], "", "Empty email");
        assert_eq!(&record[4], "", "Empty location");
        assert_eq!(&record[5], "0", "Zero followers");
        assert_eq!(&record[6], "0", "Zero repos");
        assert_eq!(&record[7], "stargazer", "Interaction type");
    }

    #[test]
    fn test_write_header_then_user() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("full_csv_test.csv");
        let path_str = path.to_str().unwrap();

        let mut output = CsvOutput::from_path(path_str).unwrap();
        output.write_header().unwrap();
        let user = make_test_user();
        output.write_user(&user, "contributor").unwrap();
        drop(output);

        // Read with headers enabled
        let mut reader = csv::Reader::from_path(&path).unwrap();
        let headers = reader.headers().unwrap().clone();
        assert_eq!(headers.len(), 8);
        assert_eq!(&headers[0], "username");
        assert_eq!(&headers[7], "user_interaction");

        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(&record[0], "octocat");
        assert_eq!(&record[7], "contributor");
    }

    // --- flush tests ---

    #[test]
    fn test_flush_file_variant() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("flush_test.csv");
        let path_str = path.to_str().unwrap();

        let mut output = CsvOutput::from_path(path_str).unwrap();
        output.write_header().unwrap();
        // flush() is already called by write_header, but test explicit flush
        let flush_result = output.flush();
        assert!(flush_result.is_ok(), "Explicit flush should succeed");
    }

    #[test]
    fn test_write_user_with_large_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large_numbers_test.csv");
        let path_str = path.to_str().unwrap();

        let user = User {
            username: "popular".to_string(),
            company: "BigCorp".to_string(),
            organizations: "org1".to_string(),
            email: "pop@example.com".to_string(),
            location: "Everywhere".to_string(),
            followers: 999999999,
            repos: 12345,
        };

        let mut output = CsvOutput::from_path(path_str).unwrap();
        output.write_user(&user, "subscriber").unwrap();
        drop(output);

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(&path)
            .unwrap();

        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(&record[5], "999999999", "Large followers count");
        assert_eq!(&record[6], "12345", "Repos count");
    }

    #[test]
    fn test_write_user_with_special_csv_characters() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("special_chars_test.csv");
        let path_str = path.to_str().unwrap();

        // Test that CSV properly handles fields with commas, quotes, etc.
        let user = User {
            username: "user_with_special".to_string(),
            company: "Company, Inc.".to_string(),
            organizations: "org1, org2, org3".to_string(),
            email: "user@example.com".to_string(),
            location: "City, State".to_string(),
            followers: 10,
            repos: 5,
        };

        let mut output = CsvOutput::from_path(path_str).unwrap();
        output.write_user(&user, "stargazer").unwrap();
        drop(output);

        // Use csv reader to verify proper quoting/escaping
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(&path)
            .unwrap();

        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(&record[1], "Company, Inc.", "Company with comma");
        assert_eq!(
            &record[2], "org1, org2, org3",
            "Organizations with commas"
        );
        assert_eq!(&record[4], "City, State", "Location with comma");
    }
}
