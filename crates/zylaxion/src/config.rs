// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Central config resolution: search the filesystem for `config.toml`.
//!
//! As of v0.3.1, Zylaxion uses a single `config.toml` file (replacing
//! the per-profile `profiles/<name>.toml` scheme). The file contains a
//! mandatory `[default]` table and optional `[[keys]]` per-scancode
//! overrides. See the repo-root `config.toml` (installed to
//! `/usr/local/share/zylaxion/config.toml`) for a documented example.
//!
//! # Search order (first found wins)
//!
//!   1. `~/.config/zylaxion/config.toml`   (user-local override)
//!   2. `/etc/zylaxion/config.toml`         (system config)
//!   3. `/usr/local/share/zylaxion/config.toml` (FHS installed default)
//!   4. `./config.toml`                      (relative to CWD, for dev)
//!   5. Hardcoded default                    (always available)
//!
//! The resolver returns a [`ProfileWithOverrides`] (default profile + an
//! optional per-scancode override map). Callers can construct a
//! [`MechanicalClick`](zactrix_profiles::MechanicalClick) from it via
//! [`MechanicalClick::with_overrides`].

use std::path::PathBuf;

use zactrix_profiles::{KeyProfile, ProfileWithOverrides};

/// System-wide data directories searched for the installed config.
const SYSTEM_DATA_DIRS: &[&str] = &["/usr/local/share/zylaxion"];

/// Filename of the central config file.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Resolve the central `config.toml` and return both the parsed profile
/// and the path it was loaded from (for diagnostics and the auto-reload
/// watcher).
///
/// Both the default profile and any per-key overrides are validated and
/// clamped to safe DSP ranges by [`ProfileWithOverrides::from_file`].
///
/// # Fallback behaviour
///
/// If a config file is found but cannot be read or parsed, a warning is
/// logged and the search continues to the next location. If no file is
/// found in any location, the hardcoded default is returned with
/// `path = None`.
///
/// # Returns
///
/// A tuple of `(profile, path)` where `path` is `Some(p)` if the
/// profile was loaded from a file (so the auto-reload watcher can poll
/// its mtime), or `None` if the hardcoded default was used (no file to
/// watch).
pub fn resolve_config() -> (ProfileWithOverrides, Option<PathBuf>) {
    // Build the candidate path list. Order matters — first found wins.
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1. User-local: ~/.config/zylaxion/config.toml
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(
            PathBuf::from(home)
                .join(".config/zylaxion")
                .join(CONFIG_FILE_NAME),
        );
    }

    // 2. System config: /etc/zylaxion/config.toml
    candidates.push(PathBuf::from("/etc/zylaxion").join(CONFIG_FILE_NAME));

    // 3. FHS installed data directories
    for data_dir in SYSTEM_DATA_DIRS {
        candidates.push(PathBuf::from(data_dir).join(CONFIG_FILE_NAME));
    }

    // 4. Relative to CWD: ./config.toml (for development from repo root)
    candidates.push(PathBuf::from(CONFIG_FILE_NAME));

    // Walk the list. First file that loads successfully wins; parse
    // failures log a warning and fall through to the next candidate.
    for candidate in &candidates {
        if candidate.is_file() {
            match ProfileWithOverrides::from_file(candidate) {
                Ok(p) => {
                    log::info!("loaded config from {}", candidate.display());
                    return (p, Some(candidate.clone()));
                }
                Err(e) => {
                    eprintln!("[zylaxion] warning: {e}");
                }
            }
        }
    }

    // 5. Fallback: hardcoded default.
    eprintln!("[zylaxion] no config.toml found — using hardcoded default");
    (
        ProfileWithOverrides {
            default: KeyProfile::default(),
            overrides: std::collections::HashMap::new(),
        },
        None,
    )
}

/// Validate-only variant of [`resolve_config`]: find the config file,
/// parse and validate it, but do NOT construct a `MechanicalClick`.
///
/// Used by the `zylaxion testconf` subcommand.
///
/// # Returns
///
/// `Ok(path)` if the config parses and validates successfully.
/// `Err((path_attempted, error_message))` if parsing or validation
/// fails. The path is the candidate that was tried (for the error
/// message); `None` means no config file was found in any search
/// location.
pub fn validate_config() -> Result<PathBuf, (Option<PathBuf>, String)> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(
            PathBuf::from(home)
                .join(".config/zylaxion")
                .join(CONFIG_FILE_NAME),
        );
    }
    candidates.push(PathBuf::from("/etc/zylaxion").join(CONFIG_FILE_NAME));
    for data_dir in SYSTEM_DATA_DIRS {
        candidates.push(PathBuf::from(data_dir).join(CONFIG_FILE_NAME));
    }
    candidates.push(PathBuf::from(CONFIG_FILE_NAME));

    let mut last_error: Option<(PathBuf, String)> = None;

    for candidate in &candidates {
        if candidate.is_file() {
            match ProfileWithOverrides::from_file(candidate) {
                Ok(_p) => return Ok(candidate.clone()),
                Err(e) => {
                    last_error = Some((candidate.clone(), e));
                }
            }
        }
    }

    match last_error {
        Some((path, err)) => Err((Some(path), err)),
        None => Err((None, "no config.toml found in any search path".to_string())),
    }
}
