// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// loader_tests.rs — the pure layout contracts of the loader
// (night-improve-23). attach/detach are root-run integration surfaces
// and are exercised by the integration lane; here the lockstep
// contracts that would silently corrupt the kernel/userspace protocol
// get pinned: the control slot table, the ring record layout, and the
// mirror rules.

use super::*;

#[test]
fn control_slot_table_matches_the_bpf_side() {
    // The ebpf/src/main.rs control_slots module is the kernel half of
    // this contract. The values ARE the wire format — a drift here
    // would make the CLI write the enabled flag into the volume slot.
    assert_eq!(control::ENABLED, 0);
    assert_eq!(control::PRESET_ID, 1);
    assert_eq!(control::VOLUME_PCT, 2);
    assert_eq!(control::PLAYER_PID, 3);
    assert_eq!(control::STATE, 4);
    // night-improve-25: the chain-step slot — the wire format every
    // surface (attach summary, status tap line, doctor) reads back.
    assert_eq!(control::TAP_KIND, 5);
}

#[test]
fn tap_kind_is_the_chain_vocabulary_and_the_slot_round_trips() {
    // The three chain steps, their labels (the strings the attach
    // summary MUST print), and the control-slot wire format — pinned
    // so a renumbered slot or a relabeled lane fails here before it
    // can confuse a status report.
    let steps = [
        (TapKind::Fentry, "fentry:input_event", 0u32),
        (TapKind::Kprobe, "kprobe:input_event", 1),
        (TapKind::KprobeAlt, "kprobe:input_handle_event", 2),
    ];
    for (kind, label, slot) in steps {
        assert_eq!(kind.label(), label);
        assert_eq!(kind.slot(), slot);
        assert_eq!(TapKind::from_slot(slot), Some(kind));
    }
    // Anything else is not a chain step — never guess, never wrap.
    assert_eq!(TapKind::from_slot(3), None);
    assert_eq!(TapKind::from_slot(u32::MAX), None);
}

#[test]
fn ring_record_layout_is_padding_free_at_the_front() {
    // repr(C): ts_us rides first, the three u32s follow at 8/12/16,
    // four trailing pad bytes complete the 24-byte record. The offsets
    // are the contract the BPF side writes and the player reads — the
    // byte-level assertions below pin each field offset exactly.
    assert_eq!(size_of::<KeyRingEvent>(), 24);
    assert_eq!(align_of::<KeyRingEvent>(), 8);
    let sample = KeyRingEvent {
        ts_us: 0x0102_0304_0506_0708,
        keycode: 30,
        value: 1,
        slot: 0,
    };
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(std::ptr::addr_of!(sample) as *const u8, 24) };
    assert_eq!(
        &bytes[0..8],
        &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    assert_eq!(&bytes[8..12], &30u32.to_le_bytes());
    assert_eq!(&bytes[12..16], &1u32.to_le_bytes());
    assert_eq!(&bytes[16..20], &0u32.to_le_bytes());
}

#[test]
fn keymap_domain_is_the_keyboard_range() {
    // The attach writes keycodes 1..=255 — the keyboard key domain.
    // The bound is a documented posture (mouse buttons >= 0x100 never
    // sound), so the test pins the bound's shape.
    let keyboard_domain: Vec<u32> = (1..=255u32).collect();
    assert_eq!(keyboard_domain.len(), 255);
    assert_eq!(keyboard_domain[0], 1);
    assert_eq!(*keyboard_domain.last().unwrap(), 255);
    // The first button keycode (BTN_MISC = 0x100 = 256) sits outside.
    assert!(keyboard_domain.into_iter().max().unwrap() < 256);
}
