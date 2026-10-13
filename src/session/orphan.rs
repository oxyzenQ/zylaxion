// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The orphaned-player lifecycle (night-improve-27): the class-of-bug
//! fix for the daemonless architecture's one lifecycle seam.
//!
//! The player could outlive the map it was watching: a `detach` unpinned
//! the control map, but the old player held the old inode and kept
//! running against a flag only it could read — and a fresh `attach`
//! then created a new control map at the same path, so two players ran
//! against two maps, both producing sound. The wire was made whole in
//! night-improve-26; this module makes the LIFECYCLE whole:
//!
//! - [`reap_player`] — the escalation driver: poll the pid through the
//!   0 → SIGTERM → SIGKILL ladder until it is gone. The detach side
//!   clears the enabled gate in the map the player watches FIRST (the
//!   player's own watcher exits within 100 ms); the signals are the
//!   belt under the suspenders.
//! - [`reap_orphans`] — the attach/`on`-side sweep: scan `/proc` for a
//!   process whose cmdline is EXACTLY this binary followed by the
//!   [`PLAYER_ARG`] argument, and reap it with the same escalation.
//!   The scan is exact-match: a different binary path, a different
//!   argument, or extra arguments never match, so unrelated processes
//!   are untouchable. This is the only place the tree ENUMERATES
//!   `/proc` — every other surface reads state from the pinned maps.
//!
//! The escalation is a pure decision table ([`ladder_next`]) driven by
//! a liveness probe (signal 0), so the test tree pins the real logic
//! the runtime walks.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, kill};

/// The hidden player verb — the exact second cmdline token the scan
/// matches. The spawn side (`crate::commands::sound::PLAYER_SPAWN_VERB`)
/// aliases this constant, so the wire and the sweep cannot drift.
pub const PLAYER_ARG: &str = "_play";

/// One liveness-probe step: 100 ms, the player watcher's own cadence.
pub const SETTLE_STEP: Duration = Duration::from_millis(100);

/// Polls per escalation stage: 5 × 100 ms = 500 ms per stage, five
/// watcher cycles for the gate to take effect before a signal flies.
pub const SETTLE_BUDGET_STEPS: u32 = 5;

/// The pure cmdline matcher: the raw `/proc/<pid>/cmdline` bytes
/// (NUL-separated) match exactly when they decode to precisely two
/// tokens — this binary's path, then the player verb. Near misses are
/// rejected by construction: a different binary path, a different
/// argument, extra arguments, a shell wrapper (`sh -c ...` carries
/// three-plus tokens), or an empty cmdline (a zombie) never match.
pub fn cmdline_matches_player(raw: &[u8], exe: &OsStr) -> bool {
    let exe_bytes = exe.as_bytes();
    let mut tokens = raw.split(|b| *b == 0).filter(|t| !t.is_empty());
    match (tokens.next(), tokens.next(), tokens.next()) {
        (Some(path), Some(arg), None) => path == exe_bytes && arg == PLAYER_ARG.as_bytes(),
        _ => false,
    }
}

/// Scan `/proc` for orphaned players: every live pid whose cmdline is
/// exactly `exe` + the player verb. `skip_pid` (the caller's own pid)
/// is never a candidate. This is the tree's only `/proc` enumeration.
pub fn scan_orphans(exe: &OsStr, skip_pid: i32) -> Vec<i32> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(Path::new("/proc")) else {
        return found;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(pid) = name.parse::<i32>() else {
            continue; // not a pid directory
        };
        if pid == skip_pid {
            continue;
        }
        let Ok(raw) = std::fs::read(Path::new("/proc").join(&name).join("cmdline")) else {
            continue; // raced exit, or a pid we cannot read (permission)
        };
        if cmdline_matches_player(&raw, exe) {
            found.push(pid);
        }
    }
    found
}

/// The escalation ladder as a pure decision table: given the stage
/// (0 = observing after the gate, 1 = SIGTERM sent, 2 = SIGKILL sent),
/// the probed liveness, and whether the stage's 500 ms budget elapsed,
/// the next step is a function — the runtime walk and the test tree
/// share this one table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderStep {
    /// Keep polling; the player may exit on its own (the cleared gate).
    Observe,
    /// The first deadline passed with the pid alive: send SIGTERM.
    SendTerm,
    /// SIGTERM ignored past its deadline: send SIGKILL.
    SendKill,
    /// The pid is gone (with or without the signals).
    ConcludeGone,
    /// SIGKILL ignored past its deadline: nothing more can be done.
    ConcludeSurvived,
}

/// The decision table (see [`LadderStep`]).
pub fn ladder_next(stage: u8, alive: bool, budget_elapsed: bool) -> LadderStep {
    if !alive {
        return LadderStep::ConcludeGone;
    }
    match stage {
        0 if budget_elapsed => LadderStep::SendTerm,
        0 => LadderStep::Observe,
        1 if budget_elapsed => LadderStep::SendKill,
        1 => LadderStep::Observe,
        _ if budget_elapsed => LadderStep::ConcludeSurvived,
        _ => LadderStep::Observe,
    }
}

