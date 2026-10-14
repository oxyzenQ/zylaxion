// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The `status` verb: the one-screen truth, plus the `--verbose` deep
//! read that appends the kernel-side typing telemetry.

use crate::ebpf::pin;
use crate::session::orphan::pid_alive;

use super::open_control;
use super::{
    ENABLED_SLOT, PLAYER_PID_SLOT, PRESET_ID_SLOT, STATE_IDLE, STATE_RUNNING, STATE_SLOT,
    TAP_KIND_SLOT, VOLUME_PCT_SLOT,
};

/// Handle `zylaxion status [--verbose]` — the one-screen truth:
/// kernel verdict, tap residency, player liveness, and the active
/// posture. The verbose flag appends the kernel-side typing
/// telemetry (the merged `stats` surface, night-improve-24) — a
/// static print-once deep read, never a live refresh: the deleted
/// `--live` loop was the wrong shape for a status surface.
pub fn cmd_status(verbose: bool) {
    let kernel = crate::session::kernel::probe();
    // night-hunter-4: the verdict rides the semantic tier — green for a
    // supported kernel, red for a verified-unsupported one. Plain bytes
    // whenever stdout is not a terminal (the pipe contract).
    let verdict = if kernel.supported() {
        crate::output::ok_line(kernel.verdict())
    } else {
        crate::output::fail_line(kernel.verdict())
    };
    println!("kernel:   {} ({})", kernel.release, verdict);

    if !pin::any_pin_present() {
        println!(
            "tap:      {}",
            crate::output::tip_line("not attached (sudo zylaxion attach to plant it)")
        );
        return;
    }
    if !pin::all_pins_present() {
        crate::output::warning(
            "the pin set is partial (a crashed attach?) — sudo zylaxion attach replants it",
        );
    }
    // The kernel-policy note (night-badas-4 round 5): a locked
    // unprivileged-bpf kernel refuses the map reads too — name why.
    if !crate::session::privilege::euid_is_root() && !kernel.unpriv_bpf_allowed() {
        crate::output::warning(kernel.unpriv_bpf_verdict());
    }
    // Open the control map once: the tap line below reads the chain
    // step from it, the posture block reads the rest.
    let control = open_control();

    // Which chain step owns the live tap (night-improve-25): the
    // TAP_KIND control slot records fentry:input_event /
    // kprobe:input_event / kprobe:input_handle_event at attach. A slot
    // this build cannot parse (a newer binary attached this tap?)
    // renders honestly as unknown instead of guessing. night-hunter-4:
    // the known step rides green (an affirmative residency verdict),
    // the unknown one rides yellow (a finding, not a failure).
    let tap_line = match control
        .as_ref()
        .ok()
        .and_then(|c| c.get(&TAP_KIND_SLOT, 0).ok())
        .and_then(crate::ebpf::loader::TapKind::from_slot)
    {
        Some(kind) => crate::output::ok_line(kind.label()),
        None => {
            crate::output::warn_line("an unknown chain step (a newer binary attached this tap?)")
        }
    };
    println!(
        "tap:      attached via {tap_line} ({} pins under {})",
        pin::ALL_PINS.len(),
        pin::PIN_DIR
    );

    let Ok(control) = control else {
        crate::output::warning("the pinned control map is not openable by this user");
        return;
    };
    let enabled = control.get(&ENABLED_SLOT, 0).unwrap_or(0) == 1;
    let preset_index = control.get(&PRESET_ID_SLOT, 0).unwrap_or(0);
    let preset_name = crate::profiles::registry_all()
        .get(preset_index as usize)
        .map(|e| e.name)
        .unwrap_or("<unknown>");
    let volume = control.get(&VOLUME_PCT_SLOT, 0).unwrap_or(100);
    let pid = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0);
    let state = control.get(&STATE_SLOT, 0).unwrap_or(0);

    println!(
        "sound:    {}",
        if enabled {
            crate::output::ok_line("on")
        } else {
            crate::output::tip_line("off")
        }
    );
    println!("preset:   {preset_name}");
    println!("volume:   {volume}% (100 = the compiled default)");
    match (pid, pid_alive(pid as i32)) {
        (0, _) => println!("player:   {}", crate::output::tip_line("not running")),
        (p, true) => {
            // night-hunter-4: the state word rides the tier — green
            // running, grey the passive waiting states, red failed,
            // yellow unknown.
            let state_word = match state {
                STATE_RUNNING => crate::output::ok_line("running"),
                STATE_IDLE => crate::output::tip_line("idle"),
                1 => crate::output::tip_line("starting"),
                3 => crate::output::fail_line("failed"),
                _ => crate::output::warn_line("unknown"),
            };
            println!(
                "player:   {} (pid {p}, state: {state_word})",
                crate::output::ok_line("running")
            );
        }
        (p, false) => println!(
            "player:   {}",
            crate::output::warn_line(&format!("stale pid {p} in the map (next `on` takes over)"))
        ),
    }
    if verbose {
        crate::commands::stats::print_telemetry();
    }
    println!();
    println!(
        "{}",
        crate::output::tip_line(&format!(
            "introspection: bpftool map dump pinned {}",
            pin::PIN_MAP_CONTROL
        ))
    );
}
