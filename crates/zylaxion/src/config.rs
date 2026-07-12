// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Central config resolution: search the filesystem for `config.toml`,
//! determine the active preset (from CLI `--preset` or `preset.tuning`
//! in the file), and load the matching `[preset.<name>]` table.
//!
//! # Active preset resolution
//!
//! The active preset is determined in this order:
//!
//!   1. `--preset <name>` on the CLI (highest priority — overrides everything)
//!   2. `tuning = "<name>"` in the `[preset]` table of `config.toml`
//!   3. `DEFAULT_PRESET` ("technical") if neither is set
//!
//! If the resolved preset name does NOT exist as a `[preset.<name>]` table
//! in `config.toml`, the program prints a clear error and exits — there
//! is **no silent fallback**. This prevents users from accidentally
//! running with the wrong sound because of a typo.
//!
//! # Auto-reload
//!
//! The config-watcher thread re-reads `config.toml` on mtime change.
//! If `--preset` was passed on the CLI, the watcher always loads that
//! preset. If `--preset` was NOT passed, the watcher re-reads
//! `preset.tuning` from the file — so changing `tuning = "cherryMX"`
//! and saving causes an immediate swap to the cherryMX preset.
//!
//! # Search order (first found wins)
//!
//!   1. `~/.config/zylaxion/config.toml`   (user-local override)
//!   2. `/etc/zylaxion/config.toml`         (system config)
//!   3. `/usr/local/share/zylaxion/config.toml` (FHS installed default)
//!   4. `./config.toml`                      (relative to CWD, for dev)
//!   5. Hardcoded default                    (always available)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use zactrix_profiles::{KeyProfile, ProfileWithOverrides};

/// System-wide data directories searched for the installed config.
const SYSTEM_DATA_DIRS: &[&str] = &["/usr/local/share/zylaxion"];

/// Filename of the central config file.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Default preset name when neither `--preset` nor `preset.tuning` is
/// provided. This is only used as a last resort — if the config file
/// exists and has a `tuning` value, that value is used instead.
pub const DEFAULT_PRESET: &str = "technical";

/// The `tuning` key inside the `[preset]` table.
const TUNING_KEY: &str = "tuning";

// ── Public API ─────────────────────────────────────────────────────────

/// Resolve the central `config.toml`, determine the active preset, and
/// return the profile + the absolute file path + the active preset name.
///
/// # Arguments
///
/// * `cli_preset` — `Some(name)` if `--preset <name>` was passed on the
///   CLI. `None` if the user did not pass `--preset` (in which case the
///   `preset.tuning` value from the file is used).
///
/// # Active preset resolution
///
/// 1. `cli_preset` (if `Some`) — highest priority.
/// 2. `preset.tuning` from the loaded `config.toml`.
/// 3. `DEFAULT_PRESET` ("technical") if neither is set.
///
/// # Errors
///
/// If the resolved preset does NOT exist in `config.toml`, returns
/// `Err(message)` with a clear error listing the available presets. The
/// caller (`cmd_start` / `cmd_daemon`) prints this and exits — there is
/// **no silent fallback** to a different preset.
///
/// If no config file is found in any search path, returns the hardcoded
/// default profile with `path = None` and `preset_name = DEFAULT_PRESET`.
pub fn resolve_config(
    cli_preset: Option<&str>,
) -> Result<
    (
        ProfileWithOverrides,
        Option<PathBuf>,
        String,
        zactrix_profiles::MasterParams,
    ),
    String,
> {
    for candidate in candidate_paths() {
        if !candidate.is_file() {
            continue;
        }
        let content = std::fs::read_to_string(&candidate)
            .map_err(|e| format!("failed to read {}: {e}", candidate.display()))?;

        let parsed = parse_config(&content)?;

        // Determine the active preset name.
        let active = determine_active_preset(cli_preset, &parsed);

        // Look up the preset table.
        let entry = parsed.presets.get(&active).ok_or_else(|| {
            format!(
                "Preset '{}' not found in config.toml. Available: {}",
                active,
                format_preset_list(&parsed.presets)
            )
        })?;

        let profiles = build_profile_from_entry(entry);
        log::info!("loaded preset '{}' from {}", active, candidate.display());

        let abs = canonicalise(&candidate);
        // v10.2.0 (P1): return the master params so the caller can
        // construct the VoicePool with the configured volume.
        return Ok((profiles, Some(abs), active, parsed.master));
    }

    // No config file found — use hardcoded default.
    crate::error_format::warning("no config.toml found — using hardcoded default");
    Ok((
        ProfileWithOverrides {
            default: KeyProfile::default(),
            overrides: HashMap::new(),
        },
        None,
        DEFAULT_PRESET.to_string(),
        zactrix_profiles::MasterParams::default(),
    ))
}

