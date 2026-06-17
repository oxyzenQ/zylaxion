// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Central config resolution: search the filesystem for `config.toml`
//! and load the `[preset.<name>]` table requested by the user.
//!
//! As of v0.3.2, Zylaxion uses a single `config.toml` file containing
//! multiple named `[preset.NAME]` tables. The active preset is selected
//! at startup via `--preset <name>` (default: `technical`). The
//! auto-reload watcher re-reads the same `[preset.<name>]` table on
//! file change.
//!
//! # Search order (first found wins)
//!
//!   1. `~/.config/zylaxion/config.toml`   (user-local override)
//!   2. `/etc/zylaxion/config.toml`         (system config)
//!   3. `/usr/local/share/zylaxion/config.toml` (FHS installed default)
//!   4. `./config.toml`                      (relative to CWD, for dev)
//!   5. Hardcoded default                    (always available)
//!
//! # Returns
//!
//! [`resolve_config`] returns `(ProfileWithOverrides, Option<PathBuf>)`:
//!   - The profile data, with the preset's `[default]` parameters and
//!     any `[[preset.NAME.keys]]` per-scancode overrides already merged
//!     and validated/clamped.
//!   - The **absolute path** of the file the data was loaded from, or
//!     `None` if the hardcoded fallback was used (no file found). The
//!     path is canonicalised so `testconf` can print a stable absolute
//!     path even when invoked from the repo root.

use std::path::{Path, PathBuf};

use zactrix_profiles::{KeyProfile, ProfileWithOverrides};

/// System-wide data directories searched for the installed config.
const SYSTEM_DATA_DIRS: &[&str] = &["/usr/local/share/zylaxion"];

/// Filename of the central config file.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Default preset name when the user does not pass `--preset`.
pub const DEFAULT_PRESET: &str = "technical";

/// Build the ordered list of candidate `config.toml` paths.
///
/// Order matches the documented search path: user-local → system config
/// → FHS installed data → CWD-relative.
fn candidate_paths() -> Vec<PathBuf> {
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

    candidates
}

/// Resolve the central `config.toml`, load the requested preset, and
/// return both the parsed profile and the absolute path it was loaded
/// from (for diagnostics and the auto-reload watcher).
///
/// # Arguments
///
/// * `preset_name` — Name of the `[preset.<name>]` table to load (e.g.
///   `"technical"`, `"cherryMX"`, `"classic"`). If the preset is not
///   found in the TOML, falls back to the hardcoded default and logs a
///   warning.
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
/// A tuple of `(profile, path)` where `path` is `Some(absolute_path)`
/// if the profile was loaded from a file, or `None` if the hardcoded
/// default was used. The path is canonicalised so callers can display a
/// stable absolute path regardless of the CWD at invocation time.
pub fn resolve_config(preset_name: &str) -> (ProfileWithOverrides, Option<PathBuf>) {
    for candidate in candidate_paths() {
        if !candidate.is_file() {
            continue;
        }
        match load_preset_from_file(&candidate, preset_name) {
            Ok(profiles) => {
                log::info!(
                    "loaded preset '{}' from {}",
                    preset_name,
                    candidate.display()
                );
                let abs = canonicalise(&candidate);
                return (profiles, Some(abs));
            }
            Err(e) => {
                eprintln!("[zylaxion] warning: {e}");
            }
        }
    }

    eprintln!(
        "[zylaxion] no config.toml found — using hardcoded default (preset '{}')",
        preset_name
    );
    (
        ProfileWithOverrides {
            default: KeyProfile::default(),
            overrides: std::collections::HashMap::new(),
        },
        None,
    )
}

/// Validate-only variant used by the `zylaxion testconf` subcommand.
///
/// Walks the same search path, finds the first `config.toml`, parses
/// it, and verifies every `[preset.*]` table (not just one) is valid.
/// This catches typos in presets the user is not currently using but
/// might switch to later.
///
/// # Returns
///
/// `Ok(absolute_path)` if every preset parses and validates.
/// `Err((Some(absolute_path), error_message))` if a file was found but
/// parsing or validation failed.
/// `Err((None, error_message))` if no config file was found in any
/// search location.
pub fn validate_config() -> Result<PathBuf, (Option<PathBuf>, String)> {
    let mut last_error: Option<(PathBuf, String)> = None;

    for candidate in candidate_paths() {
        if !candidate.is_file() {
            continue;
        }
        match load_all_presets_from_file(&candidate) {
            Ok(()) => return Ok(canonicalise(&candidate)),
            Err(e) => last_error = Some((candidate.clone(), e)),
        }
    }

    match last_error {
        Some((path, err)) => Err((Some(canonicalise(&path)), err)),
        None => Err((None, "no config.toml found in any search path".to_string())),
    }
}

