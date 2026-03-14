# Technical Specification

# 0. Agent Action Plan

## 0.1 Intent Clarification

### 0.1.1 Core Refactoring Objective

Based on the prompt, the Blitzy platform understands that the refactoring objective is to perform a **complete tech stack migration** of the GHRR (GitHub Research Runner) CLI application from its current Python implementation to Rust. The project — a command-line utility that collects stargazer, subscriber, and contributor data from any GitHub repository and exports enriched user profiles to CSV — must be entirely rewritten in idiomatic Rust while preserving all existing functional behavior, CLI interface contracts, and output formats.

- **Refactoring type:** Tech stack migration (Python → Rust)
- **Target repository:** Same repository (in-place replacement of the Python codebase with a Rust codebase)
- **Primary goals:**
  - Rewrite the entire GHRR codebase from Python 3 to Rust
  - Produce a fully compilable, runnable, and testable Rust binary that replicates all GHRR functionality
  - Establish a complete Cargo-based build process with `cargo build`, `cargo test`, and `cargo run` support
  - Implement comprehensive unit tests and integration tests that validate behavior parity with the Python original
  - Replace all Python dependencies with idiomatic Rust crate equivalents
  - Maintain identical CLI argument interface (`--organization`, `--repository`, `--file`)
  - Preserve the CSV output format (`ghusers_{ORG}_{REPO}_{DATE}.csv`) with the same column structure
  - Retain the same environment variable authentication model (`GITHUB_USER`, `GITHUB_TOKEN`)

- **Implicit requirements surfaced:**
  - Maintain identical API compatibility with the GitHub REST API v3 endpoints currently consumed
  - Preserve the rate-limit handling and retry-with-backoff resilience behavior
  - Keep the progress bar output on stderr and CSV streaming on stdout when `-f -` is specified
  - Preserve the company name normalization logic (stripping `@` prefix)
  - Maintain the organization membership aggregation per user
  - Ensure the date-stamped default filename convention is replicated exactly

### 0.1.2 Technical Interpretation

This refactoring translates to the following technical transformation strategy:

- **Current architecture:** A monolithic, single-threaded, script-style Python CLI (`ghrr/main.py`, ~148 lines) using `github3.py` for GitHub API access, `tqdm` for progress bars, and Python stdlib modules (`argparse`, `csv`, `os`, `sys`, `datetime`, `collections`, `time`) for core logic
- **Target architecture:** A modular, idiomatic Rust CLI application using `octocrab` for GitHub API access, `clap` for argument parsing, `indicatif` for progress reporting, `csv` crate for output, `tokio` for async runtime, and `chrono` for date/time handling — structured into distinct modules per concern (CLI, authentication, API client, data models, output formatting, error handling)
- **Transformation rules:**
  - Python `argparse` → Rust `clap` with derive macros
  - Python `github3.py` → Rust `octocrab` (async GitHub API client)
  - Python `tqdm` → Rust `indicatif` progress bars
  - Python `csv.writer` → Rust `csv::Writer`
  - Python `collections.namedtuple` → Rust `struct` with `serde::Serialize`
  - Python `os.getenv()` → Rust `std::env::var()`
  - Python `time.sleep()` → Rust `tokio::time::sleep()`
  - Python `datetime` → Rust `chrono`
  - Python exception handling → Rust `Result<T, E>` with `anyhow`/`thiserror`
  - Python infinite retry loops → Rust `loop` with async/await and typed error handling

### 0.1.3 Success Criteria

Success for this refactor is defined as:

- A complete Rust project that compiles without errors using `cargo build`
- A full test suite runnable via `cargo test` covering unit tests for all modules and integration tests for end-to-end workflow
- The produced binary accepts the same CLI arguments and produces identical CSV output as the Python original
- Rate limiting, retry logic, and error handling behave equivalently to the Python implementation
- The project includes a properly configured `Cargo.toml` with all dependencies, a `README.md` reflecting the Rust build and usage instructions, and development tooling configuration

## 0.2 Source Analysis

### 0.2.1 Comprehensive Source File Discovery

The GHRR repository is a compact Python CLI application containing 10 source and configuration files. Every file in the repository requires either transformation, replacement, or removal as part of the Python-to-Rust migration.

**Current Repository Structure:**

```
ghrr/                          (repository root)
├── LICENSE                    (Apache 2.0, 202 lines — retain as-is)
├── README.md                  (project documentation — rewrite for Rust)
├── requirements.txt           (15 pinned PyPI packages — replace with Cargo.toml)
├── setup.py                   (setuptools packaging — replace with Cargo.toml)
├── .devcontainer/
│   └── devcontainer.json      (Python 3.10 Bullseye dev container — update for Rust)
├── .vscode/
│   ├── launch.json            (Python debug config — update for Rust)
│   ├── settings.json          (Python venv settings — update for Rust)
│   └── tasks.json             (pip install task — update for cargo)
└── ghrr/
    ├── __init__.py            (empty package marker — remove)
    ├── main.py                (core CLI logic, 148 lines — full rewrite to Rust)
    └── version.py             (version constant — replace with Cargo.toml version)
```