/// Re-load the active preset from a known config file path.
///
/// Used by the auto-reload watcher thread. Re-reads the file and
/// determines the active preset **from the file's `preset.tuning`
/// value** — the `cli_preset` argument is IGNORED here.
///
/// # Why `cli_preset` is ignored on reload
///
/// The `--preset` CLI flag is an *initial-load* override only. Once the
/// daemon is running, `config.toml` is the single source of truth: if
/// the user edits `preset.tuning = "elegant"` and saves, the watcher
/// must swap to `elegant` immediately, even if the daemon was started
/// with `--preset cherryMX`. Treating the CLI flag as permanent would
/// make file edits silently ignored, violating the principle that
/// "save the file = apply the change".
///
/// The `cli_preset` parameter is retained in the signature for API
/// symmetry with `resolve_config` and to make the "ignored on reload"
/// contract explicit in the type. Callers should pass the same value
/// they passed to `resolve_config` at startup; this function will
/// disregard it.
pub fn reload_preset(
    path: &Path,
    _cli_preset: Option<&str>,
) -> Result<(ProfileWithOverrides, String, zactrix_profiles::MasterParams), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    let parsed = parse_config(&content)?;
    // ALWAYS use preset.tuning from the file on reload. The CLI flag
    // was only for the initial load.
    let active = determine_active_preset(None, &parsed);

    let entry = parsed.presets.get(&active).ok_or_else(|| {
        format!(
            "Preset '{}' not found in config.toml. Available: {}",
            active,
            format_preset_list(&parsed.presets)
        )
    })?;

    // v10.2.0 (P1): return the master params so the watcher can
    // update the VoicePool's volume on hot-reload.
    Ok((build_profile_from_entry(entry), active, parsed.master))
}

/// Validate-only: find `config.toml`, parse it, and verify that:
///   - Every `[preset.*]` table parses and clamps correctly.
///   - The `preset.tuning` value (if present) references an existing
///     preset table.
///
/// Used by the `zylaxion testconf` subcommand.
///
/// # Returns
///
/// `Ok(absolute_path)` if the config is fully valid.
/// `Err((Some(absolute_path), error_message))` if a file was found but
/// validation failed.
/// `Err((None, error_message))` if no config file was found.
pub fn validate_config() -> Result<PathBuf, (Option<PathBuf>, String)> {
    let mut last_error: Option<(PathBuf, String)> = None;

    for candidate in candidate_paths() {
        if !candidate.is_file() {
            continue;
        }
        let content = match std::fs::read_to_string(&candidate) {
            Ok(c) => c,
            Err(e) => {
                last_error = Some((candidate.clone(), format!("failed to read: {e}")));
                continue;
            }
        };
        match validate_config_str(&content) {
            Ok(()) => return Ok(canonicalise(&candidate)),
            Err(e) => last_error = Some((candidate.clone(), e)),
        }
    }

    match last_error {
        Some((path, err)) => Err((Some(canonicalise(&path)), err)),
        None => Err((None, "no config.toml found in any search path".to_string())),
    }
}

/// List all presets in `config.toml` + which one is active.
///
/// Used by the `zylaxion list-presets` subcommand.
///
/// # Returns
///
/// `Ok((absolute_path, active_preset, Vec<preset_name>))` on success.
/// `Err(error_message)` if no config file is found or parsing fails.
pub fn list_presets() -> Result<(PathBuf, String, Vec<String>), String> {
    for candidate in candidate_paths() {
        if !candidate.is_file() {
            continue;
        }
        let content = std::fs::read_to_string(&candidate)
            .map_err(|e| format!("failed to read {}: {e}", candidate.display()))?;
        let parsed = parse_config(&content)?;
        let active = parsed.tuning.unwrap_or_else(|| DEFAULT_PRESET.to_string());
        let mut names: Vec<String> = parsed.presets.keys().cloned().collect();
        names.sort();
        return Ok((canonicalise(&candidate), active, names));
    }
    Err("no config.toml found in any search path".to_string())
}

// ── Internal types & parsing ──────────────────────────────────────────

