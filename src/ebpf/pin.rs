// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Pin path constants + presence/cleanup helpers (night-improve-23).
//!
//! Single source of truth for every bpffs path the tap's control plane
//! owns. The layout mirrors the zelynic pin discipline: one directory,
//! one program pin, one link pin, one pin per map — every path constant
//! here is the exact string `bpftool` prints and the loader opens.

use std::path::Path;

/// Root pin directory on bpffs. Everything zylaxion leaves in the
/// kernel lives under this one directory — `detach` removes it whole.
pub const PIN_DIR: &str = "/sys/fs/bpf/zylaxion";

/// Program pin (the tap program itself, for introspection and reload).
pub const PIN_PROG: &str = "/sys/fs/bpf/zylaxion/tap";

/// Link pin (the attachment — this is what keeps the tap alive after
/// the attach process exits; unlinking it detaches the tap).
pub const PIN_LINK: &str = "/sys/fs/bpf/zylaxion/tap_link";

/// Map pins — the control plane. The kernel reads `control` and
/// `keymap`; the player opens `events` and `stats`; every surface is
/// introspectable with stock bpftool.
pub const PIN_MAP_CONTROL: &str = "/sys/fs/bpf/zylaxion/control";
pub const PIN_MAP_KEYMAP: &str = "/sys/fs/bpf/zylaxion/keymap";
pub const PIN_MAP_EVENTS: &str = "/sys/fs/bpf/zylaxion/events";
pub const PIN_MAP_STATS: &str = "/sys/fs/bpf/zylaxion/stats";

/// The attach-failure marker pin (night-improve-25): a one-entry
/// array map holding the [`crate::ebpf::attach_error::AttachError`]
/// code of the LAST failed attach, planted ONLY on failure so the
/// doctor can name the cause after the process is gone. It is NOT
/// part of [`ALL_PINS`] — a complete attach never carries it (a
/// pin-set-completeness check that demanded the failure marker would
/// be a lie). The next attach's self-heal pass and `detach` both
/// remove it.
pub const PIN_MAP_ERROR: &str = "/sys/fs/bpf/zylaxion/attach_error";

/// Every pin the attach lane plants, in removal order (link first —
/// unlinking the link detaches the tap; maps and program last).
pub const ALL_PINS: [&str; 6] = [
    PIN_LINK,
    PIN_PROG,
    PIN_MAP_CONTROL,
    PIN_MAP_KEYMAP,
    PIN_MAP_EVENTS,
    PIN_MAP_STATS,
];

/// True when every pin of a complete attach exists. A partial pin set
/// (a crashed attach, a manual rm) reads as NOT attached — `attach`
/// cleans and replants, the same self-heal posture the old daemon's
/// stale-PID forensics had, minus the forensics.
pub fn all_pins_present() -> bool {
    ALL_PINS.iter().all(|p| Path::new(p).exists())
}

/// True when at least one pin exists — the "zylaxion left something in
/// the kernel" signal `attach` uses to offer cleanup and `detach` uses
/// to report idempotence. The failure marker counts: it IS something
/// left in the kernel.
pub fn any_pin_present() -> bool {
    ALL_PINS.iter().any(|p| Path::new(p).exists()) || error_marker_present()
}

/// True when the attach-failure marker is planted — the doctor's cue
/// to name the last attach failure (and the operator's hint that the
/// tap is NOT attached despite the directory being non-empty).
pub fn error_marker_present() -> bool {
    Path::new(PIN_MAP_ERROR).exists()
}

/// Unpin everything (best-effort, idempotent). Returns the names that
/// were actually removed so `detach` can report honestly, plus the
/// classified outcome of the pin-directory removal. The failure marker
/// is removed with the rest (a detach leaves nothing — its own contract
/// says so), the pin directory goes last of all.
pub fn clear_pins() -> (Vec<String>, PinDirRemoval) {
    let mut entries: Vec<&str> = ALL_PINS.to_vec();
    entries.push(PIN_MAP_ERROR);
    remove_entries_and_dir(&entries, Path::new(PIN_DIR))
}

/// The attach-side cleanup lane (night-improve-26): unpin the real
/// pins WITHOUT sweeping the attach-failure marker. The marker is the
/// evidence the doctor reads back — the pre-attach self-heal and the
/// failure protocol must not blanket-remove it; the caller supersedes
/// it deliberately (read it, log it, unlink it right before the
/// re-plant). Returns the removed paths and the classified directory
/// outcome.
pub fn clear_pins_keeping_marker() -> (Vec<String>, PinDirRemoval) {
    remove_entries_and_dir(ALL_PINS.as_ref(), Path::new(PIN_DIR))
}

/// The night-improve-26 cleanup core, shaped for both the real bpffs
/// lanes and the test fixtures: remove every file in `entries` (in the
/// given order — the link pin must stay first so an interrupted
/// cleanup leaves a DETACHED tap), then classify the rmdir of `dir`.
/// Returns the removed entry paths (in the same order) and the
/// classified directory outcome. The real lanes pass the pinned
/// constants; the fixture tests pass temp-directory contents — the
/// classification logic is identical, while the bpffs unlink itself
/// rides the integration lane (the tests' header note still holds:
/// the real-path lanes NEVER run inside a test).
pub fn remove_entries_and_dir(entries: &[&str], dir: &Path) -> (Vec<String>, PinDirRemoval) {
    let mut removed = Vec::new();
    for entry in entries {
        // A pin file is bpffs — a plain unlink. Removing the LINK pin
        // detaches the tap; the rest free their kernel objects once the
        // last reference (player fds) drops.
        if std::fs::remove_file(entry).is_ok() {
            removed.push((*entry).to_string());
        }
    }
    let outcome = remove_dir_classified(dir);
    (removed, outcome)
}

/// The classified outcome of the pin-directory removal (night-improve-26,
/// the D3 cleanup): the rmdir is no longer a silent best-effort — every
/// outcome is named, and only the two benign ones pass quietly.
#[derive(Debug)]
pub enum PinDirRemoval {
    /// The directory was removed — the cleanup is complete.
    Removed,
    /// The directory was not there — already gone, equally fine.
    AlreadyAbsent,
    /// The directory holds entries zylaxion did not create. Left in
    /// place: foreign entries are not ours to remove. Handled, not
    /// fatal — the caller reports it honestly.
    LeftForeignEntries,
    /// Something else went wrong — the caller surfaces it.
    Unexpected(std::io::Error),
}

/// Classified rmdir over an arbitrary directory (the fixture-testable
/// core; the real-path wrapper is [`remove_pin_dir`]). ENOTEMPTY maps
/// to [`PinDirRemoval::LeftForeignEntries`] (handled-not-fatal), ENOENT
/// to [`PinDirRemoval::AlreadyAbsent`]; every other error rides
/// [`PinDirRemoval::Unexpected`] for the caller to surface.
pub fn remove_dir_classified(dir: &Path) -> PinDirRemoval {
    match std::fs::remove_dir(dir) {
        Ok(()) => PinDirRemoval::Removed,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => PinDirRemoval::AlreadyAbsent,
        Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => {
            PinDirRemoval::LeftForeignEntries
        }
        Err(e) => PinDirRemoval::Unexpected(e),
    }
}

/// The classified rmdir of the real pin directory (night-improve-26):
/// detach and the attach pre-flight both end their cleanup here, so a
/// half-state directory never survives into the next attach as a
/// surprise EEXIST.
pub fn remove_pin_dir() -> PinDirRemoval {
    remove_dir_classified(Path::new(PIN_DIR))
}

#[cfg(test)]
#[path = "../../test/ebpf/pin_tests.rs"]
mod tests;
