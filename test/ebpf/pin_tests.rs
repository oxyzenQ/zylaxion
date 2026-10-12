// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// pin_tests.rs — the pin path contract pins (night-improve-23). These
// are pure tests on purpose: clear_pins() touches real bpffs paths and
// must NEVER run inside a test (on a root dev host with a live tap it
// would detach the production tap) — the removal path is verified by
// the integration lane, not here.

use super::*;

#[test]
fn every_pin_lives_under_the_pin_directory() {
    for p in ALL_PINS {
        assert!(
            p.starts_with(PIN_DIR),
            "{p} escapes the zylaxion pin directory"
        );
    }
}

#[test]
fn the_link_pin_comes_first_in_removal_order() {
    // Unlinking the LINK detaches the tap — it must be the first thing
    // clear_pins removes, so an interrupted cleanup leaves a DETACHED
    // tap (safe) rather than a live one with missing maps (confusing).
    assert_eq!(ALL_PINS[0], PIN_LINK);
}

#[test]
fn pin_set_covers_program_link_and_four_maps() {
    assert_eq!(ALL_PINS.len(), 6);
    assert!(ALL_PINS.contains(&PIN_PROG));
    assert!(ALL_PINS.contains(&PIN_LINK));
    assert!(ALL_PINS.contains(&PIN_MAP_CONTROL));
    assert!(ALL_PINS.contains(&PIN_MAP_KEYMAP));
    assert!(ALL_PINS.contains(&PIN_MAP_EVENTS));
    assert!(ALL_PINS.contains(&PIN_MAP_STATS));
}

#[test]
fn pins_are_distinct_paths() {
    // A duplicate constant would make clear_pins double-remove and
    // presence checks lie.
    let mut sorted = ALL_PINS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ALL_PINS.len());
}

#[test]
#[cfg_attr(miri, ignore)]
fn presence_checks_agree_with_the_filesystem() {
    // Pure consistency: all-present implies any-present. The check
    // reads the LIVE pin state (statx on bpffs paths), so the miri
    // interpreter refuses it by design — isolation forbids host file
    // probes, and on pin-less hosts (CI) the conditional is vacuous
    // anyway; the assertion earns its keep on dev hosts where a live
    // tap exists. Fenced from the interpreter lane, kept everywhere
    // else.
    if all_pins_present() {
        assert!(any_pin_present());
    }
}
