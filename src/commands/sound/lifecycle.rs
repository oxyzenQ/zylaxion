// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The player lifecycle verbs: `on` spawns the stateless player and
//! enables the gate, `off` clears the gate and lets the player exit.

use std::process;
use std::time::Instant;

use crate::ebpf::pin;
use crate::session::orphan::pid_alive;

use super::{
    ControlPinStat, ENABLED_SLOT, PLAYER_PID_SLOT, PLAYER_SETTLE_BUDGET, PLAYER_SPAWN_VERB,
    PRESET_ID_SLOT, SETTLE_STEP, STATE_IDLE, STATE_RUNNING, STATE_SLOT, VOLUME_PCT_SLOT,
    context_volume_percent, control_pin_stat, init_logger, kernel_policy_gate, on_root_refusal,
    open_control, preset_index_or_error,
};

/// Handle `zylaxion on [preset]` — enable the sound.
pub fn cmd_on(cli_preset: Option<String>) {
    // The privilege verdict (night-improve-26): the parent owns the
    // door — a root `on` refuses before it resolves paths, opens maps,
    // or spawns anything. Same wording the player-side guard prints;
    // a root player would open audio in root's session, which no user
    // desktop shares.
    if let Some(refusal) = on_root_refusal(crate::session::privilege::euid_is_root()) {
        crate::output::error(refusal);
        process::exit(1);
    }

    init_logger();

    // The kernel gate: one probe, the same verdicts the doctor prints.
    let kernel = crate::session::kernel::probe();
    if !kernel.supported() {
        crate::output::error(format!(
            "kernel eBPF verdict: {} — zylaxion is eBPF-only, upgrade the kernel \
             (see `zylaxion doctor` for the full verdict)",
            kernel.verdict()
        ));
        process::exit(1);
    }
    // The kernel-policy gate (night-badas-4 round 5): the classified
    // refusal beats the misleading not-attached shape an EPERM takes.
    kernel_policy_gate();

    let mut control = match open_control() {
        Ok(c) => c,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };

    // Resolve the session preset: CLI flag > ZYLAXION_PRESET > default.
    // The resolver's gain is the ZYLAXION_VOLUME linear factor; the
    // percent mirror below is what the control map stores.
    let (_profile_unused_here, preset_name, _gain) =
        match crate::session::preset::resolve(cli_preset.as_deref()) {
            Ok(r) => r,
            Err(e) => {
                crate::output::error(e);
                process::exit(1);
            }
        };
    let preset_index = match preset_index_or_error(preset_name) {
        Ok(i) => i,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    let volume_percent = match context_volume_percent() {
        Ok(v) => v,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    // Write the control plane BEFORE spawning: the player reads its
    // initial posture from the map at startup.
    if let Err(e) = control.set(PRESET_ID_SLOT, preset_index, 0) {
        crate::output::error(format!("cannot write the preset slot: {e}"));
        process::exit(1);
    }
    if let Err(e) = control.set(VOLUME_PCT_SLOT, volume_percent, 0) {
        crate::output::error(format!("cannot write the volume slot: {e}"));
        process::exit(1);
    }

    // Already playing? A live player needs no spawn — the map writes
    // above already applied (and enabled comes below).
    let existing_pid = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0);
    let already_running = existing_pid != 0
        && pid_alive(existing_pid as i32)
        && control.get(&STATE_SLOT, 0).unwrap_or(0) == STATE_RUNNING;
    if already_running {
        let _ = control.set(ENABLED_SLOT, 1, 0);
        // night-improve-27 wording: the map writes above already applied,
        // so this surface is an UPDATE, not a spawn — say so, and keep
        // the contrast with the fresh-spawn line below ("new player").
        println!(
            "sound already on: preset {preset_name}, volume {volume_percent}% \
             (player pid {existing_pid} kept — preset updated live)"
        );
        return;
    }

    // The orphan sweep (night-improve-27): belt under attach's
    // suspenders — the spawn must be single from ANY state, so a
    // player that outlived its map dies here before a new one is
    // born. Exact-match scan; a survivor refuses with the pid.
    if let Err(e) = crate::session::orphan::reap_orphans() {
        crate::output::error(e);
        process::exit(1);
    }

    // Clear any stale registration, then spawn the stateless player:
    // same binary, hidden verb, detached into its own process group,
    // no stdio at all (night-hunter-6: the detached player owns no
    // terminal bytes — its log lane dies here, so a preset switch an
    // hour later can never dirty the shell that ran `on`). Startup
    // failure diagnosis stays honest: the parent's error messages
    // name `zylaxion _play` in the foreground, where the player's
    // log lane is visible again.
    let _ = control.set(STATE_SLOT, STATE_IDLE, 0);
    let _ = control.set(PLAYER_PID_SLOT, 0, 0);
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            crate::output::error(format!("cannot resolve this binary's path: {e}"));
            process::exit(1);
        }
    };
    use std::os::unix::process::CommandExt as _;
    let spawn = std::process::Command::new(exe)
        .arg(PLAYER_SPAWN_VERB)
        .stdin(process::Stdio::null())
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::null())
        .process_group(0)
        .spawn();
    let mut child = match spawn {
        Ok(c) => c,
        Err(e) => {
            crate::output::error(format!("failed to spawn the player: {e}"));
            process::exit(1);
        }
    };

    // Enable the gate BEFORE waiting: the tap only rings when enabled=1,
    // and the player's readiness race loses nothing either way.
    if let Err(e) = control.set(ENABLED_SLOT, 1, 0) {
        crate::output::error(format!("cannot enable the sound: {e}"));
        let _ = child.kill();
        process::exit(1);
    }

    // Wait for the player to report running (or die trying).
    let started = Instant::now();
    loop {
        if child.try_wait().map(|s| s.is_some()).unwrap_or(true) {
            crate::output::error(
                "the player exited during startup\nrun `zylaxion doctor` for the \
                 environment verdicts, or run `zylaxion _play` in a terminal to \
                 see the player's own output",
            );
            let _ = control.set(ENABLED_SLOT, 0, 0);
            process::exit(1);
        }
        if control.get(&STATE_SLOT, 0).unwrap_or(0) == STATE_RUNNING {
            break;
        }
        if started.elapsed() >= PLAYER_SETTLE_BUDGET {
            crate::output::error(
                "the player did not reach the running state in time\nrun \
                 `zylaxion doctor` for the environment verdicts, or run \
                 `zylaxion _play` in a terminal to see the player's own output",
            );
            let _ = child.kill();
            let _ = control.set(ENABLED_SLOT, 0, 0);
            process::exit(1);
        }
        std::thread::sleep(SETTLE_STEP);
    }
    let child_pid = child.id();
    drop(child);

    println!(
        "sound on: preset {preset_name}, volume {volume_percent}% (new player pid {child_pid}, \
         detached; `zylaxion off` to stop)"
    );
}

