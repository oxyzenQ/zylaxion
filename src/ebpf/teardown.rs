// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The loader's teardown side (night-improve-26): unpinning everything
//! and reading the failure evidence back. Split from loader.rs by the
//! 600-LOC no-mercy cap — one theme per file, the zelynic discipline.
//!
//! Two surfaces:
//!
//! - [`detach`] — unpin everything (idempotent, best-effort). Before a
//!   single pin is unlinked, a running player is reaped first
//!   (night-improve-27): the enabled gate clears in the map the player
//!   is watching, and the 0 → SIGTERM → SIGKILL escalation drives until
//!   the pid is gone — the player must never outlive the map it reads.
//!   After a successful detach NOTHING of zylaxion exists in the
//!   kernel: every pin unlinked, the failure marker gone, and the pin
//!   directory itself rmdir'd through the classified night-improve-26
//!   cleanup (foreign entries are left and named, unexpected errors
//!   surface).
//! - [`last_attach_error`] — the doctor's read side of the marker lane:
//!   the classified verdict a failed attach planted on bpffs.

use std::path::Path;

use aya::maps::MapData;

use super::attach_error::AttachError;
use super::pin;

// The control-slot constants — the userspace mirror of control_slots
// in ebpf/src/main.rs (the same private-alias shape player.rs carries;
// the loader's test tree pins the cross-side contract).
const ENABLED_SLOT: u32 = 0;
const PLAYER_PID_SLOT: u32 = 3;

/// Reap a running player before the pins go away (night-improve-27):
/// read the registered pid from the control map, clear the enabled
/// gate in THAT map (the player's watcher exits on it within its
/// 100 ms cadence), then drive the orphan escalation ladder until the
/// pid is gone — but only through the
/// [`pid_is_our_player`](crate::session::orphan::pid_is_our_player)
/// identity gate first (night-total-lts-1): the map is writable by
/// the pin-owning user, so the pid it carries is untrusted input to a
/// privileged kill, and a planted pid must never turn `sudo zylaxion
/// detach` into a root-mediated SIGKILL aimed by map data. A refused
/// candidate logs the refusal and loses only the signals — the
/// cleared gate above stays the real player's stop signal. Every step
/// logs at info with the pid and the outcome; the summary line names
/// what happened. Best-effort by contract: an unreadable control map
/// (foreign pin, raced unpin) logs a warning and the unpin proceeds —
/// detach's job is unpinning, and a refuse-to-detach shape would
/// strand the pins forever.
fn reap_player_from_map() -> Vec<String> {
    let mut notes = Vec::new();
    let data = match MapData::from_pin(pin::PIN_MAP_CONTROL) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("player reaping skipped: the control map is not openable: {e}");
            return notes;
        }
    };
    let owned = aya::maps::Map::Array(data);
    let Ok(mut control) = aya::maps::Array::<MapData, u32>::try_from(owned) else {
        log::warn!("player reaping skipped: the control map is not an array");
        return notes;
    };
    let pid = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0);
    if pid == 0 {
        return notes; // no player registered
    }
    if !crate::session::orphan::pid_alive(pid as i32) {
        notes.push(format!(
            "the registered player pid {pid} is already gone (stale registration)"
        ));
        return notes;
    }
    // The gate first: the player's own watcher (100 ms) exits cleanly —
    // the fade-out path, not the signal path, whenever it can be.
    if let Err(e) = control.set(ENABLED_SLOT, 0, 0) {
        log::warn!("cannot clear the enabled gate for the player: {e}");
    }
    // The identity gate (night-total-lts-1): the signals below fly
    // with this process's privileges (root, for the sudo detach), so
    // the candidate must prove the exact player shape before any of
    // them fly. The cmdline read is the same /proc surface the
    // attach-side sweep reads; a refused candidate keeps the cleared
    // gate as its stop signal and the detach proceeds to the unpins.
    let exe = std::env::current_exe().ok();
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok();
    if !crate::session::orphan::pid_is_our_player(
        cmdline.as_deref(),
        exe.as_deref().map(std::path::Path::as_os_str),
    ) {
        let note = format!(
            "the registered pid {pid} does not carry the player cmdline — signals refused \
             (identity gate, night-total-lts-1); the cleared gate stays its stop signal"
        );
        log::warn!("{note}");
        notes.push(note);
        return notes;
    }
    let (outcome, steps) = crate::session::orphan::reap_player(pid as i32);
    for step in &steps {
        log::info!("{step}");
    }
    match outcome {
        crate::session::orphan::ReapOutcome::Gone { term_used } => notes.push(format!(
            "the running player (pid {pid}) was stopped{}",
            if term_used {
                " (SIGTERM)"
            } else {
                " (the cleared gate)"
            }
        )),
        crate::session::orphan::ReapOutcome::Survived => notes.push(format!(
            "the running player (pid {pid}) SURVIVED the SIGKILL — kill it manually"
        )),
    }
    notes
}

