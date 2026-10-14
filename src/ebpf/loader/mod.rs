// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The tap loader — attach, init, pin; and detach (night-improve-23;
//! night-improve-25 restores the approved attach chain).
//!
//! This is the privileged one-shot that replaces daemon startup. It
//! loads the embedded tap object, walks the attach chain, initializes
//! the control plane, and pins EVERYTHING to bpffs — then the process
//! may exit, because the pinned link keeps the tap alive and the
//! pinned maps keep the state alive. After `attach` returns, nothing
//! of zylaxion is resident in userspace.
//!
//! ## The attach chain (locked, night-improve-25)
//!
//! Tried in order, the loader picks, no user-facing mode:
//!
//! 1. `fentry:input_event` — BPF_PROG_TYPE_TRACING on the exported
//!    `input_event()` function via the BTF trampoline. Zero overhead.
//!    Requires vmlinux BTF and the symbol in it (aya resolves the
//!    attach_btf_id at load time).
//! 2. `kprobe:input_event` — BPF_PROG_TYPE_KPROBE, pt_regs, no BTF
//!    needed. Requires the symbol off the kprobe blacklist (it is:
//!    plain-text EXPORT_SYMBOL on mainline).
//! 3. `kprobe:input_handle_event` — the alternate symbol, one step
//!    later in the input core (after the is_event_supported check);
//!    fewer events, still passive.
//! 4. Hard refusal — a classified
//!    [`AttachError`](crate::ebpf::attach_error::AttachError); nothing
//!    half-attached, no silent fallback.
//!
//! The chain step that won is recorded in the control map's TAP_KIND
//! slot (0/1/2) so `status` and the doctor can report it, and the
//! chosen step's label leads the attach summary.
//!
//! Ownership model (the boring privilege story): attach runs as root
//! (`sudo zylaxion attach`), and when invoked through sudo the pins are
//! chowned to the invoking user — so the unprivileged player and CLI can
//! open the pinned maps afterwards (bpffs file permissions govern pinned
//! map access, the same posture systemd-tmpfiles-style boot units use).
//!
//! Idempotence and self-heal: a complete pin set is already-attached
//! (a success shape — running attach twice is fine); a partial pin set
//! (a crashed attach, a manual `rm`) is cleaned and replanted — and a
//! failed attach plants ONLY the failure marker (a small pinned map
//! carrying the classified error code) so the doctor can name the
//! failure afterwards. The stale-state forensics the old daemon needed
//! (PID files, /proc/exe identity checks) reduce to "pins exist and
//! are complete", because the kernel owns the state now.
//!
//! Layout (the LOC law's dir/mod.rs shape, night-hunter-5): the wire
//! vocabulary (TapKind, the control slots, the ring record) here, the
//! attach orchestration in attach.rs, the chain steps and the map
//! planting in chain.rs.

use super::embedded;

/// The embedded pure-Rust tap object (night-improve-23): the aya-ebpf
/// ELF staged into OUT_DIR by build.rs — source-built on a git checkout,
/// registry-prebuilt on a crates.io extract; either way the same name,
/// either way it rides inside the binary.
static TAP_ELF: &[u8] =
    &embedded::AlignedElf::new(*include_bytes!(concat!(env!("OUT_DIR"), "/zylaxion-tap"))).bytes;

/// The program names in the tap object (the ELF symbol names — see
/// ebpf/src/main.rs: two attach programs over one body).
const FENTRY_PROGRAM: &str = "fentry_input_event";
const KPROBE_PROGRAM: &str = "kprobe_input_event";

/// The keyboard keycode domain the keymap covers (night-hunter-3):
/// keycodes 1..=255 — the keyboard keys. Mouse buttons and vendor keys
/// (BTN_* and friends, >= 0x100) stay unmapped and die in the kernel.
/// The attach loop populates the map from THIS constant and the test
/// tree pins its shape, so the written domain and the documented
/// domain cannot drift.
pub(crate) const KEYMAP_KEYCODE_DOMAIN: std::ops::RangeInclusive<u32> = 1..=255;

