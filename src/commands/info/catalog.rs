// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The compiled-catalog surfaces: `list-presets` and `list-backends`.

use cpal::traits::{DeviceTrait, HostTrait};

use crate::session::preset;

use super::alsa_guard::QuietAlsa;
use super::{active_preset_name, live_session_preset_name};

/// List every acoustic preset in the compiled registry and mark the
/// one that is active right now: the LIVE session's preset when a
/// player holds the pinned control plane — the map slot is what is
/// actually sounding, so a live session outranks the would-start
/// resolution (night-hunter-2, the stale `<- active` marker fix) —
/// and the would-start resolution (`ZYLAXION_PRESET` when it names a
/// registered preset, the compiled default otherwise) when no player
/// is running.
///
/// Exits 0 always — the registry is compile-time data; there is no
/// file to fail finding and no parse to fail.
pub fn cmd_list_presets() {
    let active = active_preset_name(live_session_preset_name());
    let env_name = std::env::var(preset::PRESET_ENV_VAR)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    println!("{}\n", crate::output::section("Available presets"));
    print!("{}", presets_table_from(active, env_name.as_deref()));
}

/// The pure render behind `list-presets` (night-improve-29): ONE line
/// per preset — the name in a fixed-width column, the description
/// beside it, the session-active row marked at the end — and no
/// how-to footer (the help and the overview teach switching; a list
/// lists). The env honesty note names an override that names nothing.
pub(super) fn presets_table_from(active: &str, env_preset: Option<&str>) -> String {
    let registry = crate::profiles::registry_all();
    let width = registry.iter().map(|e| e.name.len()).max().unwrap_or(0);
    let mut out = String::new();
    for entry in registry.iter() {
        out.push_str("  ");
        out.push_str(&format!("{:<width$}", entry.name));
        out.push_str("  ");
        out.push_str(entry.description);
        if entry.name == active {
            out.push_str(&crate::output::ok_marker("  <- active"));
        }
        out.push('\n');
    }
    // The honesty note: an env selection that names nothing renders
    // as if it were absent (the compiled default is marked) — the
    // note names the truth so the marker cannot mislead.
    if let Some(env_name) = env_preset
        && !registry.iter().any(|e| e.name == env_name)
    {
        out.push_str(&crate::output::tip_line(&format!(
            "\nnote: {}=\"{env_name}\" names no registered preset — using {active}",
            preset::PRESET_ENV_VAR
        )));
        out.push('\n');
    }
    out
}

/// List the audio surface via cpal (night-improve-29 render): the
/// host, then the default output in the SAME compact shape the
/// overview and the doctor carry — one device, one line. The probe
/// rides the quiet-ALSA guard; on a host with no sound card the
/// honest verdict line replaces libasound's raw stderr spew.
pub fn cmd_list_backends() {
    // The report opens with the family banner (the zelynic output
    // law): one frame, one key/value geometry, same as the doctor and
    // the overview.
    println!("{}\n", crate::output::section("Audio backends"));
    #[cfg(target_os = "linux")]
    let _quiet = QuietAlsa::new();
    let host = cpal::default_host();
    println!("{}", crate::output::kv("host:", host.id().name()));
    match host.default_output_device() {
        Some(device) => {
            // night-dinner-10: DeviceTrait::name() became description() in cpal
            // 0.18 (a DeviceDescription with a stable id); render the same
            // bare name the CLI contract always shown, same fallback.
            let name = device
                .description()
                .map(|d| d.name().to_owned())
                .unwrap_or_else(|_| "<unknown>".into());
            match device.default_output_config() {
                Ok(config) => println!(
                    "{}",
                    crate::output::kv(
                        "output:",
                        &format!(
                            "{name} ({} Hz, {}ch, {:?})",
                            config.sample_rate(),
                            config.channels(),
                            config.sample_format()
                        )
                    )
                ),
                Err(e) => {
                    // night-dragon-maintenance-4: a present-but-closed
                    // backend is a caution — the warning yellow tier
                    // names it (the fact stays plain, the verdict
                    // carries the lane).
                    println!(
                        "{}",
                        crate::output::kv(
                            "output:",
                            &format!(
                                "{name} {}",
                                crate::output::warn_line(&format!(
                                    "(present but not openable: {e})"
                                ))
                            )
                        )
                    )
                }
            }
        }
        None => println!(
            "{}",
            crate::output::kv("output:", &crate::output::warn_line("none found"))
        ),
    }
}
