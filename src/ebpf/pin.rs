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
/// to report idempotence.
pub fn any_pin_present() -> bool {
    ALL_PINS.iter().any(|p| Path::new(p).exists())
}

/// Unpin everything (best-effort, idempotent). Returns the names that
/// were actually removed so `detach` can report honestly. The pin
/// directory is removed last.
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
    let _ = std::fs::remove_dir(PIN_DIR);
    removed
}

#[cfg(test)]
#[path = "../../test/ebpf/pin_tests.rs"]
mod tests;
