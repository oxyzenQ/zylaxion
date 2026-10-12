// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// args_tests.rs — the argument-extraction shim pins (night-improve-25).
// The shim is the one contract both attach lanes read; a drift between
// the two constructors would make the fentry and kprobe taps disagree
// about the same keystroke. These tests run in the userspace gate
// lanes because the ebpf target has no test harness — the single
// source file (src/ebpf/args.rs) is compiled by both crates, so the
// pins cover the bytes the BPF side runs too. Pure by construction:
// fixtures in, assertions out, no host filesystem access (the miri
// contract).

use super::*;

/// A realistic x86-64 register snapshot for one KEY_A press as the
/// kernel dispatches it: rdi = the device pointer, rsi = EV_KEY,
/// rdx = 30 (KEY_A), rcx = 1 (press). Upper-bit garbage is planted
/// deliberately — see the truncation pins below.
const DEV: u64 = 0xffff_8880_1234_5678;

#[test]
fn kprobe_lane_extracts_the_four_arguments() {
    // The pt_regs fixture (PT_REGS_PARM1..PARM4): the kprobe wrapper
    // hands exactly these register values to the shim.
    let args = InputEventArgs::from_pt_regs(DEV, 0x01, 30, 1);
    assert_eq!(args.dev, DEV);
    assert_eq!(args.kind, 0x01);
    assert_eq!(args.code, 30);
    assert_eq!(args.value, 1);
    // The release shape through the same lane.
    let release = InputEventArgs::from_pt_regs(DEV, 0x01, 30, 0);
    assert_eq!(release.value, 0);
}

#[test]
fn fentry_lane_extracts_the_four_arguments() {
    // The BTF-slot fixture: the trampoline's typed argument slots for
    // the same call.
    let args = InputEventArgs::from_btf_slots(DEV, 0x01, 30, 1);
    assert_eq!(args.dev, DEV);
    assert_eq!(args.kind, 0x01);
    assert_eq!(args.code, 30);
    assert_eq!(args.value, 1);
}

#[test]
fn both_lanes_see_the_same_logical_input_identically() {
    // THE pin: the same keystroke, one call observed through the
    // pt_regs lane and the BTF-slot lane, must extract identically —
    // the tap body is written once, so the shims must not disagree.
    let kprobe = InputEventArgs::from_pt_regs(DEV, 0x01, 30, 1);
    let fentry = InputEventArgs::from_btf_slots(DEV, 0x01, 30, 1);
    assert_eq!(kprobe, fentry);
    // The release and autorepeat values too (autorepeat is filtered
    // later by the body, but the extraction itself must be identical).
    assert_eq!(
        InputEventArgs::from_pt_regs(DEV, 0x01, 30, 0),
        InputEventArgs::from_btf_slots(DEV, 0x01, 30, 0)
    );
    assert_eq!(
        InputEventArgs::from_pt_regs(DEV, 0x01, 30, 2),
        InputEventArgs::from_btf_slots(DEV, 0x01, 30, 2)
    );
}

#[test]
fn pt_regs_truncation_is_abi_faithful() {
    // `unsigned int` / `int` arguments occupy the low 32 bits of
    // their registers; the upper bits legally carry garbage (the
    // kernel's own C code reads the 32-bit halves). A garbage-laden
    // register and a clean one must produce the same event — pinned
    // per argument so a regression names its lane.
    let clean = InputEventArgs::from_pt_regs(DEV, 0x01, 30, 1);
    let garbage = InputEventArgs::from_pt_regs(
        DEV,
        0xdead_beef_0000_0001,
        0xcafe_babe_0000_001e,
        0xffff_ffff_0000_0001,
    );
    assert_eq!(garbage.kind, clean.kind);
    assert_eq!(garbage.code, clean.code);
    assert_eq!(garbage.value, clean.value);
    // Negative values (i32 lane) survive the register read: value
    // 2s-complements, and anything > 1 is filtered by the body as
    // not-a-press anyway.
    assert_eq!(
        InputEventArgs::from_pt_regs(DEV, 0x01, 30, 0xffff_ffff).value,
        -1
    );
}

#[test]
fn btf_slot_narrowing_agrees_with_the_register_lane() {
    // The fentry wrapper narrows register-width slots to the C
    // argument widths with the same truncation the kprobe lane
    // applies — the two halves of the equality from the test above,
    // stated directly so a future edit to one constructor cannot
    // quietly diverge.
    let slot_narrowed = InputEventArgs::from_btf_slots(
        DEV,
        0xdead_beef_0000_0001u64 as u32,
        0xcafe_babe_0000_001eu64 as u32,
        0xffff_ffff_0000_0001u64 as i32,
    );
    assert_eq!(
        slot_narrowed,
        InputEventArgs::from_pt_regs(
            DEV,
            0xdead_beef_0000_0001,
            0xcafe_babe_0000_001e,
            0xffff_ffff_0000_0001
        )
    );
}