/// Handle `zylaxion off` — silence. The player self-exits within its
/// 100 ms watcher cadence; the tap and the pins STAY attached (the next
/// `on` is instant and needs no sudo). `zylaxion detach` removes the
/// kernel side entirely.
pub fn cmd_off() {
    init_logger();

    // The kernel-policy gate, only when a tap actually lives in the
    // kernel (night-badas-4 round 5): the pin check is filesystem-
    // level, so the idempotent shape below stays posture-independent.
    if pin::any_pin_present() {
        kernel_policy_gate();
    }

    let mut control = match open_control() {
        Ok(c) => c,
        Err(e) => match control_pin_stat() {
            // The classified open (night-dinner-3): a genuine "not
            // attached" is the idempotent success shape it always
            // was — but a WALK-BLOCKED bpffs mount is a different
            // world: the enabled gate is untouched and a running
            // player keeps playing, so "nothing to stop" here would
            // be a lie with a soundtrack.
            ControlPinStat::WalkBlocked => {
                crate::output::error(
                    "the control plane is unreachable (a mode-0700 bpffs mount — the Debian \
                     13 default): the enabled gate is UNTOUCHED, and if a player is running \
                     it keeps playing\n  \
                     tip: sudo chmod o+x /sys/fs/bpf, then re-run zylaxion off",
                );
                process::exit(1);
            }
            // The map exists but refused the open: not "nothing to
            // stop" — surface the classified open error verbatim.
            ControlPinStat::Reachable => {
                crate::output::error(format!("cannot reach the control plane: {e}"));
                process::exit(1);
            }
            ControlPinStat::Absent => {
                // Not attached: off is idempotent — nothing to stop is
                // a success shape, not an error.
                println!("sound is off (the tap is not attached — nothing to stop)");
                return;
            }
        },
    };

    let pid = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0);
    if pid == 0 || !pid_alive(pid as i32) {
        // No live player: clear the gate and any stale registration.
        let _ = control.set(ENABLED_SLOT, 0, 0);
        let _ = control.set(PLAYER_PID_SLOT, 0, 0);
        let _ = control.set(STATE_SLOT, STATE_IDLE, 0);
        println!("sound is off (no player was running)");
        return;
    }

    // The stop protocol: clear the gate, the player's watcher sees it
    // within 100 ms, fades out, and unregisters itself.
    if let Err(e) = control.set(ENABLED_SLOT, 0, 0) {
        crate::output::error(format!("cannot clear the enabled flag: {e}"));
        process::exit(1);
    }
    let started = Instant::now();
    while pid_alive(pid as i32) {
        if started.elapsed() >= PLAYER_SETTLE_BUDGET {
            // The player ignored the gate (wedged audio thread?): the
            // honest report, with the manual escape hatch.
            crate::output::warning(format!(
                "the player (pid {pid}) is still alive after the stop budget — \
                 kill -TERM {pid} if it stays wedged"
            ));
            println!("sound off (stop pending)");
            return;
        }
        std::thread::sleep(SETTLE_STEP);
    }
    println!("sound off (no processes remain; the tap stays attached)");
}
