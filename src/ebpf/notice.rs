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
//! instead drives the pinned notice relay (a socket-filter program,
//! the one section shape aya natively loads and the one program type
//! whose TEST_RUN every floor kernel answers) with a 16-byte frame;
//! the relay runs IN THE KERNEL, stamps `bpf_ktime_get_ns`, and
//! publishes the 24-byte record into the ring. That is the only
//! 5.15-legal evented wake with a kernel timestamp, and it is why
//! the wake crosses the privilege boundary (no signal permission,
//! no pid targeting — the kernel produces the record) while leaving
//! an auditable trail a `bpftool map dump` can name.

/// The frame magic: "ZLAX" little-endian. A frame without it dies in
/// the relay — garbage never reaches the ring.
pub const NOTICE_MAGIC: u32 = 0x5A4C_4158;

/// The 16-byte writer frame: magic@0, kind@4, value@8, seq@12.
pub const NOTICE_FRAME_SIZE: usize = 16;

/// The 24-byte kernel record: ns@0, kind@8, value@12, seq@16 (+pad).
pub const NOTICE_RECORD_SIZE: usize = 24;

/// The control plane changed the enabled gate (value: 0 or 1).
pub const KIND_ENABLED: u32 = 1;
/// The control plane changed the preset slot (value: registry index).
pub const KIND_PRESET: u32 = 2;
/// The control plane changed the volume slot (value: percent).
pub const KIND_VOLUME: u32 = 3;

/// The notice record the relay publishes and the watcher consumes.
/// repr(C) puts `ns` at 0 (8-aligned) and the three u32s at 8/12/16
/// with an explicit pad — the identical shape on both sides is the
/// contract. The zero-leakage law is structural: the field set is
/// (timestamp, kind, value, seq) — there is NO keycode field, so a
/// control notice can never carry key data (the events ring keeps
/// its own record shape and its own pin).
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
    /// The writer's per-boot publish counter (correlates the CLI
    /// invocation with the wake row an audit reads back).
    pub seq: u32,
    /// Explicit trailing pad — the layout contract stays exact.
    pub _pad: u32,
}

/// Encode the 16-byte writer frame (little-endian; the eBPF side
/// decodes with the same byte order contract).
pub fn encode_frame(kind: u32, value: u32, seq: u32) -> [u8; NOTICE_FRAME_SIZE] {
    let mut frame = [0u8; NOTICE_FRAME_SIZE];
    frame[0..4].copy_from_slice(&NOTICE_MAGIC.to_le_bytes());
    frame[4..8].copy_from_slice(&kind.to_le_bytes());
    frame[8..12].copy_from_slice(&value.to_le_bytes());
    frame[12..16].copy_from_slice(&seq.to_le_bytes());
    frame
}

/// Decode a writer frame, or `None` for anything the magic rejects.
/// The relay drops `None` silently — a malformed or foreign frame
/// never becomes a notice. The userspace binary compiles this half
/// for the schema contract only (the tests exercise it); the eBPF
/// crate calls it for real.
#[cfg_attr(not(test), allow(dead_code))]
pub fn decode_frame(frame: &[u8]) -> Option<(u32, u32, u32)> {
    if frame.len() < NOTICE_FRAME_SIZE {
        return None;
    }
    let word = |off: usize| -> u32 {
        u32::from_le_bytes(
            frame[off..off + 4]
                .try_into()
                .expect("the length check above guarantees 4 bytes"),
        )
    };
    if word(0) != NOTICE_MAGIC {
        return None;
    }
    Some((word(4), word(8), word(12)))
}

#[cfg(test)]
#[path = "../../test/ebpf/notice_tests.rs"]
mod tests;