/// Parsed config file: the `tuning` value + all preset tables + master.
struct ParsedConfig {
    /// Value of `preset.tuning` (if present).
    tuning: Option<String>,
    /// All `[preset.<name>]` tables, keyed by preset name.
    presets: HashMap<String, PresetEntry>,
    /// Top-level `[master]` table (v10.2.0+ — dragonzen audit P1).
    /// Defaults to `MasterParams::default()` if absent.
    master: zactrix_profiles::MasterParams,
}

/// A single `[preset.<name>]` table.
#[derive(serde::Deserialize, Default)]
struct PresetEntry {
    #[serde(default)]
    click: Option<zactrix_profiles::ClickParams>,
    #[serde(default)]
    spring: Option<zactrix_profiles::SpringParams>,
    #[serde(default)]
    decay: Option<zactrix_profiles::DecayParams>,
    #[serde(default)]
    ambient: Option<zactrix_profiles::AmbientParams>,
    #[serde(default)]
    housing: Option<zactrix_profiles::HousingParams>,
    #[serde(default)]
    keys: Vec<zactrix_profiles::KeyOverride>,
}

/// Parse a TOML string into a `ParsedConfig`.
///
/// The TOML structure is:
/// ```toml
/// [preset]
/// tuning = "technical"
///
/// [preset.technical]
/// [preset.technical.click]
/// # ...
/// ```
///
/// The `preset` table has a `tuning` string key and sub-tables for each
/// preset. We parse it as `HashMap<String, toml::Value>` and extract
/// `tuning` separately so it doesn't appear as a "preset".
fn parse_config(toml_str: &str) -> Result<ParsedConfig, String> {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct ConfigFile {
        #[serde(default)]
        preset: HashMap<String, toml::Value>,
        // v10.2.0+ (dragonzen audit P1): top-level [master] table.
        // Defaults to MasterParams::default() if absent.
        #[serde(default)]
        master: zactrix_profiles::MasterParams,
    }

    let file: ConfigFile =
        toml::from_str(toml_str).map_err(|e| format!("failed to parse config TOML: {e}"))?;

    // Extract the `tuning` key if it's a string.
    let tuning = file
        .preset
        .get(TUNING_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    // All other keys are preset tables. Parse each one via serde.
    let mut presets: HashMap<String, PresetEntry> = HashMap::new();
    for (name, value) in &file.preset {
        if name == TUNING_KEY {
            continue;
        }
        let entry: PresetEntry = value
            .clone()
            .try_into()
            .map_err(|e| format!("preset '{name}': {e}"))?;
        presets.insert(name.clone(), entry);
    }

    Ok(ParsedConfig {
        tuning,
        presets,
        master: file.master,
    })
}

/// Determine the active preset name.
///
/// Priority: `cli_preset` > `config.tuning` > `DEFAULT_PRESET`.
fn determine_active_preset(cli_preset: Option<&str>, config: &ParsedConfig) -> String {
    if let Some(name) = cli_preset {
        return name.to_string();
    }
    if let Some(name) = &config.tuning {
        return name.clone();
    }
    DEFAULT_PRESET.to_string()
}

/// Build a `ProfileWithOverrides` from a parsed `PresetEntry`.
///
/// Starts from the hardcoded `KeyProfile::default()`, applies the
/// preset's click/spring/decay/ambient/housing overrides, validates+clamps,
/// then merges any `[[keys]]` per-scancode overrides on top.
fn build_profile_from_entry(entry: &PresetEntry) -> ProfileWithOverrides {
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
    if let Some(ambient) = entry.ambient {
        default.ambient = ambient;
    }
    if let Some(housing) = entry.housing {
        default.housing = housing;
    }
    default.validate_and_clamp();

    let mut overrides = HashMap::with_capacity(entry.keys.len());
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
        if let Some(ambient) = &ko.ambient {
            if let Some(v) = ambient.enabled {
                merged.ambient.enabled = v;
            }
            if let Some(v) = ambient.noise_level {
                merged.ambient.noise_level = v;
            }
            if let Some(v) = ambient.noise_decay {
                merged.ambient.noise_decay = v;
            }
        }
        if let Some(housing) = &ko.housing {
            if let Some(v) = housing.frequency {
                merged.housing.frequency = v;
            }
            if let Some(v) = housing.resonance {
                merged.housing.resonance = v;
            }
            if let Some(v) = housing.mix {
                merged.housing.mix = v;
            }
        }
        merged.validate_and_clamp();
        overrides.insert(ko.scancode, merged);
    }

    ProfileWithOverrides { default, overrides }
}

