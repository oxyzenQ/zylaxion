// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Path security guard for zylaxion's runtime state I/O.
//!
//! ## Scope
//!
//! zylaxion writes three runtime files: a lock file, a Unix socket, and a
//! PID file. All three live under `$XDG_RUNTIME_DIR` (or `/tmp` as
//! fallback). Without this guard, a malicious or careless environment
//! could set `XDG_RUNTIME_DIR=/etc` and cause zylaxion to write socket
//! and PID files into protected directories.
//!
//! ## Policy
//!
//! The runtime directory must NOT resolve to a known-dangerous system
//! path. When `XDG_RUNTIME_DIR` points to a dangerous location, this
//! module falls back to `/tmp` (the same behavior as when
//! `XDG_RUNTIME_DIR` is unset) rather than refusing to start — the
//! daemon needs to run, but it must not write state into `/etc`, `/usr`,
//! `~/.ssh`, etc.
//!
//! Config file reads from `/etc/zylaxion/` and `/usr/local/share/zylaxion/`
//! are BY DESIGN (system-wide config) and are NOT gated here — those are
//! read-only paths with fixed names, not user-controlled arbitrary paths.

use std::path::{Component, Path, PathBuf};

/// Lexically normalize a path: collapse `.` and `..` without requiring
/// the file to exist. Does NOT follow symlinks.
fn lexical_normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    let _ = out.pop();
                }
                Some(Component::RootDir) | None => {}
                _ => {
                    let _ = out.pop();
                }
            },
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Canonicalize for comparison — tries `std::fs::canonicalize`, falls
/// back to lexical normalization for paths that don't exist yet.
fn canonical_for_compare(path: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(path) {
        return c;
    }
    lexical_normalize(path).unwrap_or_else(|| path.to_path_buf())
}

/// Return true if the canonical path falls inside a known-dangerous prefix
/// that zylaxion's runtime state must NEVER touch — even if
/// `XDG_RUNTIME_DIR` points there.
fn is_dangerous(canonical: &Path) -> bool {
    let s = canonical.to_string_lossy();
    let home = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).to_string_lossy().into_owned())
        .unwrap_or_default();

    // System paths — never valid for zylaxion runtime state, even if
    // XDG_RUNTIME_DIR points here. zylaxion is a user-space desktop app;
    // its runtime state must live in /run/user/$UID or /tmp, NOT in /etc,
    // /usr, /var, etc.
    let deny_prefixes: &[&str] = &[
        "/etc", "/usr", "/var", "/bin", "/sbin", "/lib", "/lib64", "/boot", "/root", "/proc",
        "/sys", "/dev",
    ];

    for prefix in deny_prefixes {
        // Match exact path OR path with trailing slash OR path starting
        // with prefix + "/" (e.g. "/etc" matches, "/etc/" matches, "/etc/zylaxion.sock" matches)
        if s == *prefix || s.starts_with(&format!("{prefix}/")) {
            return true;
        }
    }

    // User credential stores — never valid for runtime state
    let user_deny_subdirs: &[&str] = &[".ssh", ".gnupg", ".kwallet", ".local/share/keyrings"];
    if !home.is_empty() {
        for sub in user_deny_subdirs {
            let full = format!("{home}/{sub}");
            if s == full || s.starts_with(&format!("{full}/")) {
                return true;
            }
        }
    }

    false
}

/// Resolve the safe runtime directory for zylaxion state files.
///
/// Returns the canonicalized, validated directory path. If
/// `$XDG_RUNTIME_DIR` is set and safe, uses it. If it's set but
/// dangerous (e.g. `/etc`), falls back to `/tmp`. If it's unset,
/// uses `/tmp`.
pub fn resolve_runtime_dir() -> PathBuf {
    let candidate: PathBuf =
        if let Some(xdg) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
            PathBuf::from(&xdg)
        } else {
            PathBuf::from("/tmp")
        };

    let canonical = canonical_for_compare(&candidate);
    if is_dangerous(&canonical) {
        // Fall back to /tmp — don't refuse to start, but don't write
        // state into the dangerous location either.
        PathBuf::from("/tmp")
    } else {
        canonical
    }
}

/// Serialize env-mutating tests across the crate (night-improve-4,
/// edition 2024). `std::env::set_var`/`remove_var` became unsafe
/// because they race with concurrent environment reads; every test
/// that mutates the environment must hold this crate-wide lock so
/// parallel test threads never race each other's save/restore
/// windows. This suite takes it inside `with_test_env`; the
/// `instance_lock` suite takes it via `crate::session::pathguard::ENV_LOCK`.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
#[path = "../../test/session/pathguard_tests.rs"]
mod tests;