/// The chain step that owns the live tap — the vocabulary of the
/// control map's TAP_KIND slot and of every surface that reports
/// which lane is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapKind {
    /// Step 1: `fentry:input_event` (BPF_PROG_TYPE_TRACING, BTF).
    Fentry,
    /// Step 2: `kprobe:input_event` (BPF_PROG_TYPE_KPROBE).
    Kprobe,
    /// Step 3: `kprobe:input_handle_event` (the alternate symbol).
    KprobeAlt,
}

impl TapKind {
    /// The chain label every surface prints — the attach summary, the
    /// status tap line, the doctor.
    pub fn label(self) -> &'static str {
        match self {
            TapKind::Fentry => "fentry:input_event",
            TapKind::Kprobe => "kprobe:input_event",
            TapKind::KprobeAlt => "kprobe:input_handle_event",
        }
    }

    /// The control-map slot value (the wire format, pinned by tests).
    pub fn slot(self) -> u32 {
        match self {
            TapKind::Fentry => 0,
            TapKind::Kprobe => 1,
            TapKind::KprobeAlt => 2,
        }
    }

    /// The inverse of [`Self::slot`] — the read side `status` uses.
    pub fn from_slot(slot: u32) -> Option<Self> {
        Some(match slot {
            0 => TapKind::Fentry,
            1 => TapKind::Kprobe,
            2 => TapKind::KprobeAlt,
            _ => return None,
        })
    }
}

/// The control plane slot contract — the userspace mirror of
/// `control_slots` in ebpf/src/main.rs. Keep both sides in lockstep;
/// the ebpf crate's module doc carries the same table.
pub mod control {
    /// 1 while the sound is on. The kernel gate reads ONLY this slot.
    pub const ENABLED: u32 = 0;
    /// Registry index of the active preset (userspace-resolved).
    pub const PRESET_ID: u32 = 1;
    /// Master volume, percent of the compiled default gain.
    pub const VOLUME_PCT: u32 = 2;
    /// The player process pid (the "pidfile" lives in the kernel now).
    pub const PLAYER_PID: u32 = 3;
    /// Player lifecycle: 0 idle, 1 starting, 2 running, 3 failed.
    pub const STATE: u32 = 4;
    /// Which attach-chain step owns the live tap (night-improve-25):
    /// the [`super::TapKind`] slot value (0/1/2).
    pub const TAP_KIND: u32 = 5;
}

/// The ring buffer record — layout contract with `KeyRingEvent` in
/// ebpf/src/main.rs (24 bytes: repr(C) puts ts_us at 0 and the three
/// u32s at 8/12/16, four trailing pad bytes — identical shape on both
/// sides is the contract). The player parses ring items into this
/// shape; until the player lane lands it exists as the pinned contract
/// the tests pin.
#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRingEvent {
    /// CLOCK_MONOTONIC microseconds (stamped by the tap).
    pub ts_us: u64,
    /// The evdev keycode (e.g. KEY_A = 30).
    pub keycode: u32,
    /// 1 = press, 0 = release (autorepeat is filtered in-kernel).
    pub value: u32,
    /// The keymap sound class.
    pub slot: u32,
}

mod chain;
mod orchestrate;

pub use orchestrate::attach;

/// Detach the tap: unpin everything (idempotent, best-effort). Lives
/// in the teardown module (night-improve-26); this re-export keeps the
/// verb surface stable.
pub use super::teardown::detach;

/// The last attach failure recorded in the marker map, if one is
/// planted and readable — the doctor's read side of the marker lane.
/// Lives in the teardown module (night-improve-26); this re-export
/// keeps the doctor's path stable.
pub use super::teardown::last_attach_error;

#[cfg(test)]
#[path = "../../../test/ebpf/loader_tests.rs"]
mod tests;