### 0.2.2 Source File Inventory

The following table enumerates every source file requiring transformation:

| Source File | Lines | Role | Migration Action |
|---|---|---|---|
| `ghrr/main.py` | 148 | Core application logic: CLI parsing, GitHub API interaction, CSV output, rate limiting, retry logic, progress reporting | Full rewrite to Rust modules |
| `ghrr/version.py` | 1 | Version constant (`version = '0.0.1'`) | Replace with `Cargo.toml` version field |
| `ghrr/__init__.py` | 0 | Python package marker (empty) | Remove entirely (not needed in Rust) |
| `setup.py` | ~30 | Python packaging with setuptools, metadata, install_requires | Replace with `Cargo.toml` |
| `requirements.txt` | 15 | Pinned PyPI dependencies | Replace with Cargo.toml `[dependencies]` |
| `README.md` | ~50 | Usage documentation for Python installation and CLI | Rewrite with Rust build/install/usage instructions |
| `.devcontainer/devcontainer.json` | ~15 | Dev container using Python 3.10 Bullseye image | Update to Rust dev container image |
| `.vscode/launch.json` | ~15 | Python debugger launch configuration | Update for Rust debugging (lldb/codelldb) |
| `.vscode/settings.json` | ~10 | Python venv path, file exclusions | Update for Rust/Cargo settings |
| `.vscode/tasks.json` | ~20 | Shell task for `pip install -r requirements.txt` | Update for `cargo build` / `cargo test` |
| `LICENSE` | 202 | Apache License 2.0 full text | Retain unchanged |

### 0.2.3 Core Logic Analysis — `ghrr/main.py`

The single substantive source file (`ghrr/main.py`) contains the following functional components that must each be mapped to Rust equivalents:

- **Imports and Constants:** `RATE_LIMIT_BACKOFF = 10`, env var reads for `GITHUB_USER` and `GITHUB_TOKEN`
- **CLI Argument Parsing:** `argparse.ArgumentParser` with `-o/--organization`, `-r/--repository`, `-f/--file` arguments and `--version`
- **Data Model:** `User = namedtuple('User', ['email', 'location', 'username', 'company', 'followers', 'repos', 'organizations'])` — a 7-field named tuple representing enriched user profiles
- **User Data Enrichment (`get_user_data`):** Takes lightweight user references from listing endpoints, calls `gh.user(user.login)` for full profile, aggregates `user.organizations()` memberships into a joined string, strips `@` characters from company names
- **Parameter Validation (`validate_params`):** Verifies `GITHUB_USER` and `GITHUB_TOKEN` are set; exits with error if missing
- **Progress Bar Stubs:** `DummyUpdater` and `DummyProgress` classes suppress progress bar output when writing to stdout (`-f -` mode)
- **Rate Limit Handler (`wait_rate_limit`):** Reads `X-RateLimit-Reset` header, computes sleep duration with 3-second buffer, sleeps until reset
- **User Iterator (`iterate_users`):** Core loop that iterates over user listing (stargazers/subscribers/contributors), enriches each with `get_user_data`, handles `ForbiddenError` with `wait_rate_limit`, handles generic errors with infinite retry and `RATE_LIMIT_BACKOFF` delay, writes CSV rows immediately
- **Main Execution Block (`__main__`):** Parses args → validates params → calls `github3.login()` → resolves repository (with retry loop) → creates CSV writer (file or stdout) → iterates stargazers, subscribers, contributors sequentially

### 0.2.4 Python Dependency Graph

The Python source depends on 15 pinned packages. Two are direct functional dependencies; the remaining 13 are transitive:

- **Direct:** `github3.py` (3.2.0, GitHub API wrapper), `tqdm` (4.64.1, progress bars)
- **Transitive through github3.py:** `requests` (2.28.1), `uritemplate` (4.1.1), `PyJWT` (2.6.0), `cryptography` (38.0.3), `cffi` (1.15.1), `pycparser` (2.21), `python-dateutil` (2.8.2), `six` (1.16.0), `lxml` (4.9.1)
- **Shared transitive:** `certifi` (2022.9.24), `charset-normalizer` (2.1.1), `idna` (3.4), `urllib3` (1.26.12)

**Version discrepancy identified:** `setup.py` pins `github3.py==2.3.0` for installation while `requirements.txt` pins `github3.py==3.2.0` for development — a major version discrepancy that the Rust migration resolves entirely by replacing with `octocrab`.

## 0.3 Scope Boundaries

### 0.3.1 Exhaustively In Scope

