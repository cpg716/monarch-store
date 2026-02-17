//! Centralized constants for monarch-gui (helper client, merge keys, discovery).
//! Keeps HELPER_DEBOUNCE, CMD_FILE_*, and VARIANT_SUFFIXES in one place.

use std::time::Duration;

/// Minimum interval between helper invocations (debounce) to mitigate DoS from rapid/spam invokes.
pub const HELPER_DEBOUNCE: Duration = Duration::from_millis(800);

/// Temp file prefix for helper command (helper deletes after reading).
pub const CMD_FILE_PREFIX: &str = "monarch-cmd-";
/// Use /var/tmp so both the app and root (sudo) see the same path.
pub const CMD_FILE_DIR: &str = "/var/tmp";

/// Variant suffixes for merge deduplication (e.g. firefox + firefox-developer-edition → one entry).
/// Longer suffixes first so we strip -developer-edition before -edition.
/// Note: canonical_merge_key in utils.rs uses its own inline list; this is kept for reference/other use.
#[allow(dead_code)]
pub const VARIANT_SUFFIXES: &[&str] = &[
    "-developer-edition",
    "-developer-edition-bin",
    "-esr",
    "-esr-bin",
    "-stable",
    "-dev",
    "-bin",
    "-git",
    "-nightly",
    "-beta",
    "-pure",
    "-appimage",
    "-wayland",
    "-x11",
    "-hg",
    "-svn",
    "-cn",
    "-fresh",
    "-still",
    "-native",
    "-runtime",
    "-lts",
    "-edge",
];
