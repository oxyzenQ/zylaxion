// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Build script — captures the current git commit hash at compile time and
//! exposes it to the crate via the `GIT_HASH` environment variable.
//!
//! Used by `cli.rs` to render the real commit hash in `zylaxion -V` /
//! `--version`, replacing the previous hardcoded placeholder.
//!
//! Falls back to the literal string `"unknown"` if:
//!   - `git` is not installed, or
//!   - the crate is built outside a git work tree (e.g. from a tarball), or
//!   - `git rev-parse` exits non-zero for any other reason.
//!
//! In all fallback cases the build still succeeds — only the displayed
//! hash degrades gracefully.

use std::process::Command;

fn main() {
    // Re-run this script whenever HEAD (or the work tree state) changes so
    // the embedded hash stays in sync with what was actually compiled.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");

    let hash = match git_short_hash() {
        Some(h) => h,
        None => "unknown".to_string(),
    };

    println!("cargo:rustc-env=GIT_HASH={}", hash);
}

/// Runs `git rev-parse --short HEAD` from the workspace root and returns
/// the trimmed short hash on success.
fn git_short_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
