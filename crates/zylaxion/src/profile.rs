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

use std::path::PathBuf;

use zactrix_profiles::{load_profile_from_file, KeyProfile};

/// System-wide data directories searched for installed profiles.
const SYSTEM_DATA_DIRS: &[&str] = &["/usr/local/share/zylaxion/profiles"];

/// Resolve an acoustic profile by name.
///
/// If `name` is `None`, returns the hardcoded default immediately.
/// Otherwise walks the search path list and loads the first `.toml` found.
pub fn resolve_profile(name: &Option<String>) -> KeyProfile {
    let name = match name.as_deref() {
        Some(n) => n,
        None => return KeyProfile::default(),
    };

    let toml_name = format!("{name}.toml");

    // 1. User-local: ~/.config/zylaxion/profiles/<name>.toml
    if let Some(home) = std::env::var_os("HOME") {
        let user_path = PathBuf::from(home)
            .join(".config/zylaxion/profiles")
            .join(&toml_name);
        if user_path.is_file() {
            match load_profile_from_file(&user_path) {
                Ok(p) => {
                    log::info!("loaded profile '{}' from {}", name, user_path.display());
                    return p;
                }
                Err(e) => {
                    eprintln!("[zylaxion] warning: {e}");
                }
            }
        }
    }

    // 2. System config: /etc/zylaxion/profiles/<name>.toml
    let etc_path = PathBuf::from("/etc/zylaxion/profiles").join(&toml_name);
    if etc_path.is_file() {
        match load_profile_from_file(&etc_path) {
            Ok(p) => {
                log::info!("loaded profile '{}' from {}", name, etc_path.display());
                return p;
            }
            Err(e) => {
                eprintln!("[zylaxion] warning: {e}");
            }
        }
    }

    // 3. FHS installed data directories
    for data_dir in SYSTEM_DATA_DIRS {
        let data_path = PathBuf::from(data_dir).join(&toml_name);
        if data_path.is_file() {
            match load_profile_from_file(&data_path) {
                Ok(p) => {
                    log::info!("loaded profile '{}' from {}", name, data_path.display());
                    return p;
                }
                Err(e) => {
                    eprintln!("[zylaxion] warning: {e}");
                }
            }
        }
    }

    // 4. Relative to CWD: ./profiles/<name>.toml (for development)
    let cwd_path = PathBuf::from("profiles").join(&toml_name);
    if cwd_path.is_file() {
        match load_profile_from_file(&cwd_path) {
            Ok(p) => {
                log::info!("loaded profile '{}' from {}", name, cwd_path.display());
                return p;
            }
            Err(e) => {
                eprintln!("[zylaxion] warning: {e}");
            }
        }
    }

    // 5. Fallback: hardcoded default.
    eprintln!(
        "[zylaxion] profile '{}' not found — using default profile",
        name
    );
    KeyProfile::default()
}
