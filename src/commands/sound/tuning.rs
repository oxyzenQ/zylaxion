// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The live tuning verbs: `preset --switch` and `volume` — one map
//! write each, effective while the player runs.

use std::process;

use crate::session::orphan::pid_alive;

use super::{
    PLAYER_PID_SLOT, PRESET_ID_SLOT, VOLUME_PCT_SLOT, init_logger, kernel_policy_gate,
    open_control, preset_index_or_error,
};

/// Handle `zylaxion preset --switch <name>` — one map write, live
/// swap (night-improve-24: the flag is `--switch`, renamed from the
/// silent `--name`).
pub fn cmd_preset(name: &str) {
    init_logger();
    kernel_policy_gate();
    let preset_index = match preset_index_or_error(name) {
        Ok(i) => i,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    let mut control = match open_control() {
        Ok(c) => c,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    if let Err(e) = control.set(PRESET_ID_SLOT, preset_index, 0) {
        crate::output::error(format!("cannot write the preset slot: {e}"));
        process::exit(1);
    }
    let playing = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0).ne(&0)
        && pid_alive(control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0) as i32);
    if playing {
        println!("preset set to {name} (swapped live)");
    } else {
        println!("preset set to {name} (takes effect on `zylaxion on`)");
    }
}

/// Handle `zylaxion volume <percent>` — one map write, live retune.
/// 100 percent is the compiled laptop-speaker default; the gate is the
/// session layer's [`crate::session::preset::volume_gate`]: 0..=200 by
/// default, `--force-this` lifts the ceiling to 10000, past that
/// nothing passes. The success print names the cap, no editorializing.
pub fn cmd_volume(percent: u32, force_this: bool) {
    init_logger();
    let percent = match crate::session::preset::volume_gate(percent, force_this) {
        Ok(p) => p,
        Err(e) => {
            crate::output::error(e);
            process::exit(2);
        }
    };
    kernel_policy_gate();
    let mut control = match open_control() {
        Ok(c) => c,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    if let Err(e) = control.set(VOLUME_PCT_SLOT, percent, 0) {
        crate::output::error(format!("cannot write the volume slot: {e}"));
        process::exit(1);
    }
    // night-hunter-1: the note names the lane that actually carried the
    // write — the old print said "cap 200%" even when --force-this had
    // just written 201..=10000, contradicting the value on the same line.
    let cap = crate::session::preset::MAX_VOLUME_PERCENT;
    let cap_note = if percent > cap {
        format!(
            "--force-this carried it past the {cap}% cap (force ceiling {})%)",
            crate::session::preset::MAX_FORCE_VOLUME_PERCENT
        )
    } else {
        format!("cap {cap}%")
    };
    println!("volume set to {percent}% (100 = the compiled default, {cap_note})");
}
