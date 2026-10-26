// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The no-subcommand overview: the compact one-screen posture report.

use cpal::traits::{DeviceTrait, HostTrait};

use crate::ebpf::pin;
use crate::session::preset;

use super::active_preset_name;
use super::alsa_guard::QuietAlsa;

/// Quick status overview shown when `zylaxion` is run with no
/// subcommand (v10.2.0+ — user feedback). night-improve-23 rewrite:
/// the daemon lines are gone; the tap residency, the player state, and
/// the active posture replace them — the same facts `status` prints,
/// in the compact no-subcommand shape.
pub fn cmd_overview() {
    println!("{}\n", crate::output::section("zylaxion"));

    // 1. Kernel verdict (both lanes — the kernel probe has no deps).
    //    night-hunter-4: the verdict rides the semantic tier, the same
    //    shape the status line carries.
    let kernel = crate::session::kernel::probe();
    let verdict = if kernel.supported() {
        crate::output::ok_line(kernel.verdict())
    } else {
        crate::output::fail_line(kernel.verdict())
    };
    println!(
        "{}",
        crate::output::kv("kernel:", &format!("{} ({})", kernel.release, verdict))
    );

    // 2. Tap residency + active posture. The posture reads the control plane
    //    through the named slot constants (night-hunter-2 — the raw
    //    numbers here predate the crate-visible constants and could
    //    drift from the verbs' own mirror).
    use crate::commands::sound::{
        ENABLED_SLOT, PLAYER_PID_SLOT, PRESET_ID_SLOT, TAP_KIND_SLOT, VOLUME_PCT_SLOT,
    };
    if !pin::any_pin_present() {
        println!(
            "{}",
            crate::output::kv(
                "tap:",
                &crate::output::tip_line("not attached (sudo zylaxion attach)")
            )
        );
    } else if pin::all_pins_present() {
        // The chain step that owns the tap (night-improve-25) —
        // the same TAP_KIND read the status line carries.
        let step = crate::commands::sound::open_control()
            .ok()
            .and_then(|c| c.get(&TAP_KIND_SLOT, 0).ok())
            .and_then(crate::ebpf::loader::TapKind::from_slot)
            .map(crate::ebpf::loader::TapKind::label)
            .unwrap_or("unknown step");
        println!(
            "{}",
            crate::output::kv(
                "tap:",
                &format!("{} ({step})", crate::output::ok_line("attached"))
            )
        );
    } else {
        println!(
            "{}",
            crate::output::kv(
                "tap:",
                &crate::output::warn_line("PARTIAL pin set (sudo zylaxion attach replants it)")
            )
        );
    }
    if let Ok(control) = crate::commands::sound::open_control() {
        let enabled = control.get(&ENABLED_SLOT, 0).unwrap_or(0) == 1;
        let pid = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0);
        // The posture law (night-hunter-2, shared with list-presets):
        // a LIVE player means the map slots are what is sounding;
        // a dead session means the slots are leftovers — the honest
        // posture is what the next `on` would resolve, and the
        // player line names the stale pid instead of parroting it
        // as if it were running.
        let alive = pid != 0 && crate::session::orphan::pid_alive(pid as i32);
        let (preset_name, volume_percent, player_line) = if alive {
            let index = control.get(&PRESET_ID_SLOT, 0).unwrap_or(0);
            let name = crate::profiles::registry_all()
                .get(index as usize)
                .map(|e| e.name)
                .unwrap_or("<unknown>");
            let volume = control.get(&VOLUME_PCT_SLOT, 0).unwrap_or(100);
            (name, volume, crate::output::ok_line(&format!("pid {pid}")))
        } else {
            let name = active_preset_name(None);
            // The next-start gain posture; on a malformed
            // ZYLAXION_VOLUME the env percent refuses — the map's
            // last value renders rather than a silent 100 (the
            // stale pid on the player line already flags the row
            // as last-session data).
            let volume = preset::context_volume_percent()
                .unwrap_or_else(|_| control.get(&VOLUME_PCT_SLOT, 0).unwrap_or(100));
            let line = if pid == 0 {
                crate::output::tip_line("not running")
            } else {
                crate::output::warn_line(&format!("stale pid {pid} — the next `on` re-resolves"))
            };
            (name, volume, line)
        };
        println!(
            "{}",
            crate::output::kv(
                "sound:",
                &if enabled {
                    crate::output::ok_line("on")
                } else {
                    crate::output::tip_line("off")
                }
            )
        );
        println!("{}", crate::output::kv("preset:", preset_name));
        println!(
            "{}",
            crate::output::kv(
                "volume:",
                &format!("{volume_percent}% (100 = the compiled default)")
            )
        );
        println!("{}", crate::output::kv("player:", &player_line));
    } else if pin::any_pin_present() {
        println!(
            "{}",
            crate::output::kv(
                "control:",
                &crate::output::warn_line("not openable by this user (pin ownership?)")
            )
        );
    }

    // 3. Audio device (the quiet-ALSA guard keeps libasound's C-level
    //    error log off the overview on cardless hosts).
    #[cfg(target_os = "linux")]
    let _quiet = QuietAlsa::new();
    let host = cpal::default_host();
    match host.default_output_device() {
        Some(device) => {
            // night-dinner-10: DeviceTrait::name() became description() in cpal
            // 0.18 (a DeviceDescription with a stable id); render the same
            // bare name the CLI contract always shown, same fallback.
            let name = device
                .description()
                .map(|d| d.name().to_owned())
                .unwrap_or_else(|_| "<unknown>".into());
            if let Ok(dev_config) = device.default_output_config() {
                println!(
                    "{}",
                    crate::output::kv(
                        "audio:",
                        &format!(
                            "{name} ({} Hz, {}ch, {:?})",
                            dev_config.sample_rate(),
                            dev_config.channels(),
                            dev_config.sample_format()
                        )
                    )
                );
            } else {
                println!(
                    "{}",
                    crate::output::kv("audio:", &format!("{name} (config unavailable)"))
                );
            }
        }
        None => {
            println!(
                "{}",
                crate::output::kv("audio:", &crate::output::warn_line("no device found"))
            );
        }
    }

    println!();
    // night-hunter-4: the runnable commands ride the status-green tier
    // (the help reference's example law), the trailing comments ride
    // the calm-grey grammar tier. Plain mode reproduces the exact
    // historical bytes. The comment column is computed from the
    // longest command so the rows stay aligned without hand-tuned
    // padding (night-dragon-maintenance-1, the tidy-geometry law).
    let rows = [
        ("zylaxion on", "# start the sound"),
        ("zylaxion preset --switch <name>", "# switch sound live"),
        ("zylaxion status", "# the full truth"),
    ];
    let width = rows.iter().map(|(cmd, _)| cmd.len()).max().unwrap_or(0);
    for (cmd, comment) in rows {
        println!(
            "  {}{} {}",
            crate::output::ok_line(cmd),
            " ".repeat(width - cmd.len()),
            crate::output::tip_line(comment)
        );
    }
    println!();
}
