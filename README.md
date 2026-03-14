# GitHub Research Runner (ghrr)

A utility to collect data from github stargazers, subscribers and contributors of a selected project.

Usecases are detailed at the following blog: https://www.battery.com/blog/fantastic-developer-community-heroes-and-where-to-find-them/

## Prerequisites

- [Rust toolchain](https://rustup.rs/) (Rust 1.85+ required — Edition 2024)
- A GitHub personal access token — create one using the following guide:
  https://docs.github.com/en/github/authenticating-to-github/creating-a-personal-access-token
- Environment variables:
  - `GITHUB_USER` — your GitHub username
  - `GITHUB_TOKEN` — your GitHub personal access token

## Installing

```bash
git clone https://github.com/schosterbarak/ghrr.git
cd ghrr
cargo build --release
```

The compiled binary will be located at `target/release/ghrr`.

## Using

1. Set your GitHub credentials as environment variables:

```bash
export GITHUB_USER=XXX
export GITHUB_TOKEN=YYY
```

2. Run the tool:

```bash
# Using cargo run
cargo run -- --organization bridgecrewio --repository checkov

# Or using the compiled binary
./target/release/ghrr --organization bridgecrewio --repository checkov
```

### CLI Arguments

| Argument | Short | Description |
| --- | --- | --- |
| `--organization` | `-o` | GitHub organization or user (required) |
| `--repository` | `-r` | GitHub repository name (required) |
| `--file` | `-f` | Output file path, use `-` for stdout (optional) |
| `--version` | | Print version information |

When using `--file -`, CSV output is written to stdout and progress bars are suppressed.

## Result

A CSV file will be created in the following format under the working directory:
`ghusers_{ORG}_{REPO}_{DATE}.csv`

example:
`ghusers_bridgecrewio_checkov_20200921.csv`

| username   | company  | organizations   | email           | location    | followers_count   | public_repos_count   | user_interaction   |
| --------   | -------  | -------------   | -------         | --------    | ---------------   | ------------------   | ---------------   |
| Jon        | ACME     | ACME            | jon@acme.com    | US          | 3                 | 200                  | stargazer          |
| Jane       | ACME     | ACME            | jane@acme.com   | IL          | 3                 | 200                  | contributor        |

When loading to a BI tool:
![GitHub Influencers Dashboard](/influencers%20dashboard.png "GitHub Influencers Dashboard")

## Development

```bash
cargo build            # Debug build
cargo build --release  # Release build
cargo test             # Run all tests
cargo clippy           # Run linter
```

## License

This project is licensed under the Apache License 2.0 — see the [LICENSE](LICENSE) file for details.