**Source transformations (Python → Rust rewrite):**
- `ghrr/main.py` — Full decomposition into modular Rust source files under `src/`
- `ghrr/version.py` — Version constant migrated to `Cargo.toml` `[package].version`
- `ghrr/__init__.py` — Removal (no Rust equivalent needed)

**Build and packaging replacements:**
- `setup.py` — Replace entirely with `Cargo.toml` (Cargo-based build system)
- `requirements.txt` — Replace entirely with `Cargo.toml` `[dependencies]` section

**Configuration updates:**
- `.devcontainer/devcontainer.json` — Update to Rust dev container image and toolchain
- `.vscode/launch.json` — Update debug configuration for Rust (LLDB-based debugging)
- `.vscode/settings.json` — Update editor settings for Rust analyzer and Cargo
- `.vscode/tasks.json` — Update build/test tasks from pip/python to cargo commands

**New Rust source files to create:**
- `src/main.rs` — Application entry point and orchestration
- `src/cli.rs` — CLI argument parsing with `clap` derive macros
- `src/auth.rs` — GitHub authentication via environment variables
- `src/github_client.rs` — GitHub API client wrapping `octocrab` with rate limit handling
- `src/models.rs` — Data structures (`User` struct with serde serialization)
- `src/csv_output.rs` — CSV writer and output handling (file and stdout modes)
- `src/progress.rs` — Progress bar management with `indicatif` and silent mode stubs
- `src/errors.rs` — Custom error types using `thiserror` and `anyhow`

**New Rust test files to create:**
- `tests/integration_test.rs` — End-to-end integration tests for CLI behavior
- Unit tests embedded within each module via `#[cfg(test)]` blocks

**Documentation updates:**
- `README.md` — Complete rewrite for Rust build/install/usage instructions

**Files to retain unchanged:**
- `LICENSE` — Apache 2.0 license text (no changes required)

### 0.3.2 Explicitly Out of Scope

- **No new features:** This migration strictly preserves existing functionality; no new CLI commands, output formats, or API endpoints will be introduced
- **No GraphQL migration:** The current implementation uses GitHub REST API v3; migration to GitHub GraphQL API v4 is not part of this refactor
- **No async concurrency for user streams:** The original processes stargazers, subscribers, and contributors sequentially; this sequencing is preserved (no parallel stream processing)
- **No CI/CD pipeline creation:** While the build process will work with `cargo build`/`cargo test`, creating GitHub Actions workflows or other CI/CD pipelines is not in scope
- **No data persistence or caching:** The stateless execution model of the original is preserved; no database, cache, or checkpoint mechanisms will be added
- **No cross-platform packaging:** Distribution as platform-specific packages (Homebrew, apt, Chocolatey) is not in scope; the deliverable is a Cargo project buildable to a native binary
- **No web UI or API server:** The application remains a CLI-only tool

## 0.4 Target Design

### 0.4.1 Refactored Structure Planning

The target Rust project replaces the flat Python module structure with a modular, idiomatic Rust layout following Cargo conventions and single-responsibility principles. Each functional concern from the monolithic `ghrr/main.py` is extracted into its own Rust module.

**Target Architecture:**

```
ghrr/                              (repository root)
├── Cargo.toml                     (Rust package manifest — replaces setup.py + requirements.txt)
├── Cargo.lock                     (dependency lock file — auto-generated by cargo)
├── LICENSE                        (Apache 2.0 — retained unchanged)
├── README.md                      (rewritten for Rust build/install/usage)
├── .devcontainer/
│   └── devcontainer.json          (updated for Rust dev container)
├── .vscode/
│   ├── launch.json                (updated for Rust/LLDB debugging)
│   ├── settings.json              (updated for rust-analyzer)
│   └── tasks.json                 (updated for cargo build/test tasks)
├── src/
│   ├── main.rs                    (entry point: parse args → validate → run pipeline)
│   ├── cli.rs                     (clap-based CLI argument definitions)
│   ├── auth.rs                    (env var authentication + validation)
│   ├── github_client.rs           (octocrab wrapper: API calls, rate limiting, retries)
│   ├── models.rs                  (User struct, serde serialization, company normalization)
│   ├── csv_output.rs              (CSV writer: file and stdout modes)
│   ├── progress.rs                (indicatif progress bars + silent mode stubs)
│   └── errors.rs                  (custom error types: thiserror + anyhow integration)
└── tests/
    └── integration_test.rs        (end-to-end CLI integration tests)
```

### 0.4.2 Module Responsibility Mapping

Each Rust module maps directly to a functional concern from the original Python source:

