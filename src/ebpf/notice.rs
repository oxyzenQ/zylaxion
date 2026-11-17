// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The control-notice schema (night-frontier-Z2, B1 — the evented
//! control plane).
//!
//! The writer (a CLI verb) publishes ONE notice per control-map
//! change; the player's watcher wakes on the pinned notice ring,
//! re-reads the control map (the ONLY authority), and applies. The
//! schema is single-sourced exactly like the argument shim: the
//! userspace crate owns the file (so its tests run in every gate
//! lane) and the eBPF crate compiles the very same bytes via
//! `#[path]` — see `src/ebpf/args.rs` for the discipline.
//!
//! The kernel-produces rule: a ringbuf's producer pages are
//! read-only to userspace on every kernel — including the 5.15
//! floor — so the writer cannot write the record itself. The writer
//! instead writes kind and value into the control map's two request
//! slots, then drives the pinned notice relay (a socket-filter
//! program, the one section shape aya natively loads and the one
//! program type whose TEST_RUN every floor kernel answers) with one
//! TEST_RUN; the relay runs IN THE KERNEL, stamps `bpf_ktime_get_ns`,
//! and publishes the 16-byte record into the ring. That is the only
//! 5.15-legal evented wake with a kernel timestamp, and it is why
//! the wake crosses the privilege boundary (no signal permission,
//! no pid targeting — the kernel produces the record) while leaving
//! an auditable trail a `bpftool map dump` can name. The transport
//! rides the control map's two request slots (6, 7) — the relay's
//! argument channel, never state — because the first CI leg's
//! verifier rejected the packet-reading shape with the reason beyond
//! the captured log tail, while the slot-reading shape uses only the
//! base helper set the tap itself already proves.

/// The 16-byte kernel record: ns@0, kind@8, value@12.
/// night-strict-warnings-1: this file is the ONE shared definition of
/// the wire layout, included verbatim by path from both sides — the
/// tap build never reads the three contract facts below (it only ever
/// publishes KIND_VOLUME); the loader and the watcher consume them on
/// the userspace side. The allow is the honest shape: the tap carries
/// the contract whole, the marker names which side answers.
#[allow(dead_code)] // userspace contract: the watcher's record layout
pub const NOTICE_RECORD_SIZE: usize = 16;

/// The control plane changed the enabled gate (value: 0 or 1).
#[allow(dead_code)] // userspace contract: consumed by the watcher
pub const KIND_ENABLED: u32 = 1;
/// The control plane changed the preset slot (value: registry index).
#[allow(dead_code)] // userspace contract: consumed by the watcher
pub const KIND_PRESET: u32 = 2;
/// The control plane changed the volume slot (value: percent).
pub const KIND_VOLUME: u32 = 3;

/// The notice record the relay publishes and the watcher consumes.
/// repr(C) puts `ns` at 0 (8-aligned) and the two u32s at 8/12 — the
/// identical shape on both sides is the contract. The zero-leakage
/// law is structural: the field set is (timestamp, kind, value) —
/// there is NO keycode field, so a control notice can never carry
/// key data (the events ring keeps its own record shape and its own
/// pin). The correlation key is the kernel timestamp itself: the
/// writer's publish instant and the record's ns land within one
/// syscall of each other.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ControlNotice {
    /// `bpf_ktime_get_ns` — CLOCK_MONOTONIC nanoseconds, the same
    /// epoch every process on the host reads, so the watcher computes
    /// the wake latency as `now - ns` with no clock sync.
    pub ns: u64,
    /// What changed (one of the KIND_* constants).
    pub kind: u32,
    /// The new value the writer wrote into the control map (a sound
    /// setting — percent, index, or gate).
    pub value: u32,
}

#[cfg(test)]
#[path = "../../test/ebpf/notice_tests.rs"]
mod tests;
