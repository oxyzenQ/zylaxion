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
/// were actually removed so `detach` can report honestly. The failure
/// marker is removed with the rest (link first — unlinking the link
/// detaches the tap; maps, program and marker last); the pin directory
/// goes last of all.
pub fn clear_pins() -> Vec<&'static str> {
    let mut removed = Vec::new();
    for p in ALL_PINS {
        // A pin file is bpffs — a plain unlink. Removing the LINK pin
        // detaches the tap; the rest free their kernel objects once the
        // last reference (player fds) drops.
        if std::fs::remove_file(p).is_ok() {
            removed.push(p);
        }
    }
    // The failure marker rides the same directory discipline.
    if std::fs::remove_file(PIN_MAP_ERROR).is_ok() {
        removed.push(PIN_MAP_ERROR);
    }
    let _ = std::fs::remove_dir(PIN_DIR);
    removed
}

#[cfg(test)]
#[path = "../../test/ebpf/pin_tests.rs"]
mod tests;