| Rust Module | Responsibility | Python Origin |
|---|---|---|
| `src/main.rs` | Application entry point, orchestration pipeline (parse → validate → login → resolve repo → iterate users), sequential processing of stargazers/subscribers/contributors | `__main__` block in `ghrr/main.py` (lines 115-148) |
| `src/cli.rs` | CLI argument struct with `clap::Parser` derive: `--organization`, `--repository`, `--file` flags and `--version` | `argparse.ArgumentParser` setup (lines 15-24 of `main.py`) |
| `src/auth.rs` | Read `GITHUB_USER` and `GITHUB_TOKEN` from environment, validate presence, construct authentication credentials | `validate_params()` function + env var reads (lines 12-13, 52-61) |
| `src/github_client.rs` | `octocrab`-based GitHub API wrapper: repository resolution with retry, list stargazers/subscribers/contributors, per-user profile enrichment, rate limit detection and sleep-until-reset logic | `wait_rate_limit()`, retry logic in `iterate_users()`, repository resolution retry loop (lines 69-113) |
| `src/models.rs` | `User` struct with 7 fields (email, location, username, company, followers, repos, organizations), serde Serialize derive, company name normalization (strip `@`), organizations join logic | `User = namedtuple(...)` and `get_user_data()` (lines 26-49) |
| `src/csv_output.rs` | CSV writer factory: file mode (with date-stamped filename `ghusers_{org}_{repo}_{date}.csv`) and stdout mode; header row writing; per-record row writing | CSV writer setup + write calls throughout `iterate_users()` |
| `src/progress.rs` | `indicatif::ProgressBar` wrapper with two modes: active (stderr rendering) and silent (hidden, for stdout output mode matching `-f -`) | `DummyUpdater`, `DummyProgress` classes + `tqdm` usage (lines 63-67, 83-85) |
| `src/errors.rs` | Custom error enum using `thiserror` for typed errors (auth, API, rate limit, CSV), `anyhow` for ergonomic error propagation | Implicit: `die()` lambda, `ForbiddenError` handling, generic `except` blocks |

### 0.4.3 Web Search Research Conducted

The following research informed the Rust technology selections:

- **Octocrab (GitHub API):** The leading Rust GitHub API client (v0.49.3), providing strongly typed semantic API with pagination support, low-level HTTP methods for custom endpoints, and `tokio`-based async execution. Supports personal access token authentication via `OctocrabBuilder`.
- **Clap (CLI parsing):** The de facto standard Rust CLI parser (v4.5.60), supporting derive-based declarative argument definitions with automatic help generation, version flags, and type-safe argument extraction.
- **Indicatif (progress bars):** Rust's primary progress bar library (v0.17.11), providing `ProgressBar` with customizable templates, `ProgressBar::hidden()` for silent mode, and stderr-default rendering — a direct analog to Python's `tqdm`.
- **CSV crate:** BurntSushi's high-performance CSV library (v1.4.0) with native serde integration for struct serialization, `Writer::from_writer()` and `Writer::from_path()` for output modes.
- **Tokio (async runtime):** The standard Rust async runtime (v1.50.0) required by `octocrab`, providing `tokio::time::sleep()` for rate-limit backoff delays and `#[tokio::main]` for async entry points.
- **Serde (serialization):** Rust's universal serialization framework (v1.0.228) with derive macros for automatic `Serialize`/`Deserialize` implementations on the `User` struct.
- **Chrono (datetime):** Rust date/time library (v0.4.42) for generating date-stamped filenames matching the Python `datetime.datetime.now().strftime('%Y%m%d')` pattern.
- **Anyhow/Thiserror (error handling):** `anyhow` (v1.0.100) for ergonomic application-level error propagation and `thiserror` (v2.0.12) for custom typed error enums replacing Python's exception handling.

### 0.4.4 Design Pattern Applications

- **Module pattern:** Each Rust source file encapsulates a single concern with a clean public API, replacing the monolithic single-file Python approach
- **Builder pattern:** `OctocrabBuilder` for GitHub client construction, `clap::Parser` derive for CLI argument building, `csv::WriterBuilder` for output configuration
- **Strategy pattern:** Output mode selection (file vs. stdout) determines which `csv::Writer` variant is constructed; progress bar mode (active vs. hidden) determined by output target
- **Retry pattern with exponential awareness:** Rate limit handling reads `X-RateLimit-Reset` header and computes exact sleep duration plus buffer, with fallback to constant `RATE_LIMIT_BACKOFF` delay for non-rate-limit errors — preserving the Python original's resilience model
- **Type-state pattern:** Rust's type system enforces that authentication credentials are validated before API client construction, making invalid states unrepresentable

## 0.5 Transformation Mapping

### 0.5.1 File-by-File Transformation Plan

The following table provides the complete, exhaustive mapping of every target file to its source origin. Every target file in the Rust project is accounted for, and every source file in the Python project is addressed.

