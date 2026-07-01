// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Centralised error/warning formatting.
//!
//! All user-facing error and warning messages go through these helpers
//! to guarantee a consistent prefix:
//!
//!   - errors:   `zylaxion: error: <message>`
//!   - warnings: `zylaxion: warning: <message>`
//!
//! This matches the GNU coding-standard convention used by `gcc`,
//! `clang`, `make`, etc., and makes it trivial for users to `grep`
//! zylaxion's output for errors.

/// Print an error message to stderr in the standard format.
///
/// Format: `zylaxion: error: <message>\n`
#[inline]
pub fn error(msg: impl AsRef<str>) {
    eprintln!("zylaxion: error: {}", msg.as_ref());
}

/// Print a warning message to stderr in the standard format.
///
/// Format: `zylaxion: warning: <message>\n`
#[inline]
pub fn warning(msg: impl AsRef<str>) {
    eprintln!("zylaxion: warning: {}", msg.as_ref());
}