/// Load a single preset from `config.toml`.
///
/// Parses the file, extracts the `[preset.<preset_name>]` table, merges
/// any `[[preset.<preset_name>.keys]]` overrides, validates+clamps, and
/// returns a `ProfileWithOverrides`.
///
/// Public so the config-watcher thread (in `commands::daemon`) can
/// re-read the same preset on file change.
pub fn load_preset_from_file(
    path: &Path,
    preset_name: &str,
) -> Result<ProfileWithOverrides, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    load_preset_from_str(&content, preset_name)
}

/// Parse a preset from a TOML string.
///
/// Looks up the `[preset.<preset_name>]` table. If not found, returns
/// an error so the caller can decide whether to fall back to the
/// hardcoded default (in `resolve_config`) or surface the error to the
/// user (in `validate_config`).
fn load_preset_from_str(toml_str: &str, preset_name: &str) -> Result<ProfileWithOverrides, String> {
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Deserialize)]
    struct ConfigFile {
        #[serde(default)]
        preset: HashMap<String, PresetEntry>,
    }

    #[derive(Deserialize, Default)]
    struct PresetEntry {
        #[serde(default)]
        click: Option<zactrix_profiles::ClickParams>,
        #[serde(default)]
        spring: Option<zactrix_profiles::SpringParams>,
        #[serde(default)]
        decay: Option<zactrix_profiles::DecayParams>,
        #[serde(default)]
        keys: Vec<zactrix_profiles::KeyOverride>,
    }

    let file: ConfigFile =
        toml::from_str(toml_str).map_err(|e| format!("failed to parse config TOML: {e}"))?;

    let entry = file.preset.get(preset_name).ok_or_else(|| {
        format!(
            "preset '{}' not found in config.toml (available presets: {})",
            preset_name,
            available_presets(&file.preset)
        )
    })?;

    // Build the default KeyProfile: start from hardcoded default, apply
    // any preset-level overrides for click/spring/decay.
    let mut default = KeyProfile::default();
    if let Some(click) = entry.click {
        default.click = click;
    }
    if let Some(spring) = entry.spring {
        default.spring = spring;
    }
    if let Some(decay) = entry.decay {
        default.decay = decay;
    }
    default.validate_and_clamp();

    // Merge per-key overrides on top of the (now-clamped) default.
    let mut overrides = std::collections::HashMap::with_capacity(entry.keys.len());
    for ko in &entry.keys {
        let mut merged = default;
        if let Some(click) = &ko.click {
            if let Some(v) = click.frequency {
                merged.click.frequency = v;
            }
            if let Some(v) = click.resonance {
                merged.click.resonance = v;
            }
            if let Some(v) = click.duration_ms {
                merged.click.duration_ms = v;
            }
            if let Some(v) = click.amplitude {
                merged.click.amplitude = v;
            }
        }
        if let Some(spring) = &ko.spring {
            if let Some(v) = spring.frequency {
                merged.spring.frequency = v;
            }
            if let Some(v) = spring.resonance {
                merged.spring.resonance = v;
            }
            if let Some(v) = spring.mix {
                merged.spring.mix = v;
            }
        }
        if let Some(decay) = &ko.decay {
            if let Some(v) = decay.coefficient {
                merged.decay.coefficient = v;
            }
            if let Some(v) = decay.voice_off_threshold {
                merged.decay.voice_off_threshold = v;
            }
        }
        merged.validate_and_clamp();
        overrides.insert(ko.scancode, merged);
    }

    Ok(ProfileWithOverrides { default, overrides })
}

/// Load and validate EVERY preset in `config.toml` (for `testconf`).
///
/// Used by the `testconf` subcommand so a typo in any preset — even one
/// the user is not currently using — is caught before it can surprise
/// them later.
fn load_all_presets_from_file(path: &Path) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Deserialize)]
    struct ConfigFile {
        #[serde(default)]
        preset: HashMap<String, toml::Value>,
    }

    let file: ConfigFile =
        toml::from_str(&content).map_err(|e| format!("failed to parse config TOML: {e}"))?;

    if file.preset.is_empty() {
        return Err("config.toml contains no [preset.*] tables".to_string());
    }

    // Validate each preset by attempting to load it via load_preset_from_str.
    // We re-serialize each preset's sub-tree to a mini-TOML string and
    // parse it back via the single-preset loader — this exercises the
    // same validate+clamp path as production code.
    let mut preset_names: Vec<&String> = file.preset.keys().collect();
    preset_names.sort();
    for name in preset_names {
        // Re-serialize just this preset's table.
        let mini = toml::to_string(&toml::Value::Table(toml::value::Table::from_iter([(
            "preset".to_string(),
            toml::Value::Table(toml::value::Table::from_iter([(
                name.clone(),
                file.preset[name].clone(),
            )])),
        )])))
        .map_err(|e| format!("internal error: failed to re-serialize preset '{name}': {e}"))?;

        load_preset_from_str(&mini, name).map_err(|e| format!("preset '{name}': {e}"))?;
    }

    Ok(())
}

