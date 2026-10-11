// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The config search path: where config.toml may live, and how a
//! found path is canonicalised.
//!
//! Split from config.rs in night-improve-18 (the no-mercy LOC law):
//! the path-discovery concern is self-contained — the consts name
//! the homes, the scanners walk them. `pub(super)` so the parent
//! config module is the only consumer.

use std::path::{Path, PathBuf};

const SYSTEM_DATA_DIRS: &[&str] = &["/usr/local/share/zylaxion"];

use super::CONFIG_FILE_NAME;

/// Build the ordered list of candidate `config.toml` paths.
///
/// v10.2.0 (dragonzen audit P2): honor `$XDG_CONFIG_HOME` per the XDG
/// Base Directory Specification. Previously this hardcoded `~/.config/
/// zylaxion/` — if a user set `XDG_CONFIG_HOME=~/.myconfig`, the daemon
/// ignored it and looked in `~/.config/zylaxion/` instead. The fix
/// resolves the config home as:
///
/// 1. `$XDG_CONFIG_HOME` if set and non-empty (and not a relative
///    path — the spec requires absolute).
/// 2. `$HOME/.config` otherwise.
///
/// Falls back to no user-config candidate if neither is set.
pub(super) fn candidate_paths() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Resolve the XDG config home directory.
    let config_home: Option<PathBuf> = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute()) // XDG spec: must be absolute
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|h| PathBuf::from(h).join(".config"))
        });

    if let Some(ch) = config_home {
        candidates.push(ch.join("zylaxion").join(CONFIG_FILE_NAME));
    }
    candidates.push(PathBuf::from("/etc/zylaxion").join(CONFIG_FILE_NAME));
    for data_dir in SYSTEM_DATA_DIRS {
        candidates.push(PathBuf::from(data_dir).join(CONFIG_FILE_NAME));
    }
    candidates.push(PathBuf::from(CONFIG_FILE_NAME));
    candidates
}

/// Re-resolve the current best config file path by walking the search
/// order and returning the first candidate that exists as a regular file.
///
/// Returns `Some(absolute_path)` if a config file is found in any of
/// the search paths, or `None` if no config file is present anywhere
/// (the daemon will then run with the hardcoded default).
///
/// Used by the auto-reload watcher thread in
/// `commands::daemon::spawn_config_watcher` to detect when a
/// higher-priority config file appears (e.g. the user creates
/// `~/.config/zylaxion/config.toml` while the daemon was already
/// running against `/usr/local/share/zylaxion/config.toml`). Without
/// this re-evaluation, the watcher would be locked to the path it
/// resolved at startup and never notice the new higher-priority file.
///
/// This function performs NO caching — it stats every candidate on
/// every call. It is intended to be called once per poll iteration
/// (1 Hz by default), so the cost is negligible.
pub(crate) fn find_config_path() -> Option<PathBuf> {
    for candidate in candidate_paths() {
        if candidate.is_file() {
            return Some(canonicalise(&candidate));
        }
    }
    None
}

pub(super) fn canonicalise(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