| Target File | Transformation | Source File | Key Changes |
|---|---|---|---|
| `Cargo.toml` | CREATE | `setup.py`, `requirements.txt` | Create Cargo package manifest with metadata from setup.py (name, version, author, license, description) and all Rust crate dependencies replacing PyPI packages from requirements.txt |
| `src/main.rs` | CREATE | `ghrr/main.py` | Extract `__main__` orchestration block (lines 115-148): parse args → validate auth → create GitHub client → resolve repo → create CSV writer → iterate stargazers/subscribers/contributors sequentially; wrap in `#[tokio::main] async fn main()` |
| `src/cli.rs` | CREATE | `ghrr/main.py` | Extract argparse setup (lines 15-24) into a `clap::Parser` derived `Args` struct with `#[arg(short = 'o', long = "organization")]`, `#[arg(short = 'r', long = "repository")]`, `#[arg(short = 'f', long = "file")]` fields and `#[command(version)]` |
| `src/auth.rs` | CREATE | `ghrr/main.py` | Extract `validate_params()` (lines 52-61) and env var reads (lines 12-13) into `AuthCredentials` struct with `from_env()` constructor; replace `os.getenv()` with `std::env::var()` and `die()` with `Result`-based error returns |
| `src/github_client.rs` | CREATE | `ghrr/main.py` | Extract `wait_rate_limit()` (lines 69-79), repository resolution retry loop (lines 121-133), and API iteration logic from `iterate_users()` (lines 81-113); replace `github3.login()` with `Octocrab::builder().personal_token()`, replace `ForbiddenError` with octocrab error type matching, implement `list_stargazers()`, `list_subscribers()`, `list_contributors()` and `get_user_profile()` methods |
| `src/models.rs` | CREATE | `ghrr/main.py` | Extract `User` namedtuple (line 26) and `get_user_data()` (lines 28-49) into a `User` struct with `#[derive(serde::Serialize)]`; replicate company normalization (`replace('@', '')`) and organizations aggregation (`join(', ')`) |
| `src/csv_output.rs` | CREATE | `ghrr/main.py` | Extract CSV writer creation (lines 134-139) and row writing from `iterate_users()`; implement `CsvOutput` enum with `File(csv::Writer<File>)` and `Stdout(csv::Writer<Stdout>)` variants; replicate date-stamped filename pattern `ghusers_{org}_{repo}_{date}.csv` |
| `src/progress.rs` | CREATE | `ghrr/main.py` | Extract `DummyUpdater`/`DummyProgress` stubs (lines 63-67) and tqdm usage (lines 83-85); implement `create_progress_bar(total, silent)` returning `indicatif::ProgressBar` or `ProgressBar::hidden()` |
| `src/errors.rs` | CREATE | `ghrr/main.py` | Extract implicit error handling (`warn`/`die` lambdas at line 25, `ForbiddenError` catch at lines 92-97, generic `except` at lines 98-104) into a `GhrrError` enum with `#[derive(thiserror::Error)]` |
| `tests/integration_test.rs` | CREATE | — | New file; create integration tests verifying CLI argument parsing, auth validation error messages, CSV header output, and end-to-end workflow with mocked API responses |
| `README.md` | UPDATE | `README.md` | Rewrite installation (from `python setup.py install` to `cargo build --release`), usage instructions, prerequisites (Rust toolchain instead of Python), and examples while preserving project description and CLI interface documentation |
| `LICENSE` | REFERENCE | `LICENSE` | Retain unchanged; reference for `Cargo.toml` license field value (`Apache-2.0`) |
| `.devcontainer/devcontainer.json` | UPDATE | `.devcontainer/devcontainer.json` | Change image from `mcr.microsoft.com/devcontainers/python:3.10-bullseye` to `mcr.microsoft.com/devcontainers/rust:1-bullseye`; remove Node.js feature; add rust-analyzer extension |
| `.vscode/launch.json` | UPDATE | `.vscode/launch.json` | Replace Python debug configuration with Rust LLDB/CodeLLDB configuration targeting `target/debug/ghrr` binary; remove `preLaunchTask: pipInstall` |
| `.vscode/settings.json` | UPDATE | `.vscode/settings.json` | Replace `python.pythonPath` with `rust-analyzer` settings; update `files.exclude` to add `target/` directory |
| `.vscode/tasks.json` | UPDATE | `.vscode/tasks.json` | Replace `pip install -r requirements.txt` task with `cargo build` and `cargo test` tasks |

**Python files to remove (no Rust equivalent):**

| Removed File | Reason |
|---|---|
| `ghrr/__init__.py` | Python package marker; Rust uses `mod` declarations instead |
| `ghrr/main.py` | Replaced entirely by `src/*.rs` modules |
| `ghrr/version.py` | Version now in `Cargo.toml` `[package].version` |
| `setup.py` | Replaced by `Cargo.toml` |
| `requirements.txt` | Replaced by `Cargo.toml` `[dependencies]` |

### 0.5.2 Cross-File Dependencies

**Import transformation rules for the Rust codebase:**

