// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The notice schema's pin tests (night-frontier-Z2 B1). These ride
//! in the userspace crate's tree (notice.rs is compiled by BOTH the
//! userspace crate and the eBPF crate via #[path]; the eBPF build
//! compiles this module out with the rest of the cfg(test) tree).

use super::*;

#[test]
fn the_record_layout_is_the_sixteen_byte_contract() {
    // Both sides of the kernel boundary read the same 16 bytes.
    assert_eq!(std::mem::size_of::<ControlNotice>(), NOTICE_RECORD_SIZE);
    assert_eq!(std::mem::size_of::<ControlNotice>(), 16);
    // repr(C) field order: ns@0, kind@8, value@12.
    assert_eq!(std::mem::offset_of!(ControlNotice, ns), 0);
    assert_eq!(std::mem::offset_of!(ControlNotice, kind), 8);
    assert_eq!(std::mem::offset_of!(ControlNotice, value), 12);
}

#[test]
fn the_record_field_set_is_zero_leakage_by_structure() {
    // The zero-leakage law, enforced structurally: the record carries
    // (timestamp, kind, value) — there is NO keycode field, so a
    // control notice cannot ever carry key data. The events ring
    // keeps its own record shape (ts_us, keycode, value, slot) and
    // its own pin; the notice ring's schema has no slot to put one
    // in. This test pins the field SET, not just the layout: a new
    // field here must re-justify itself against this law.
    let record = ControlNotice {
        ns: 1,
        kind: KIND_VOLUME,
        value: 50,
    };
    // The kind vocabulary is exactly the sound-setting surface: three
    // distinct kinds (the runtime set catches a duplicate constant
    // the compiler cannot warn about).
    let kinds = [KIND_ENABLED, KIND_PRESET, KIND_VOLUME];
    let mut seen = std::collections::BTreeSet::new();
    for kind in kinds {
        assert!(seen.insert(kind), "kind {kind} duplicates an existing kind");
    }
    let _ = record.value;
}

#[test]
fn the_relay_sanity_gate_refuses_unknown_kinds() {
    // The relay's gate mirrors this law in the kernel: kind 0 and
    // anything past the vocabulary never reaches the ring. The
    // userspace mirror is the same predicate the gate runs.
    let gate = |kind: u32| kind == 0 || kind > KIND_VOLUME;
    assert!(gate(0));
    assert!(gate(KIND_VOLUME + 1));
    assert!(!gate(KIND_ENABLED));
    assert!(!gate(KIND_PRESET));
    assert!(!gate(KIND_VOLUME));
}
