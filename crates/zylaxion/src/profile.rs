// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Profile resolution: search the filesystem for acoustic profile TOMLs.
//!
//! Search order (first found wins):
//!
//!   1. `~/.config/zylaxion/profiles/<name>.toml`   (user-local)
//!   2. `/etc/zylaxion/profiles/<name>.toml`         (system config)
//!   3. `/usr/local/share/zylaxion/profiles/`       (FHS installed data)
//!   4. `./profiles/<name>.toml`                      (relative to CWD, for dev)
//!   5. Hardcoded default                             (always available)
//!
//! The resolver returns a [`ProfileWithOverrides`] (default profile + an
//! optional per-scancode override map). Callers can construct a
//! [`MechanicalClick`](zactrix_profiles::MechanicalClick) from it via
//! [`MechanicalClick::with_overrides`].

use std::path::PathBuf;

use zactrix_profiles::{KeyProfile, ProfileWithOverrides};

/// System-wide data directories searched for installed profiles.
const SYSTEM_DATA_DIRS: &[&str] = &["/usr/local/share/zylaxion/profiles"];

/// Resolve an acoustic profile by name.
///
/// If `name` is `None`, returns a profile with the hardcoded default and
/// no per-key overrides. Otherwise walks the search path list and loads
/// the first `.toml` found. Both the default profile and any per-key
/// overrides are validated and clamped to safe DSP ranges by
/// [`ProfileWithOverrides::from_file`].
///
/// # Fallback behaviour
///
/// If a profile file is found but cannot be read or parsed, a warning is
/// logged and the search continues to the next location. If no file is
/// found in any location, the hardcoded default is returned.
pub fn resolve_profile(name: &Option<String>) -> ProfileWithOverrides {
    let name = match name.as_deref() {
        Some(n) => n,
        None => {
            return ProfileWithOverrides {
                default: KeyProfile::default(),
                overrides: std::collections::HashMap::new(),
            }
        }
    };

    let toml_name = format!("{name}.toml");

    // Build the candidate path list. Order matters — first found wins.
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1. User-local: ~/.config/zylaxion/profiles/<name>.toml
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(
            PathBuf::from(home)
                .join(".config/zylaxion/profiles")
                .join(&toml_name),
        );
    }

    // 2. System config: /etc/zylaxion/profiles/<name>.toml
    candidates.push(PathBuf::from("/etc/zylaxion/profiles").join(&toml_name));

    // 3. FHS installed data directories
    for data_dir in SYSTEM_DATA_DIRS {
        candidates.push(PathBuf::from(data_dir).join(&toml_name));
    }

    // 4. Relative to CWD: ./profiles/<name>.toml (for development)
    candidates.push(PathBuf::from("profiles").join(&toml_name));

    // Walk the list. First file that loads successfully wins; parse
    // failures log a warning and fall through to the next candidate.
    for candidate in &candidates {
        if candidate.is_file() {
            match ProfileWithOverrides::from_file(candidate) {
                Ok(p) => {
                    log::info!("loaded profile '{}' from {}", name, candidate.display());
                    return p;
                }
                Err(e) => {
                    eprintln!("[zylaxion] warning: {e}");
                }
            }
        }
    }

    // 5. Fallback: hardcoded default.
    eprintln!(
        "[zylaxion] profile '{}' not found — using default profile",
        name
    );
    ProfileWithOverrides {
        default: KeyProfile::default(),
        overrides: std::collections::HashMap::new(),
    }
}