- `src/main.rs` imports from all other modules:
  - `mod cli;` → `use cli::Args;`
  - `mod auth;` → `use auth::AuthCredentials;`
  - `mod github_client;` → `use github_client::GithubClient;`
  - `mod models;` → `use models::User;`
  - `mod csv_output;` → `use csv_output::CsvOutput;`
  - `mod progress;` → `use progress::create_progress_bar;`
  - `mod errors;` → `use errors::GhrrError;`

- `src/github_client.rs` depends on:
  - `crate::models::User` for user data enrichment return type
  - `crate::errors::GhrrError` for error handling
  - `octocrab` for API access
  - `tokio::time::sleep` for rate limit delays

- `src/csv_output.rs` depends on:
  - `crate::models::User` for serialization
  - `csv` crate for writing
  - `chrono` for date-stamped filenames

- `src/models.rs` depends on:
  - `serde::Serialize` for CSV serialization

- `tests/integration_test.rs` depends on:
  - `assert_cmd` for CLI binary testing
  - `predicates` for output assertion

### 0.5.3 One-Phase Execution

The entire refactor is executed by Blitzy in **one phase**. All file removals (Python sources), file creations (Rust sources), and file updates (configuration and documentation) are included in a single transformation batch. There is no phased or incremental migration — the repository transitions from a Python project to a Rust project atomically.

## 0.6 Dependency Inventory

### 0.6.1 Key Public Packages

The following table lists all Rust crate dependencies required for the refactored project, mapped to the Python packages they replace. All versions have been verified against crates.io as of March 2026.

| Registry | Crate Name | Version | Purpose | Replaces (Python) |
|---|---|---|---|---|
| crates.io | `octocrab` | 0.49.3 | GitHub REST API v3 client with typed models, pagination, and authentication | `github3.py` (3.2.0) |
| crates.io | `tokio` | 1.50.0 | Async runtime required by octocrab; provides `sleep()` for rate-limit backoff | `time.sleep()` (stdlib) |
| crates.io | `clap` | 4.5.60 | CLI argument parsing with derive macros for type-safe arg definitions | `argparse` (stdlib) |
| crates.io | `csv` | 1.4.0 | High-performance CSV reading/writing with serde integration | `csv` (stdlib) |
| crates.io | `serde` | 1.0.228 | Serialization/deserialization framework with derive macros | `collections.namedtuple` (stdlib) |
| crates.io | `serde_json` | 1.0.145 | JSON serialization for API response handling | Implicit via `github3.py` |
| crates.io | `indicatif` | 0.17.11 | Terminal progress bars with hidden mode support | `tqdm` (4.64.1) |
| crates.io | `chrono` | 0.4.42 | Date/time library for date-stamped filename generation | `datetime` (stdlib) |
| crates.io | `anyhow` | 1.0.100 | Ergonomic application-level error handling with context | Implicit: `try/except` patterns |
| crates.io | `thiserror` | 2.0.12 | Derive macro for custom error types | Implicit: `ForbiddenError`, `Exception` |
| crates.io | `reqwest` | 0.12.26 | HTTP client (transitive via octocrab; may be used for custom header inspection) | `requests` (2.28.1) |

**Development and test dependencies:**

| Registry | Crate Name | Version | Purpose |
|---|---|---|---|
| crates.io | `assert_cmd` | 2.0.16 | Integration testing of CLI binary execution |
| crates.io | `predicates` | 3.1.2 | Assertion predicates for CLI output testing |
| crates.io | `wiremock` | 0.6.2 | HTTP mocking for GitHub API integration tests |
| crates.io | `tempfile` | 3.15.0 | Temporary file creation for CSV output testing |

### 0.6.2 Cargo Feature Flags

Specific crate features required for this project:

- `tokio`: `features = ["full"]` — enables all tokio components (runtime, time, macros, io)
- `clap`: `features = ["derive"]` — enables `#[derive(Parser)]` macro support
- `serde`: `features = ["derive"]` — enables `#[derive(Serialize, Deserialize)]` macro support
- `octocrab`: default features sufficient (includes reqwest-based HTTP)

### 0.6.3 Target Cargo.toml Structure

```toml
[package]
name = "ghrr"
version = "0.0.1"
edition = "2024"
license = "Apache-2.0"
```

### 0.6.4 Dependency Removals

All Python dependencies are fully removed as the entire Python packaging ecosystem is replaced by Cargo:

