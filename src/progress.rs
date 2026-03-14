// src/progress.rs — Progress Bar Management
//
// This module manages progress bar display using the `indicatif` crate,
// replacing Python's `tqdm` usage and the `DummyUpdater`/`DummyProgress`
// stub classes from `ghrr/main.py`.
//
// Behavioral parity (AAP §0.7.1):
// - Progress bars render to stderr (indicatif default).
// - When output is directed to stdout (`-f -` mode), progress bars are
//   completely suppressed via `ProgressBar::hidden()`.
// - The `silent` parameter maps to Python's `progress=False` (main.py line 131).

use indicatif::{ProgressBar, ProgressStyle};

/// Creates a progress bar for tracking user data fetching progress.
///
/// This factory function returns either an active or hidden progress bar,
/// replacing the Python `tqdm` constructor (line 80) and the
/// `DummyProgress`/`DummyUpdater` stub classes (lines 52-63) from
/// `ghrr/main.py`.
///
/// # Arguments
///
/// * `total` - The total number of items to process, or `None` when the
///   count is unknown (e.g., contributors in the Python original pass
///   `total=None` to tqdm). When `Some(t)`, a determinate progress bar
///   is created; when `None`, a spinner is created.
/// * `description` - A label identifying the current operation, such as
///   `"stargazer"`, `"subscriber"`, or `"contributor"`. This is embedded
///   into the progress bar prefix as `"Fetching {description} data"` to
///   match Python's `desc=f'Fetching {user_interaction} data'`.
/// * `silent` - If `true`, returns a `ProgressBar::hidden()` instance that
///   suppresses all output — equivalent to the Python `DummyProgress` class.
///   Set to `true` when CSV output is directed to stdout (`-f -` mode) so
///   that progress rendering does not corrupt the data stream.
///
/// # Returns
///
/// An `indicatif::ProgressBar` that:
/// - Renders to stderr (indicatif's default draw target) when active.
/// - Is completely invisible when `silent` is `true`.
/// - Supports `.inc(1)` for per-item advancement (replaces Python
///   `progress_bar.update(1)` at line 98).
/// - Supports `.finish()` and `.finish_and_clear()` for completion.
///
/// # Examples
///
/// ```rust
/// use ghrr::progress::create_progress_bar;
///
/// // Active bar with known total (stargazers / subscribers)
/// let pb = create_progress_bar(Some(100), "stargazer", false);
/// pb.inc(1);
/// pb.finish_and_clear();
///
/// // Spinner with unknown total (contributors)
/// let pb = create_progress_bar(None, "contributor", false);
/// pb.inc(1);
/// pb.finish();
///
/// // Silent / hidden bar (stdout output mode)
/// let pb = create_progress_bar(Some(50), "subscriber", true);
/// pb.inc(1); // no-op, no output
/// pb.finish(); // no-op
/// ```
pub fn create_progress_bar(total: Option<u64>, description: &str, silent: bool) -> ProgressBar {
    if silent {
        // Equivalent to DummyProgress/DummyUpdater — completely hidden.
        // Python lines 57-63: DummyProgress context manager that returns
        // a DummyUpdater whose .update() is a no-op. In indicatif,
        // ProgressBar::hidden() automatically ignores .inc() and .finish() calls.
        ProgressBar::hidden()
    } else {
        // Build the description string matching Python's tqdm desc parameter:
        //   tqdm(total=total, desc=f'Fetching {user_interaction} data', unit='users')
        // Python line 80
        let desc = format!("Fetching {} data", description);

        let pb = match total {
            Some(t) => {
                // Determinate progress bar with a known total.
                // Equivalent to: tqdm(total=<count>, desc=..., unit='users')
                let bar = ProgressBar::new(t);

                // Apply a tqdm-like display template:
                //   Fetching stargazer data: ████████░░░░ 45/100 [00:30 < 00:40] 1.5/s
                //
                // Template keys:
                //   {prefix}         — set via set_prefix(), maps to tqdm's desc
                //   {bar:40}         — 40-char wide progress bar
                //   {pos}/{len}      — current / total count
                //   {elapsed_precise} — elapsed wall time
                //   {eta_precise}    — estimated remaining time
                //   {per_sec}        — throughput (items/s)
                bar.set_style(
                    ProgressStyle::with_template(
                        "{prefix}: {bar:40} {pos}/{len} [{elapsed_precise} < {eta_precise}] {per_sec}",
                    )
                    .unwrap_or_else(|_| ProgressStyle::default_bar()),
                );
                bar
            }
            None => {
                // Spinner for unknown total (contributors case where Python
                // passes total=None to tqdm).
                let spinner = ProgressBar::new_spinner();

                // Spinner template — no bar or ETA since length is unknown.
                spinner.set_style(
                    ProgressStyle::with_template(
                        "{prefix}: {spinner} {pos} [{elapsed_precise}] {per_sec}",
                    )
                    .unwrap_or_else(|_| ProgressStyle::default_bar()),
                );
                spinner
            }
        };

        // Set the prefix (displayed via {prefix} in the template) to match
        // tqdm's desc parameter: "Fetching stargazer data", etc.
        pb.set_prefix(desc.clone());

        // Also set the message field for accessibility and completeness.
        // While {msg} is not referenced in the active templates above,
        // this ensures the description is available if the fallback
        // ProgressStyle::default_bar() is used (which renders {msg}).
        pb.set_message(desc);

        pb
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that an active bar with a known total reports the correct length.
    #[test]
    fn test_create_progress_bar_active_with_total() {
        let pb = create_progress_bar(Some(100), "stargazer", false);
        assert_eq!(pb.length(), Some(100));
    }

    /// Verify that an active bar with no total creates a spinner (length is None).
    #[test]
    fn test_create_progress_bar_active_without_total() {
        let pb = create_progress_bar(None, "contributor", false);
        // ProgressBar::new_spinner() has no fixed length.
        assert!(pb.length().is_none() || pb.length() == Some(0));
    }

    /// Verify that a silent bar with a known total does not panic on inc/finish.
    #[test]
    fn test_create_progress_bar_silent() {
        let pb = create_progress_bar(Some(100), "stargazer", true);
        // Hidden progress bar — must not render or panic.
        pb.inc(1);
        pb.finish();
    }

    /// Verify that a silent bar with no total does not panic on inc/finish.
    #[test]
    fn test_create_progress_bar_silent_no_total() {
        let pb = create_progress_bar(None, "contributor", true);
        pb.inc(1);
        pb.finish();
    }

    /// Verify that finish_and_clear works on an active bar.
    #[test]
    fn test_create_progress_bar_finish_and_clear() {
        let pb = create_progress_bar(Some(50), "subscriber", false);
        pb.inc(1);
        pb.finish_and_clear();
    }

    /// Verify that finish_and_clear works on a silent bar.
    #[test]
    fn test_create_progress_bar_silent_finish_and_clear() {
        let pb = create_progress_bar(Some(50), "subscriber", true);
        pb.inc(1);
        pb.finish_and_clear();
    }

    /// Verify that incrementing up to the total works without panic.
    #[test]
    fn test_create_progress_bar_multiple_increments() {
        let pb = create_progress_bar(Some(10), "stargazer", false);
        for _ in 0..10 {
            pb.inc(1);
        }
        pb.finish();
    }

    /// Verify behaviour with zero total (edge case — empty repository).
    #[test]
    fn test_create_progress_bar_zero_total() {
        let pb = create_progress_bar(Some(0), "stargazer", false);
        assert_eq!(pb.length(), Some(0));
        pb.finish();
    }

    /// Verify that a spinner can be incremented many times without panic.
    #[test]
    fn test_create_progress_bar_spinner_multiple_increments() {
        let pb = create_progress_bar(None, "contributor", false);
        for _ in 0..25 {
            pb.inc(1);
        }
        pb.finish_and_clear();
    }

    /// Verify that a large total value is handled correctly.
    #[test]
    fn test_create_progress_bar_large_total() {
        let pb = create_progress_bar(Some(1_000_000), "stargazer", false);
        assert_eq!(pb.length(), Some(1_000_000));
        pb.inc(1);
        pb.finish();
    }

    /// Verify that an empty description does not cause any issues.
    #[test]
    fn test_create_progress_bar_empty_description() {
        let pb = create_progress_bar(Some(10), "", false);
        assert_eq!(pb.length(), Some(10));
        pb.inc(1);
        pb.finish();
    }

    /// Verify that all three user-interaction types work as descriptions.
    #[test]
    fn test_create_progress_bar_all_interaction_types() {
        for interaction in &["stargazer", "subscriber", "contributor"] {
            let pb = create_progress_bar(Some(5), interaction, false);
            pb.inc(1);
            pb.finish();
        }
    }
}
