// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Active-preset resolution (night-improve-22, the no-config law).
//!
//! night-improve-22 deletes `config.toml` and its whole support stack
//! — the filesystem search path, the TOML parser, the mtime watcher
//! thread, the `testconf` validator. The owner's verdict stands: a
//! file format plus a parser plus a watcher plus a validation
//! subcommand is absurd maintenance weight for choosing one of
//! twenty-four sounds. The masterclass replacement is the Unix
//! configuration surface itself:
//!
//!   1. `--preset <name>` on the CLI (highest priority)
//!   2. `ZYLAXION_PRESET=<name>` in the environment (persistent
//!      selection — shell profile, systemd `Environment=`, wrapper
//!      script; the standard env-var contract, zero parsing of ours)
//!   3. the compiled default preset ("technical") from the registry
//!
//! There is no fourth layer at process start: a preset is resolved
//! once when a surface boots. While the player runs, the runtime
//! `zylaxion preset <name>` command applies changes live — it writes
//! the preset slot of the pinned control map and the player's watcher
//! thread swaps the model through the ArcSwap within 100 ms, no
//! restart involved.
//!
//! The master output volume follows the same law: `ZYLAXION_VOLUME`
//! in the environment, the compiled laptop-speaker default (5.5)
//! otherwise. An invalid value is a hard error — the no-silent-
//! fallback contract the config era honored survives here.
//!
//! Unknown preset names produce the canonical GNU-shaped error with
//! the did-you-mean tip (the night-improve-19 value-suggestion
//! lineage), listing every registered preset so the message can never
//! drift from the catalog.

use crate::profiles::{DEFAULT_PRESET, KeyProfile, registry_lookup, registry_names};

/// The environment variable holding the persistent preset selection.
pub const PRESET_ENV_VAR: &str = "ZYLAXION_PRESET";

/// The environment variable holding the master output volume override.
pub const VOLUME_ENV_VAR: &str = "ZYLAXION_VOLUME";

/// The master-volume ceiling as a percent of the compiled default
/// (night-improve-27: the owner-mandated safety cap). 100 percent is
/// the compiled laptop-speaker tuning; 200 percent is the loudest
/// value any surface accepts — a volume write above the cap is
/// refused, never clamped, so the map never carries a value the user
/// did not ask for in writing. Both write entries enforce it: the
/// `volume` verb at the CLI parser (exit 2, the fatal-input lane) and
/// the `ZYLAXION_VOLUME` env lane in `context_volume_percent` (a hard
/// error, the no-silent-fallback law).
pub const MAX_VOLUME_PERCENT: u32 = 200;

/// Resolve the active preset and master volume.
///
/// Priority: `cli_preset` > `ZYLAXION_PRESET` > the compiled default.
///
/// # Returns
///
/// `(profile, preset_name, master_volume)` on success. The profile is
/// the registry's validated DSP table — compile-time data, already
/// proven in-range by the registry test tree, so no runtime clamping
/// pass is needed here.
///
/// # Errors
///
/// Unknown preset name (from the flag or the environment) or an
/// unparsable `ZYLAXION_VOLUME` — both are hard errors. The caller
/// (`cmd_on` / `cmd_preset`) prints the message and exits; there
/// is **no silent fallback** to a different preset or volume.
pub fn resolve(cli_preset: Option<&str>) -> Result<(KeyProfile, &'static str, f32), String> {
    let preset_env = std::env::var(PRESET_ENV_VAR).ok();
    let volume_env = std::env::var(VOLUME_ENV_VAR).ok();
    resolve_from(cli_preset, preset_env.as_deref(), volume_env.as_deref())
}

/// The pure resolution core: same contract as [`resolve`], with the
/// environment values passed in explicitly. All decision and error
/// formatting logic lives here so the test tree can pin it without
/// touching the process environment (night-improve-22: env mutation
/// races concurrent env readers — the edition-2024 hazard — so the
/// pure core keeps the suite deterministic; the env-reading shell
/// above it is one thin integration test).
pub fn resolve_from(
    cli_preset: Option<&str>,
    preset_env: Option<&str>,
    volume_env: Option<&str>,
) -> Result<(KeyProfile, &'static str, f32), String> {
    let (preset, source) = match cli_preset {
        Some(name) => (lookup_or_error(name, "the --preset flag")?, "--preset flag"),
        None => match preset_env.map(str::trim).filter(|v| !v.is_empty()) {
            Some(name) => (
                lookup_or_error(name, &format!("the {PRESET_ENV_VAR} environment variable"))?,
                PRESET_ENV_VAR,
            ),
            None => (
                registry_lookup(DEFAULT_PRESET)
                    .expect("compiled DEFAULT_PRESET must exist in the registry"),
                "compiled default",
            ),
        },
    };
    let volume = master_volume_from(volume_env)?;
    log::info!("preset '{}' selected via {source}", preset.name);
    Ok((preset.profile, preset.name, volume))
}