| Removed Package | Registry | Reason |
|---|---|---|
| `github3.py` (3.2.0) | PyPI | Replaced by `octocrab` |
| `tqdm` (4.64.1) | PyPI | Replaced by `indicatif` |
| `requests` (2.28.1) | PyPI | Replaced by `reqwest` (transitive via octocrab) |
| `certifi` (2022.9.24) | PyPI | Rust uses system/native TLS certificates |
| `cffi` (1.15.1) | PyPI | No equivalent needed (Rust has native FFI) |
| `charset-normalizer` (2.1.1) | PyPI | Rust strings are UTF-8 by default |
| `cryptography` (38.0.3) | PyPI | Replaced by Rust TLS stack (rustls/native-tls) |
| `idna` (3.4) | PyPI | Handled by `url` crate (transitive) |
| `lxml` (4.9.1) | PyPI | No XML parsing required in core functionality |
| `pycparser` (2.21) | PyPI | No equivalent needed |
| `PyJWT` (2.6.0) | PyPI | JWT handling not required for token auth |
| `python-dateutil` (2.8.2) | PyPI | Replaced by `chrono` |
| `six` (1.16.0) | PyPI | Python 2/3 compatibility shim; not applicable |
| `uritemplate` (4.1.1) | PyPI | Handled internally by `octocrab` |
| `urllib3` (1.26.12) | PyPI | Replaced by `reqwest`/`hyper` (transitive) |

### 0.6.5 External Reference Updates

Configuration and documentation files requiring dependency-related updates:

- `README.md` — Update prerequisite section from Python/pip to Rust/Cargo toolchain
- `.devcontainer/devcontainer.json` — Update from Python dev container to Rust dev container
- `.vscode/tasks.json` — Update from `pip install` to `cargo build`

## 0.7 Refactoring Rules

### 0.7.1 Behavioral Parity Requirements

The following rules ensure the Rust implementation maintains functional equivalence with the Python original:

- **CLI interface contract:** The binary must accept the same argument flags (`-o`/`--organization`, `-r`/`--repository`, `-f`/`--file`) with identical semantics. The `--version` flag must output the same version string (`0.0.1`). The `-f -` flag must redirect CSV output to stdout and suppress progress bars.
- **CSV output format:** The output file must contain the exact same column headers in the same order: `username`, `company`, `organizations`, `email`, `location`, `followers_count`, `public_repos_count`, `user_interaction`. The filename pattern must be `ghusers_{ORG}_{REPO}_{DATE}.csv` with `DATE` formatted as `YYYYMMDD`.
- **Authentication model:** The binary must read `GITHUB_USER` and `GITHUB_TOKEN` from environment variables. If either is missing, the program must print a descriptive error message to stderr and exit with a non-zero status code.
- **Rate limit handling:** When a GitHub API `403 Forbidden` response is received indicating rate limiting, the program must read the `X-RateLimit-Reset` response header, compute the remaining wait time, add a 3-second buffer, sleep for that duration, and then retry the request. This behavior must be preserved exactly.
- **Generic error resilience:** For non-rate-limit errors during user iteration, the program must wait `RATE_LIMIT_BACKOFF` seconds (10) and retry indefinitely, matching the Python infinite retry loop.
- **Repository resolution retry:** If the initial repository lookup fails, the program must sleep for `RATE_LIMIT_BACKOFF` seconds and retry, matching the existing retry loop.
- **User data enrichment:** For each lightweight user reference from listing endpoints, the program must fetch the full user profile and their organization memberships, concatenate organization names with `', '` separator, and strip all `@` characters from the company field.
- **Sequential stream processing:** Stargazers must be processed first, then subscribers, then contributors — in that exact order — each prefixed with their interaction type in the CSV output.
- **Progress reporting:** Progress bars must render to stderr. When output is directed to stdout (`-f -` mode), progress bars must be completely suppressed (hidden).

### 0.7.2 Rust-Specific Conventions

- **Idiomatic Rust:** All code must follow standard Rust conventions — snake_case for functions and variables, CamelCase for types and traits, ALL_CAPS for constants
- **Error handling:** Use `Result<T, E>` return types throughout; never use `panic!()` or `unwrap()` for recoverable errors in production code paths. `unwrap()` is acceptable only in tests.
- **Ownership and borrowing:** Prefer borrowing over cloning where possible. Use `String` for owned data in structs and `&str` for function parameters where appropriate.
- **Async/await:** Since `octocrab` requires an async runtime, the main function uses `#[tokio::main]` and API-interacting functions are `async`. Non-async helper functions (CSV writing, progress bar updates) remain synchronous.
- **Module visibility:** Each module exposes only its public API via `pub` items. Internal implementation details remain private.

### 0.7.3 Build and Test Requirements

- **Full build process:** `cargo build` must complete without errors or warnings
- **Unit tests:** Each module must include `#[cfg(test)] mod tests` blocks testing its core logic in isolation
- **Integration tests:** The `tests/` directory must include tests that invoke the compiled binary with `assert_cmd` and verify CLI behavior, argument validation, and error output
- **Test coverage targets:** All public functions must have at least one corresponding test. Error paths (missing auth, API failures, rate limiting) must be tested.
- **Linting:** Code must pass `cargo clippy` without warnings at the default lint level

### 0.7.4 Special Instructions and Constraints

