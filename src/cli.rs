//! CLI argument definitions for the GHRR (GitHub Research Runner) application.
//!
//! This module defines the command-line interface using `clap` with derive macros,
//! replacing the Python `argparse` setup from `ghrr/main.py` (lines 16-22).
//! The CLI interface contract is maintained exactly as specified in AAP §0.7.1:
//!
//! - `-o` / `--organization` (required): GitHub organization name
//! - `-r` / `--repository` (required): GitHub repository name
//! - `-f` / `--file` (optional): Output file path; `-` means stdout
//! - `--version`: Prints version from Cargo.toml (0.0.1)

use clap::Parser;

/// stargazers crawler
///
/// A command-line utility that collects stargazer, subscriber, and contributor
/// data from any GitHub repository and exports enriched user profiles to CSV.
#[derive(Parser, Debug)]
#[command(name = "ghrr", version, about = "stargazers crawler")]
pub struct Args {
    /// github organization
    #[arg(short = 'o', long = "organization")]
    pub organization: String,

    /// github repository
    #[arg(short = 'r', long = "repository")]
    pub repository: String,

    /// output file path
    #[arg(short = 'f', long = "file")]
    pub file: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Verify that required args (-o and -r) are parsed correctly
    /// and that optional file arg defaults to None.
    #[test]
    fn test_parse_required_args() {
        let args = Args::parse_from(["ghrr", "-o", "myorg", "-r", "myrepo"]);
        assert_eq!(args.organization, "myorg");
        assert_eq!(args.repository, "myrepo");
        assert!(args.file.is_none());
    }

    /// Verify that the -f flag with a file path is parsed correctly.
    #[test]
    fn test_parse_with_file() {
        let args = Args::parse_from([
            "ghrr", "-o", "myorg", "-r", "myrepo", "-f", "output.csv",
        ]);
        assert_eq!(args.organization, "myorg");
        assert_eq!(args.repository, "myrepo");
        assert_eq!(args.file, Some("output.csv".to_string()));
    }

    /// Verify that -f - is accepted for stdout output mode.
    /// This is a critical behavioral parity requirement: when the user passes
    /// `-f -`, CSV output goes to stdout and progress bars are suppressed.
    #[test]
    fn test_parse_with_stdout() {
        let args = Args::parse_from(["ghrr", "-o", "myorg", "-r", "myrepo", "-f", "-"]);
        assert_eq!(args.organization, "myorg");
        assert_eq!(args.repository, "myrepo");
        assert_eq!(args.file, Some("-".to_string()));
    }

    /// Verify that long-form argument names work correctly.
    #[test]
    fn test_parse_long_args() {
        let args = Args::parse_from([
            "ghrr",
            "--organization",
            "myorg",
            "--repository",
            "myrepo",
            "--file",
            "out.csv",
        ]);
        assert_eq!(args.organization, "myorg");
        assert_eq!(args.repository, "myrepo");
        assert_eq!(args.file, Some("out.csv".to_string()));
    }

    /// Verify that missing the required -o/--organization flag causes a parse error.
    #[test]
    fn test_missing_required_org() {
        let result = Args::try_parse_from(["ghrr", "-r", "myrepo"]);
        assert!(result.is_err());
    }

    /// Verify that missing the required -r/--repository flag causes a parse error.
    #[test]
    fn test_missing_required_repo() {
        let result = Args::try_parse_from(["ghrr", "-o", "myorg"]);
        assert!(result.is_err());
    }

    /// Verify that providing no arguments at all causes a parse error.
    #[test]
    fn test_missing_all_required_args() {
        let result = Args::try_parse_from(["ghrr"]);
        assert!(result.is_err());
    }

    /// Verify that mixed short and long flags work together.
    #[test]
    fn test_mixed_short_and_long_flags() {
        let args = Args::parse_from([
            "ghrr",
            "-o",
            "myorg",
            "--repository",
            "myrepo",
            "-f",
            "data.csv",
        ]);
        assert_eq!(args.organization, "myorg");
        assert_eq!(args.repository, "myrepo");
        assert_eq!(args.file, Some("data.csv".to_string()));
    }

    /// Verify that argument values with special characters are handled.
    #[test]
    fn test_args_with_special_characters() {
        let args = Args::parse_from([
            "ghrr",
            "-o",
            "my-org_123",
            "-r",
            "my-repo.name",
            "-f",
            "/tmp/path/to/output file.csv",
        ]);
        assert_eq!(args.organization, "my-org_123");
        assert_eq!(args.repository, "my-repo.name");
        assert_eq!(args.file, Some("/tmp/path/to/output file.csv".to_string()));
    }

    /// Verify that the --version flag is recognized (try_parse_from returns
    /// an error of kind DisplayVersion, which indicates the version was requested).
    #[test]
    fn test_version_flag() {
        let result = Args::try_parse_from(["ghrr", "--version"]);
        assert!(result.is_err());
        // clap returns an error with kind DisplayVersion for --version
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    /// Verify that the --help flag is recognized.
    #[test]
    fn test_help_flag() {
        let result = Args::try_parse_from(["ghrr", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    /// Verify that an unknown flag causes an error.
    #[test]
    fn test_unknown_flag() {
        let result = Args::try_parse_from(["ghrr", "--unknown", "value"]);
        assert!(result.is_err());
    }

    /// Verify that the organization field cannot be empty (clap requires
    /// a non-empty value after the flag).
    #[test]
    fn test_org_flag_without_value() {
        let result = Args::try_parse_from(["ghrr", "-o"]);
        assert!(result.is_err());
    }

    /// Verify that the file flag with an empty-like path still parses
    /// (clap accepts any string value including empty-looking ones).
    #[test]
    fn test_file_with_relative_path() {
        let args = Args::parse_from([
            "ghrr", "-o", "org", "-r", "repo", "-f", "./relative/path.csv",
        ]);
        assert_eq!(args.file, Some("./relative/path.csv".to_string()));
    }
}
