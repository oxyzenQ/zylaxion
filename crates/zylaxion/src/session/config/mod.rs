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
/// Filename of the central config file.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Hardcoded fallback preset name used when:
/// (a) no `--preset` CLI flag was passed, AND
/// (b) the resolved config file has no `preset.tuning` value, OR
/// (c) no config file was found in any search path.
///
/// This is the **last-resort** default — the shipping `config.toml`
/// sets `preset.tuning = "ibm_model_m"`, so in a normal install this
/// constant is never reached. It exists only as a safety net for
/// stripped-down installs that ship a config without a `tuning` key.
///
/// v10.2.0 (dragonzen audit I3): the value remains "technical" (the
/// classic crisp click) rather than "ibm_model_m" because the
/// fallback should be a known-good universally-available preset, not
/// the showcase preset that may not exist in stripped configs. If
/// you change this, also update the comment in `config.toml`'s
/// `[preset]` block to keep them in sync.
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
        let entry = parsed
            .presets
            .get(&active)
            .ok_or_else(|| preset_not_found_error(&active, &parsed.presets))?;

        let profiles = build_profile_from_entry(entry);
        log::info!("loaded preset '{}' from {}", active, candidate.display());

        let abs = canonicalise(&candidate);
        // v10.2.0 (P1): return the master params so the caller can
        // construct the VoicePool with the configured volume.
        return Ok((profiles, Some(abs), active, parsed.master));
    }

    // No config file found — use hardcoded default.
    crate::output::warning("no config.toml found — using hardcoded default");
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

    let entry = parsed
        .presets
        .get(&active)
        .ok_or_else(|| preset_not_found_error(&active, &parsed.presets))?;

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
            // v10.2.0+ (N7): two-stage decay per-key overrides.
            if let Some(v) = decay.coefficient_fast {
                merged.decay.coefficient_fast = v;
            }
            if let Some(v) = decay.fast_samples_ms {
                merged.decay.fast_samples_ms = v;
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

    // Validate each preset by building a profile from it. This
    // exercises the full parse + validate + clamp path — if any
    // preset's sub-tables have malformed types (e.g. `frequency =
    // "not a number"`), the error surfaces here as a parse error
    // rather than later at runtime.
    //
    // v10.2.0 (dragonzen audit P6): the previous `let _ = ...` was
    // misleading — `build_profile_from_entry` doesn't return Result,
    // so "errors would surface as panics from try_into" was wrong.
    // The try_into happens inside `parse_config` (already done above),
    // so by the time we get here the preset is structurally valid.
    // The build call here exercises validate_and_clamp for range
    // checking. We discard the result because we only care that it
    // doesn't panic — clamping is non-fatal and emits log::warn!
    // internally.
    let mut names: Vec<&String> = parsed.presets.keys().collect();
    names.sort();
    for name in &names {
        let entry = &parsed.presets[*name];
        let _profile = build_profile_from_entry(entry);
    }

    // Check that tuning (if present) references an existing preset.
    // night-improve-4 (edition 2024): let-chains collapse the nested
    // guard clippy flagged as collapsible_if.
    if let Some(tuning) = &parsed.tuning
        && !parsed.presets.contains_key(tuning)
    {
        return Err(format!(
            "preset.tuning = '{}' references a preset that does not exist. Available: {}",
            tuning,
            format_preset_list(&parsed.presets)
        ));
    }

    Ok(())
}

/// Validate a specific config file (v10.2.0+ — user feedback).
///
/// Reads + parses + validates the given file. Does NOT search the
/// standard paths — the caller specifies the exact file to check.
/// Used by `zylaxion testconf -f <path>`.
pub fn validate_config_file(path: &std::path::Path) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    validate_config_str(&content)
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

/// The canonical preset-not-found error: the GNU-shaped message plus
/// the did-you-mean tip when a preset within edit distance 2 exists
/// (night-improve-19, the zelynic value-suggestion lineage). Both
/// lookup sites share this constructor so the tip cannot drift from
/// the list.
fn preset_not_found_error(active: &str, presets: &HashMap<String, PresetEntry>) -> String {
    let mut msg = format!(
        "Preset '{}' not found in config.toml. Available: {}",
        active,
        format_preset_list(presets)
    );
    let names: Vec<&str> = presets.keys().map(String::as_str).collect();
    if let Some(best) = crate::cli::suggestion::closest_value_match(active, &names) {
        msg.push_str(&format!("\ntip: did you mean '{best}'?"));
    }
    msg
}

mod paths;

pub(crate) use paths::find_config_path;
use paths::{candidate_paths, canonicalise};

#[cfg(test)]
#[path = "../../../test/session/config_tests.rs"]
mod tests;