/// The preset name the resolver would pick right now without a CLI
/// flag: the environment selection when it names a registered preset,
/// the compiled default otherwise. A malformed environment value does
/// NOT fail here — the diagnostic surfaces at `resolve` time when a
/// session actually starts. Used by `list-presets` and the overview
/// to mark the active row.
// night-improve-22: consumed by the commands tree wired in this same
// night (list-presets, overview).
#[allow(dead_code)]
pub fn context_preset_name() -> &'static str {
    let preset_env = std::env::var(PRESET_ENV_VAR).ok();
    context_preset_name_from(preset_env.as_deref())
}

/// The pure core of [`context_preset_name`]: same contract, the
/// environment value passed in explicitly (no process-env access —
/// see [`resolve_from`] for the race-free testing rationale).
// night-improve-22: consumed by the commands tree wired in this same
// night (list-presets, overview).
#[allow(dead_code)]
pub fn context_preset_name_from(preset_env: Option<&str>) -> &'static str {
    match preset_env.map(str::trim).filter(|v| !v.is_empty()) {
        Some(name) => match registry_lookup(name) {
            Some(p) => p.name,
            None => DEFAULT_PRESET,
        },
        None => DEFAULT_PRESET,
    }
}

/// The master output volume: `ZYLAXION_VOLUME` if set and parseable,
/// the compiled laptop-speaker default (5.5) otherwise. An empty or
/// whitespace value counts as unset. Range clamping stays where it
/// has always lived — `VoicePool::with_volume` clamps to [-100, 100]
/// with the hard sample clamp as the final ceiling.
pub fn master_volume() -> Result<f32, String> {
    let volume_env = std::env::var(VOLUME_ENV_VAR).ok();
    master_volume_from(volume_env.as_deref())
}

/// The pure core of [`master_volume`]: same contract, the environment
/// value passed in explicitly (see [`resolve_from`] for the rationale).
pub fn master_volume_from(volume_env: Option<&str>) -> Result<f32, String> {
    let default = crate::engine::VoicePool::DEFAULT_MASTER_VOLUME;
    let raw = match volume_env {
        None => return Ok(default),
        Some(raw) => raw,
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    let value: f32 = trimmed.parse().map_err(|_| {
        format!(
            "invalid {VOLUME_ENV_VAR} value '{raw}': must be a number \
             (examples: 5.5 laptop speakers, 3.0 monitor speakers, \
             1.5 headphones, 0.5 subtle background)"
        )
    })?;
    if !value.is_finite() {
        return Err(format!(
            "invalid {VOLUME_ENV_VAR} value '{raw}': must be finite"
        ));
    }
    Ok(value)
}

/// Look a name up in the registry or produce the canonical
/// preset-not-found error: the GNU-shaped message, the full available
/// list, and the did-you-mean tip when a preset within edit distance
/// 2 exists (night-improve-19, the zelynic value-suggestion lineage).
fn lookup_or_error(name: &str, source: &str) -> Result<&'static crate::profiles::Preset, String> {
    registry_lookup(name).ok_or_else(|| preset_not_found_error(name, source))
}

/// The shared preset-not-found constructor — every lookup site goes
/// through it so the tip can never drift from the list.
fn preset_not_found_error(name: &str, source: &str) -> String {
    let names = registry_names();
    let mut msg = format!(
        "Preset '{name}' not found (from {source}). Available: {}",
        names.join(", ")
    );
    if let Some(best) = crate::cli::suggestion::closest_value_match(name, &names) {
        msg.push_str(&format!("\ntip: did you mean '{best}'?"));
    }
    msg
}

#[cfg(test)]
#[path = "../../test/session/preset_tests.rs"]
mod tests;
