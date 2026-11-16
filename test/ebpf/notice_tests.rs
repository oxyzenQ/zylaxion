// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The notice schema's pin tests (night-frontier-Z2 B1). These ride
//! in the userspace crate's tree (notice.rs is compiled by BOTH the
//! userspace crate and the eBPF crate via #[path]; the eBPF build
//! compiles this module out with the rest of the cfg(test) tree).

use super::*;

#[test]
fn the_frame_is_sixteen_bytes_little_endian() {
    let frame = encode_frame(KIND_VOLUME, 61, 7);
    assert_eq!(frame.len(), NOTICE_FRAME_SIZE);
    assert_eq!(&frame[0..4], &NOTICE_MAGIC.to_le_bytes());
    assert_eq!(&frame[4..8], &KIND_VOLUME.to_le_bytes());
    assert_eq!(&frame[8..12], &61u32.to_le_bytes());
    assert_eq!(&frame[12..16], &7u32.to_le_bytes());
}

#[test]
fn the_frame_round_trips_through_the_relay_contract() {
    for (kind, value, seq) in [
        (KIND_ENABLED, 0u32, 0u32),
        (KIND_ENABLED, 1, 1),
        (KIND_PRESET, 4, 42),
        (KIND_VOLUME, 100, u32::MAX),
    ] {
        let frame = encode_frame(kind, value, seq);
        let decoded = decode_frame(&frame).expect("a valid frame always decodes");
        assert_eq!(decoded, (kind, value, seq));
    }
}

#[test]
fn foreign_and_short_frames_die_before_the_ring() {
    // Wrong magic: garbage never becomes a notice.
    let mut frame = encode_frame(KIND_VOLUME, 61, 7);
    frame[0] = 0x00;
    assert!(decode_frame(&frame).is_none());

    // A short frame: the length check fires first.
    let full = encode_frame(KIND_PRESET, 0, 1);
    assert!(decode_frame(&full[..NOTICE_FRAME_SIZE - 1]).is_none());
    // Zero bytes: same.
    assert!(decode_frame(&[]).is_none());
}

#[test]
fn the_record_layout_is_the_twentyfour_byte_contract() {
    // Both sides of the kernel boundary read the same 24 bytes.
    assert_eq!(std::mem::size_of::<ControlNotice>(), NOTICE_RECORD_SIZE);
    assert_eq!(std::mem::size_of::<ControlNotice>(), 24);
    // repr(C) field order: ns@0, kind@8, value@12, seq@16, pad@20.
    assert_eq!(std::mem::offset_of!(ControlNotice, ns), 0);
    assert_eq!(std::mem::offset_of!(ControlNotice, kind), 8);
    assert_eq!(std::mem::offset_of!(ControlNotice, value), 12);
    assert_eq!(std::mem::offset_of!(ControlNotice, seq), 16);
    assert_eq!(std::mem::offset_of!(ControlNotice, _pad), 20);
}

#[test]
fn the_record_field_set_is_zero_leakage_by_structure() {
    // The zero-leakage law, enforced structurally: the record carries
    // (timestamp, kind, value, seq) — there is NO keycode field, so a
    // control notice cannot ever carry key data. The events ring
    // keeps its own record shape (ts_us, keycode, value, slot) and
    // its own pin; the notice ring's schema has no slot to put one
    // in. This assert pins the field SET, not just the layout: a new
    // field here must re-justify itself against this law.
    let record = ControlNotice {
        ns: 1,
        kind: KIND_VOLUME,
        value: 50,
        seq: 2,
        _pad: 0,
    };
    // The kind vocabulary is exactly the sound-setting surface:
    // three distinct kinds, each unique in the frame contract (the
    // runtime set catches a duplicate constant the compiler cannot
    // warn about).
    let kinds = [KIND_ENABLED, KIND_PRESET, KIND_VOLUME];
    let mut seen = std::collections::BTreeSet::new();
    for kind in kinds {
        assert!(seen.insert(kind), "kind {kind} duplicates an existing kind");
    }
    // The value lanes are sound settings (percent, index, gate).
    let _ = (record.kind, record.value);
}