/// Detach the tap: unpin everything (idempotent, best-effort). Returns
/// a human-readable summary. After a successful detach NOTHING of
/// zylaxion exists in the kernel.
pub fn detach() -> Result<String, AttachError> {
    if !pin::any_pin_present() {
        return Ok("tap is not attached — nothing to detach".to_string());
    }
    // The player never outlives its map (night-improve-27): the gate
    // clears and the escalation drives BEFORE a single pin is unlinked —
    // the old player must not keep reading an inode the pins just lost.
    let reap_notes = reap_player_from_map();
    let (removed, dir_outcome) = pin::clear_pins();
    let missing: Vec<_> = pin::ALL_PINS
        .iter()
        .chain(std::iter::once(&pin::PIN_MAP_ERROR))
        .filter(|p| Path::new(p).exists())
        .collect();
    if !missing.is_empty() {
        return Err(AttachError::Unknown {
            detail: format!(
                "tap detach incomplete: {removed:?} removed, but {} could not be removed \
                 (owned by another user? remove with sudo)",
                missing
                    .iter()
                    .map(|p| p.as_ref())
                    .collect::<Vec<&str>>()
                    .join(", ")
            ),
        });
    }
    // The classified rmdir (night-improve-26): the two benign outcomes
    // read as a clean detach; foreign entries are handled-not-fatal and
    // reported honestly; anything unexpected is surfaced, not swallowed.
    match dir_outcome {
        pin::PinDirRemoval::Removed | pin::PinDirRemoval::AlreadyAbsent => Ok(format!(
            "tap detached: {} pin(s) removed{} — nothing of zylaxion remains in the kernel",
            removed.len(),
            if reap_notes.is_empty() {
                String::new()
            } else {
                format!("; {}", reap_notes.join(", "))
            }
        )),
        pin::PinDirRemoval::LeftForeignEntries => Ok(format!(
            "tap detached: {} pin(s) removed{} — the pin directory holds entries zylaxion \
             did not create and was left in place (foreign entries are not ours to remove)",
            removed.len(),
            if reap_notes.is_empty() {
                String::new()
            } else {
                format!("; {}", reap_notes.join(", "))
            }
        )),
        pin::PinDirRemoval::Unexpected(e) => Err(AttachError::Unknown {
            detail: format!(
                "tap detached: {} pin(s) removed, but the pin directory could not be \
                 removed: {e}",
                removed.len()
            ),
        }),
    }
}

/// The last attach failure recorded in the marker map, if one is
/// planted and readable — the doctor's read side of the marker lane.
pub fn last_attach_error() -> Option<AttachError> {
    use aya::maps::MapData;
    let data = MapData::from_pin(pin::PIN_MAP_ERROR).ok()?;
    let owned = aya::maps::Map::Array(data);
    let marker: aya::maps::Array<MapData, u32> = aya::maps::Array::try_from(owned).ok()?;
    let code = marker.get(&0u32, 0).ok()?;
    AttachError::from_code(code)
}