/// Format the available preset names as a comma-separated list for
/// inclusion in error messages.
fn available_presets(map: &std::collections::HashMap<String, impl Sized>) -> String {
    if map.is_empty() {
        return "(none)".to_string();
    }
    let mut names: Vec<&String> = map.keys().collect();
    names.sort();
    names
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Canonicalise a path to its absolute form for display.
///
/// Falls back to the original path if canonicalisation fails (e.g. the
/// file was just deleted). This is best-effort — callers only need the
/// absolute form for diagnostic messages, not for actually opening the
/// file.
fn canonicalise(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD_TOML: &str = r#"
[preset.technical]
[preset.technical.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[preset.technical.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[preset.technical.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001

[preset.classic]
[preset.classic.click]
frequency = 3200.0
resonance = 2.5
duration_ms = 2.5
amplitude = 0.7
[preset.classic.spring]
frequency = 1200.0
resonance = 4.0
mix = 0.75
[preset.classic.decay]
coefficient = 0.9992
voice_off_threshold = 0.00001
"#;

    #[test]
    fn load_existing_preset_succeeds() {
        let p = load_preset_from_str(GOOD_TOML, "technical").expect("technical should load");
        assert_eq!(p.default.click.frequency, 4500.0);
        assert!(p.overrides.is_empty());
    }

    #[test]
    fn load_second_preset_succeeds() {
        let p = load_preset_from_str(GOOD_TOML, "classic").expect("classic should load");
        assert_eq!(p.default.click.frequency, 3200.0);
        assert_eq!(p.default.spring.mix, 0.75);
    }

    #[test]
    fn load_missing_preset_errors_with_available_list() {
        let err = load_preset_from_str(GOOD_TOML, "nonexistent").unwrap_err();
        assert!(err.contains("preset 'nonexistent' not found"));
        assert!(err.contains("technical"));
        assert!(err.contains("classic"));
    }

    #[test]
    fn load_preset_clamps_invalid_values() {
        let bad = r#"
[preset.bad]
[preset.bad.click]
frequency = 100000.0  # above 8000
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[preset.bad.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[preset.bad.decay]
coefficient = 9999.0  # would cause infinite loop
voice_off_threshold = 0.00001
"#;
        let p = load_preset_from_str(bad, "bad").expect("should still load (after clamping)");
        assert_eq!(p.default.click.frequency, 8000.0);
        assert!(p.default.decay.coefficient < 1.0);
        assert_eq!(p.default.decay.coefficient, 0.9999);
    }

    #[test]
    fn load_preset_with_per_key_override() {
        let toml = r#"
[preset.technical]
[preset.technical.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[preset.technical.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[preset.technical.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001

[[preset.technical.keys]]
scancode = 28
[preset.technical.keys.click]
frequency = 3000.0
"#;
        let p = load_preset_from_str(toml, "technical").expect("should parse");
        assert_eq!(p.overrides.len(), 1);
        let enter = p.for_scancode(28);
        assert_eq!(enter.click.frequency, 3000.0);
        assert_eq!(enter.click.resonance, 2.0); // inherited from default
    }

    #[test]
    fn validate_config_catches_bad_preset() {
        let bad = r#"
[preset.technical]
[preset.technical.click]
frequency = "not-a-number"
"#;
        let result = load_all_presets_from_file(&write_tmp(bad));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("failed to parse config TOML"));
    }

    #[test]
    fn validate_config_rejects_empty_file() {
        let empty = "";
        let result = load_all_presets_from_file(&write_tmp(empty));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no [preset.*] tables"));
    }

    #[test]
    fn validate_config_accepts_good_file() {
        let result = load_all_presets_from_file(&write_tmp(GOOD_TOML));
        assert!(result.is_ok(), "{:?}", result.err());
    }

    /// Helper: write content to a temp file and return the path.
    fn write_tmp(content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "zylaxion-config-test-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, content).expect("write tmp file");
        path
    }
}
