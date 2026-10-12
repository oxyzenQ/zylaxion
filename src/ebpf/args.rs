// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The input-event argument shim (night-improve-25) — the four
//! arguments of the kernel's exported `input_event()` function, the
//! one contract both attach lanes read.
//!
//! The tap runs as TWO programs over ONE body (the
//! `ebpf/src/main.rs` wrappers): a `fentry` program (BTF trampoline,
//! zero-overhead) and a `kprobe` program (pt_regs, works without BTF).
//! Both wrappers funnel into this module so the body logic is written
//! exactly once, and so the extraction is a pure, testable fact
//! instead of two unverifiable unsafe blocks.
//!
//! The kernel ABI (drivers/input/input.c, stable across the 5.15
//! floor and every kernel since):
//!
//! ```text
//! void input_event(struct input_dev *dev, unsigned int type,
//!                  unsigned int code, int value)
//! ```
//!
//! - `fentry` reads the args as register-width BTF slots
//!   (argument 0..=3); scalars occupy the low bits of each slot.
//! - `kprobe` reads the same args from `PT_REGS_PARM1..PARM4`
//!   (x86-64: rdi, rsi, rdx, rcx) — the same registers the BTF
//!   trampoline exposes, so the two lanes see the same bytes.
//!
//! Single-source discipline: this file is compiled by BOTH crates.
//! The userspace crate owns it (a normal module, so its tests run in
//! every gate lane); the ebpf crate includes the very same file via
//! `#[path]` (`mod args` in `ebpf/src/main.rs`) — the ebpf crate is
//! never packaged and only builds from a git checkout, so the
//! cross-boundary include is safe, and there is exactly one source of
//! truth for the extraction.

/// The four arguments of `input_event()` the tap reads.
///
/// `dev` is extracted (it is argument 0 of the ABI and part of the
/// contract) but never dereferenced or otherwise used: the tap is a
/// passive observer of the event values and must never touch the
/// device object.
///
/// Dead-code note (the KeyRingEvent precedent, src/ebpf/loader.rs):
/// the userspace crate compiles this module for its tests and as the
/// single source of truth the ebpf crate `#[path]`-includes; the
/// PRODUCTION constructor calls live on the ebpf side, so a plain
/// userspace build sees the items unused — the allows are that
/// cross-crate contract, not accidents.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEventArgs {
    /// The `struct input_dev *` (argument 0). Never dereferenced —
    /// passive-tap law.
    pub dev: u64,
    /// The event type (argument 1). `EV_KEY == 0x01` is the only one
    /// that sounds.
    pub kind: u32,
    /// The event code (argument 2) — the evdev keycode (e.g.
    /// KEY_A == 30).
    pub code: u32,
    /// The event value (argument 3): 1 press, 0 release, 2 autorepeat
    /// (autorepeat never sounds — the libinput parity rule).
    pub value: i32,
}

impl InputEventArgs {
    /// The kprobe lane: raw `PT_REGS_PARM1..PARM4` register values.
    ///
    /// Truncation is ABI-faithful, not a bug: the C prototype passes
    /// `unsigned int` / `int` arguments, which occupy the low 32 bits
    /// of their registers while the upper bits legally carry garbage.
    /// The kernel's own C code reads the 32-bit register halves; the
    /// shim does the same, so a garbage-carrying register and a clean
    /// one produce the identical event.
    #[allow(
        dead_code,
        reason = "production caller lives in the ebpf crate (see the struct's dead-code note)"
    )]
    pub fn from_pt_regs(parm1: u64, parm2: u64, parm3: u64, parm4: u64) -> Self {
        Self {
            dev: parm1,
            kind: parm2 as u32,
            code: parm3 as u32,
            value: parm4 as i32,
        }
    }

    /// The fentry lane: the BTF trampoline's typed argument slots.
    ///
    /// The trampoline exposes each argument as a register-width slot;
    /// scalars arrive already narrowed (or the wrapper narrows them
    /// with the same truncation [`Self::from_pt_regs`] applies — the
    /// two constructors must agree, and the tests pin that they do).
    #[allow(
        dead_code,
        reason = "production caller lives in the ebpf crate (see the struct's dead-code note)"
    )]
    pub fn from_btf_slots(dev: u64, kind: u32, code: u32, value: i32) -> Self {
        Self {
            dev,
            kind,
            code,
            value,
        }
    }
}

#[cfg(test)]
#[path = "../../test/ebpf/args_tests.rs"]
mod tests;