- **No phased migration:** The entire codebase transforms in one phase. There is no intermediate state where both Python and Rust coexist.
- **Version preservation:** The initial Rust release must use version `0.0.1` matching the Python original to maintain version continuity.
- **License preservation:** The Apache 2.0 license must be retained. The `Cargo.toml` must reference `license = "Apache-2.0"`.
- **Author attribution:** The original author attribution (`schoster barak`) from `setup.py` must be preserved in `Cargo.toml` metadata.

## 0.8 References

### 0.8.1 Repository Files Searched

The following files were comprehensively retrieved and analyzed to derive conclusions for this Agent Action Plan:

| File Path | Purpose | Key Findings |
|---|---|---|
| `ghrr/main.py` | Core application logic (148 lines) | Complete functional decomposition: CLI parsing, GitHub API interaction via github3.py, CSV output, rate limit handling, retry logic, progress bars, user data enrichment with organization aggregation |
| `ghrr/version.py` | Version constant | Single line: `version = '0.0.1'` |
| `ghrr/__init__.py` | Python package marker | Empty file (returned file-not-found on read but described as empty in folder summary) |
| `setup.py` | Python packaging configuration | setuptools-based; key metadata: name="ghrr", license="Apache 2.0", author="schoster barak", classifiers for Python 3.6/3.7; install_requires pins `github3.py==2.3.0` (version discrepancy with requirements.txt) |
| `requirements.txt` | Python dependency manifest | 15 pinned PyPI packages; direct deps: github3.py==3.2.0, tqdm==4.64.1; all dependencies dated to late 2022 |
| `README.md` | Project documentation | Documents CLI usage, env var requirements, installation via `python setup.py install`, output format and column structure |
| `.devcontainer/devcontainer.json` | Development container config | Python 3.10 Bullseye image with Node.js and GitHub CLI features |
| `.vscode/launch.json` | VS Code debug config | Python: Current File debug configuration with preLaunchTask |
| `.vscode/settings.json` | VS Code editor settings | Python venv path, standard file exclusions |
| `.vscode/tasks.json` | VS Code build tasks | pip install task for requirements.txt |
| `LICENSE` | License text | Full Apache License 2.0 (202 lines) |

### 0.8.2 Folders Searched

| Folder Path | Contents |
|---|---|
| `` (repository root) | 4 files + 3 directories: LICENSE, README.md, requirements.txt, setup.py, .devcontainer/, .vscode/, ghrr/ |
| `ghrr/` | 3 files: __init__.py, main.py, version.py |
| `.devcontainer/` | 1 file: devcontainer.json |
| `.vscode/` | 3 files: launch.json, settings.json, tasks.json |

### 0.8.3 Technical Specification Sections Referenced

| Section Heading | Key Information Extracted |
|---|---|
| 1.1 Executive Summary | Project overview, stakeholders, business value, core problem statement |
| 3.2 Programming Languages | Python compatibility (3.6/3.7 declared, 3.10 dev runtime), codebase size (~150 lines) |
| 3.3 Frameworks and Libraries | Core functional dependencies identification |
| github3.py — GitHub REST API Wrapper | github3.py version discrepancy analysis, tqdm usage patterns, maintenance status |
| 3.4 Open Source Dependencies | Complete 15-package dependency manifest with roles, categories, and security status |
| 5.1 High-Level Architecture | Monolithic single-threaded architecture, 10-stage data flow pipeline, 7 GitHub API endpoint categories, resilience model |

### 0.8.4 External Research Sources

| Topic | Source | Key Finding |
|---|---|---|
| Rust GitHub API crate | crates.io/octocrab (v0.49.3) | Leading Rust GitHub API client with typed semantic API and low-level HTTP methods; async/tokio-based |
| Rust CLI parsing | crates.io/clap (v4.5.60) | De facto standard CLI parser with derive macro support for type-safe argument definitions |
| Rust progress bars | crates.io/indicatif (v0.17.11) | Primary Rust progress bar library with hidden mode, stderr default, and customizable templates |
| Rust CSV handling | crates.io/csv (v1.4.0) | High-performance CSV with serde integration; 129M+ total downloads |
| Rust async runtime | crates.io/tokio (v1.50.0) | Standard async runtime; LTS releases available (1.43.x until March 2026, 1.47.x until September 2026) |
| Rust serialization | crates.io/serde (v1.0.228) | Universal serialization framework with derive macros |
| Rust datetime | crates.io/chrono (v0.4.42) | Date/time library for proleptic Gregorian calendar operations |
| Rust error handling | crates.io/anyhow (v1.0.100), crates.io/thiserror (v2.0.12) | Application-level errors (anyhow) and library-level typed errors (thiserror) |
| Rust stable version | releases.rs | Rust 1.94.0 stable; Edition 2024 available |

### 0.8.5 Attachments

No attachments were provided for this project. No Figma screens or external design assets are referenced.

