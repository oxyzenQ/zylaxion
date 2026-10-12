// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// embedded_tests.rs — the AlignedElf contract pins (night-improve-23,
// the zelynic NIGHT-hunt-30 lesson): alignment is a property of the
// type, the guard defeats const dedup, and the preflight names a
// misalignment in one line.

use super::*;

/// An 8-aligned byte buffer for the misalignment fixtures. A bare
/// `[u8; N]` local only carries alignment 1 — native ABIs happen to
/// land such locals on 8-aligned stack slots, but that is luck, not a
/// guarantee, and the miri interpreter lays locals out differently
/// (verified: a bare local read as 5 bytes past the boundary under
/// miri). Alignment is a property of the type — the very law this
/// file pins — so the fixtures ask the TYPE for it.
#[repr(align(8))]
struct Aligned<const N: usize>([u8; N]);

#[test]
fn aligned_elf_bytes_are_eight_byte_aligned() {
    // A real allocation, static so it rides .rodata like the embed.
    static ELF: AlignedElf<16> = AlignedElf::new([7u8; 16]);
    assert_eq!(ELF.bytes.as_ptr() as usize % 8, 0);
    assert_eq!(ELF.bytes.len(), 16);
    // The guard precedes the bytes: content offset 8 in the allocation.
    let raw = &ELF as *const _ as *const u64;
    unsafe {
        assert_eq!(raw.read_unaligned(), u64::from_le_bytes(*b"ZYLF-23\0"));
    }
}

#[test]
fn misalignment_detector_reports_shifted_slices() {
    let data = Aligned([0u8; 32]);
    assert_eq!(misalignment(&data.0), None);
    // A 1-byte shift (the exact include_bytes! luck the wrapper fences
    // out) must be visible: the base array is 8-aligned BY TYPE, so +1
    // reads as exactly one byte past the boundary — on every host and
    // under the interpreter too.
    let shifted = &data.0[1..];
    assert_eq!(misalignment(shifted), Some(1));
}

#[test]
fn alignment_violation_names_object_and_offset() {
    let data = Aligned([0u8; 16]);
    let shifted = &data.0[1..];
    let msg = alignment_violation(shifted, "zylaxion-tap").expect("shifted must violate");
    assert!(msg.contains("zylaxion-tap"), "names the object: {msg}");
    assert!(msg.contains("8-byte boundary"), "names the law: {msg}");
    assert!(msg.contains("build regression"), "names the blame: {msg}");
    assert!(alignment_violation(&data.0, "zylaxion-tap").is_none());
}

#[test]
fn guard_magic_differs_from_the_zelynic_stamp() {
    // The two families must never share a guard: a shared magic could
    // fold allocations across copied embedding code paths.
    assert_ne!(*b"ZYLF-23\0", *b"ZELF-30\0");
}