/// The reaping outcome, logged and (for the attach/`on` sweep)
/// escalated to a classified refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReapOutcome {
    /// The pid exited — on the cleared gate, or on a signal.
    Gone { term_used: bool },
    /// The pid survived SIGKILL past its deadline (D-state, or a pid
    /// we cannot signal).
    Survived,
}

/// Drive the escalation ladder against one pid: poll liveness in
/// [`SETTLE_STEP`] steps for [`SETTLE_BUDGET_STEPS`] per stage, send
/// SIGTERM at the first deadline, SIGKILL at the second. Returns the
/// outcome and the human-readable step log (every step names the pid
/// and what happened — the caller logs at info).
pub fn reap_player(pid: i32) -> (ReapOutcome, Vec<String>) {
    let mut log = Vec::new();
    let mut stage: u8 = 0;
    let mut term_used = false;
    let mut steps_in_stage: u32 = 0;
    let mut stage_started = Instant::now();
    loop {
        let alive = pid_alive(pid);
        let budget_elapsed = steps_in_stage >= SETTLE_BUDGET_STEPS
            || stage_started.elapsed() >= SETTLE_STEP * SETTLE_BUDGET_STEPS;
        match ladder_next(stage, alive, budget_elapsed) {
            LadderStep::ConcludeGone => {
                log.push(format!(
                    "player pid {pid} exited{}",
                    if term_used {
                        " on SIGTERM"
                    } else {
                        " on the cleared gate"
                    }
                ));
                break (ReapOutcome::Gone { term_used }, log);
            }
            LadderStep::ConcludeSurvived => {
                log.push(format!(
                    "player pid {pid} SURVIVED SIGKILL past the deadline — kill it manually"
                ));
                break (ReapOutcome::Survived, log);
            }
            LadderStep::Observe => {
                steps_in_stage += 1;
                std::thread::sleep(SETTLE_STEP);
            }
            LadderStep::SendTerm => {
                term_used = true;
                let sent = signal_pid(pid, Signal::SIGTERM);
                log.push(format!("player pid {pid} sent SIGTERM ({sent})"));
                stage = 1;
                steps_in_stage = 0;
                stage_started = Instant::now();
            }
            LadderStep::SendKill => {
                let sent = signal_pid(pid, Signal::SIGKILL);
                log.push(format!("player pid {pid} sent SIGKILL ({sent})"));
                stage = 2;
                steps_in_stage = 0;
                stage_started = Instant::now();
            }
        }
    }
}

/// Send a signal, rendering the outcome for the log (the error text on
/// failure, "ok" on success).
fn signal_pid(pid: i32, signal: Signal) -> String {
    match kill(nix::unistd::Pid::from_raw(pid), signal) {
        Ok(()) => "ok".to_string(),
        Err(e) => e.to_string(),
    }
}

/// Liveness probe: signal 0 checks existence, but a ZOMBIE (a dead
/// child its parent has not waited yet) still answers the kill probe —
/// the state byte in /proc/<pid>/stat tells them apart. A zombie is a
/// dead player by any honest measure, so the probe reads it.
pub fn pid_alive(pid: i32) -> bool {
    if kill(nix::unistd::Pid::from_raw(pid), None).is_err() {
        return false;
    }
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        && let Some(idx) = stat.rfind(')')
        && let Some(state) = stat[idx + 1..].trim_start().chars().next()
    {
        return state != 'Z';
    }
    true
}

/// The attach/`on` sweep: scan for orphaned players of THIS binary and
/// reap each with the full escalation. Returns `Err` naming the pid(s)
/// that survived — the classified refusal the caller owes the user.
/// Every step logs at info with the pid and the outcome.
pub fn reap_orphans() -> Result<(), String> {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            // The sweep is best-effort at its source: without a binary
            // path there is nothing exact to match, and the spawn side
            // has its own current_exe gate.
            log::warn!("orphan scan skipped: cannot resolve this binary's path: {e}");
            return Ok(());
        }
    };
    let orphans = scan_orphans(exe.as_os_str(), nix::unistd::getpid().as_raw());
    if orphans.is_empty() {
        return Ok(());
    }
    log::info!(
        "orphan scan found {} orphaned player process(es): {orphans:?}",
        orphans.len()
    );
    let mut survivors = Vec::new();
    for pid in orphans {
        let (outcome, steps) = reap_player(pid);
        for step in steps {
            log::info!("{step}");
        }
        if outcome == ReapOutcome::Survived {
            survivors.push(pid);
        }
    }
    match survivors.is_empty() {
        true => Ok(()),
        false => Err(format!(
            "orphaned player(s) {survivors:?} could not be terminated; \
             kill them manually (kill <pid>), then retry"
        )),
    }
}

#[cfg(test)]
#[path = "../../test/session/orphan_tests.rs"]
mod tests;