/// Validate every preset in the config + check tuning references a
/// valid preset.
fn validate_config_str(content: &str) -> Result<(), String> {
    let parsed = parse_config(content)?;

    if parsed.presets.is_empty() {
        return Err("config.toml contains no [preset.*] tables".to_string());
    }

    // Validate each preset by building a profile from it (exercises
    // the full validate+clamp path).
    let mut names: Vec<&String> = parsed.presets.keys().collect();
    names.sort();
    for name in &names {
        let entry = &parsed.presets[*name];
        let _ = build_profile_from_entry(entry); // errors would surface as panics from try_into
    }

    // Check that tuning (if present) references an existing preset.
    if let Some(tuning) = &parsed.tuning {
        if !parsed.presets.contains_key(tuning) {
            return Err(format!(
                "preset.tuning = '{}' references a preset that does not exist. Available: {}",
                tuning,
                format_preset_list(&parsed.presets)
            ));
        }
    }

    Ok(())
}

/// Format preset names as a comma-separated list for error messages.
fn format_preset_list(map: &HashMap<String, PresetEntry>) -> String {
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

/// Build the ordered list of candidate `config.toml` paths.
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
pub fn find_config_path() -> Option<PathBuf> {
    for candidate in candidate_paths() {
        if candidate.is_file() {
            return Some(canonicalise(&candidate));
        }
    }
    None
}

/// Canonicalise a path to its absolute form for display.
fn canonicalise(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_PRESET_TOML: &str = r#"
[preset]
tuning = "technical"

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
    fn parse_extracts_tuning_and_presets() {
        let parsed = parse_config(TWO_PRESET_TOML).expect("parse should succeed");
        assert_eq!(parsed.tuning.as_deref(), Some("technical"));
        assert_eq!(parsed.presets.len(), 2);
        assert!(parsed.presets.contains_key("technical"));
        assert!(parsed.presets.contains_key("classic"));
    }

    #[test]
    fn parse_handles_missing_tuning() {
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
"#;
        let parsed = parse_config(toml).expect("parse should succeed");
        assert!(parsed.tuning.is_none());
        assert_eq!(parsed.presets.len(), 1);
    }

    #[test]
    fn determine_active_preset_cli_overrides_tuning() {
        let parsed = parse_config(TWO_PRESET_TOML).unwrap();
        let active = determine_active_preset(Some("classic"), &parsed);
        assert_eq!(active, "classic");
    }

    #[test]
    fn determine_active_preset_uses_tuning_when_no_cli() {
        let parsed = parse_config(TWO_PRESET_TOML).unwrap();
        let active = determine_active_preset(None, &parsed);
        assert_eq!(active, "technical");
    }

    #[test]
    fn determine_active_preset_defaults_when_no_tuning() {
        let toml = r#"
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
        let parsed = parse_config(toml).unwrap();
        let active = determine_active_preset(None, &parsed);
        assert_eq!(active, DEFAULT_PRESET);
    }

    #[test]
    fn resolve_config_with_existing_preset_succeeds() {
        if !Path::new("config.toml").exists() {
            return; // skip if not running from repo root
        }
        let (profiles, _path, active, _master) =
            resolve_config(Some("classic")).expect("should resolve classic");
        assert_eq!(active, "classic");
        assert_eq!(profiles.default.click.frequency, 3200.0);
    }

    #[test]
    fn resolve_config_with_nonexistent_preset_errors() {
        if !Path::new("config.toml").exists() {
            return; // skip if not running from repo root
        }
        let result = resolve_config(Some("nonexistent"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Preset 'nonexistent' not found"));
        assert!(err.contains("Available:"));
    }

    #[test]
    fn resolve_config_uses_tuning_when_no_cli() {
        if !Path::new("config.toml").exists() {
            return;
        }
        let (_profiles, _path, active, _master) =
            resolve_config(None).expect("should resolve via tuning");
        assert_eq!(active, "technical");
    }

    #[test]
    fn reload_preset_ignores_cli_preset_and_uses_tuning() {
        // The --preset CLI flag is for INITIAL load only. On reload,
        // preset.tuning from the file is the single source of truth.
        // This test verifies that passing Some("cherryMX") to
        // reload_preset does NOT override the file's tuning="technical".
        let path = Path::new("config.toml");
        if !path.exists() {
            return; // skip if not running from repo root
        }
        let (_profiles, active, _master) = reload_preset(path, Some("cherryMX"))
            .expect("should reload from tuning, ignoring cli_preset");
        // The file's tuning is "technical", so reload must return
        // "technical" — NOT "cherryMX" from the cli_preset arg.
        assert_eq!(active, "technical");
    }

    #[test]
    fn reload_preset_uses_tuning_when_no_cli() {
        let path = Path::new("config.toml");
        if !path.exists() {
            return;
        }
        let (_profiles, active, _master) =
            reload_preset(path, None).expect("should reload via tuning");
        assert_eq!(active, "technical");
    }

    #[test]
    fn reload_preset_follows_tuning_change_in_file() {
        // Simulate the user editing config.toml: write a temp file with
        // tuning = "classic", reload, and verify the active preset is
        // "classic" — even when cli_preset is Some("technical").
        let toml = r#"
[preset]
tuning = "classic"

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
        let path = std::env::temp_dir().join(format!(
            "zylaxion-reload-test-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, toml).expect("write tmp");

        // Pass Some("technical") as cli_preset — reload MUST ignore it
        // and use the file's tuning = "classic".
        let (profiles, active, _master) =
            reload_preset(&path, Some("technical")).expect("should reload classic");
        assert_eq!(active, "classic");
        assert_eq!(profiles.default.click.frequency, 3200.0);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn validate_config_str_accepts_good_file() {
        assert!(validate_config_str(TWO_PRESET_TOML).is_ok());
    }

    #[test]
    fn validate_config_str_rejects_bad_tuning_reference() {
        let toml = r#"
[preset]
tuning = "nonexistent"

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
"#;
        let result = validate_config_str(toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("preset.tuning = 'nonexistent'"));
        assert!(err.contains("Available:"));
    }

    #[test]
    fn validate_config_str_rejects_empty_file() {
        assert!(validate_config_str("").is_err());
    }

    #[test]
    fn validate_config_str_rejects_no_presets() {
        let toml = r#"
[preset]
tuning = "technical"
"#;
        let result = validate_config_str(toml);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no [preset.*] tables"));
    }

    #[test]
    fn list_presets_returns_all_names() {
        let path = Path::new("config.toml");
        if !path.exists() {
            return;
        }
        let (path, active, names) = list_presets().expect("should list presets");
        assert!(path.is_absolute());
        assert!(!names.is_empty());
        assert!(names.contains(&active.to_string()));
    }

    #[test]
    fn build_profile_clamps_invalid_values() {
        use zactrix_profiles::{ClickParams, DecayParams, SpringParams};
        let entry = PresetEntry {
            click: Some(ClickParams {
                frequency: 100_000.0, // above 8000
                resonance: 2.0,
                duration_ms: 1.5,
                amplitude: 0.8,
            }),
            spring: Some(SpringParams {
                frequency: 1800.0,
                resonance: 3.5,
                mix: 0.6,
            }),
            decay: Some(DecayParams {
                coefficient: 9999.0, // would cause infinite loop
                voice_off_threshold: 0.00001,
            }),
            ambient: None,
            housing: None,
            keys: vec![],
        };
        let profiles = build_profile_from_entry(&entry);
        assert_eq!(profiles.default.click.frequency, 8000.0);
        assert!(profiles.default.decay.coefficient < 1.0);
    }

    #[test]
    fn build_profile_merges_per_key_overrides() {
        use zactrix_profiles::{ClickParams, DecayParams, KeyOverride, SpringParams};
        let entry = PresetEntry {
            click: Some(ClickParams {
                frequency: 4500.0,
                resonance: 2.0,
                duration_ms: 1.5,
                amplitude: 0.8,
            }),
            spring: Some(SpringParams {
                frequency: 1800.0,
                resonance: 3.5,
                mix: 0.6,
            }),
            decay: Some(DecayParams {
                coefficient: 0.9994,
                voice_off_threshold: 0.00001,
            }),
            ambient: None,
            housing: None,
            keys: vec![KeyOverride {
                scancode: 28,
                click: Some(zactrix_profiles::OverrideClick {
                    frequency: Some(3000.0),
                    ..Default::default()
                }),
                ..Default::default()
            }],
        };
        let profiles = build_profile_from_entry(&entry);
        assert_eq!(profiles.overrides.len(), 1);
        let enter = profiles.for_scancode(28);
        assert_eq!(enter.click.frequency, 3000.0);
        assert_eq!(enter.click.resonance, 2.0); // inherited
    }
}
