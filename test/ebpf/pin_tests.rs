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
fn the_failure_marker_is_not_part_of_a_complete_attach() {
    // night-improve-25: the marker is planted ONLY by a failed attach
    // — a complete pin set never includes it, and a completeness check
    // that demanded it would read failure as success. It IS removed by
    // clear_pins (with the real pins), so detach leaves the directory
    // empty either way.
    assert!(!ALL_PINS.contains(&PIN_MAP_ERROR));
    assert!(PIN_MAP_ERROR.starts_with(PIN_DIR));
    // The marker path is distinct from every real pin.
    let mut paths = ALL_PINS.to_vec();
    paths.push(PIN_MAP_ERROR);
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(paths.len(), ALL_PINS.len() + 1);
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

// ── night-improve-26: the classified cleanup fixtures ──────────────────
//
// The REAL-path lanes (clear_pins, clear_pins_keeping_marker,
// remove_pin_dir) still NEVER run inside a test — the header note above
// holds. What the fixtures exercise is the shared core
// (remove_entries_and_dir + remove_dir_classified) over plain
// temp-directory entries: the classification logic is identical, while
// the bpffs unlink itself rides the integration lane. std-only
// fixtures — no new dependencies.

/// A unique per-test fixture directory (process id + test name —
/// parallel tests never collide). Re-created fresh on every call.
#[cfg_attr(miri, ignore)]
fn fixture_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "zylaxion-pin-fixture-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture directory must create");
    dir
}

#[test]
#[cfg_attr(miri, ignore)]
fn the_classified_rmdir_removes_a_stale_empty_directory() {
    // The stale-empty state that turned a prior failed attach into a
    // surprise EEXIST: the directory exists, nothing is in it. The
    // classified rmdir removes it (Removed), and a second attempt
    // reads as AlreadyAbsent — idempotent from any prior state.
    let dir = fixture_dir("stale-empty");
    let outcome = remove_dir_classified(&dir);
    assert!(
        matches!(outcome, PinDirRemoval::Removed),
        "a stale-empty directory must classify as Removed, got {outcome:?}"
    );
    let outcome = remove_dir_classified(&dir);
    assert!(
        matches!(outcome, PinDirRemoval::AlreadyAbsent),
        "the second rmdir must classify as AlreadyAbsent, got {outcome:?}"
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn the_cleanup_core_unpins_n_and_rmdirs_the_parent() {
    // N zylaxion entries: the core unpins all N (in the given order)
    // and the parent directory goes with them.
    let dir = fixture_dir("n-entries");
    let names = ["tap_link", "control", "stats"];
    let entries: Vec<String> = names
        .iter()
        .map(|n| dir.join(n).to_string_lossy().into_owned())
        .collect();
    for entry in &entries {
        std::fs::write(entry, b"pin").expect("fixture entry must create");
    }
    let (removed, outcome) = remove_entries_and_dir(
        &entries.iter().map(String::as_str).collect::<Vec<_>>(),
        &dir,
    );
    assert_eq!(removed.len(), entries.len(), "every entry must be removed");
    assert_eq!(removed, entries, "the removed order must mirror the input");
    assert!(
        matches!(outcome, PinDirRemoval::Removed),
        "an emptied parent must classify as Removed, got {outcome:?}"
    );
    assert!(!dir.exists(), "the parent directory must be gone");
}

#[test]
#[cfg_attr(miri, ignore)]
fn the_cleanup_core_leaves_foreign_entries_handled_not_fatal() {
    // A foreign entry lives in the directory: the zylaxion entries
    // still unpin, the foreign one stays (it is not ours to remove),
    // and the rmdir classifies as LeftForeignEntries — the ENOTEMPTY
    // is handled-not-fatal, the caller reports it honestly.
    let dir = fixture_dir("foreign-entry");
    let zylaxion = ["tap_link", "control"];
    let entries: Vec<String> = zylaxion
        .iter()
        .map(|n| dir.join(n).to_string_lossy().into_owned())
        .collect();
    for entry in &entries {
        std::fs::write(entry, b"pin").expect("fixture entry must create");
    }
    let foreign = dir.join("notours");
    std::fs::write(&foreign, b"someone else's pin").expect("foreign entry must create");

    let (removed, outcome) = remove_entries_and_dir(
        &entries.iter().map(String::as_str).collect::<Vec<_>>(),
        &dir,
    );
    assert_eq!(removed.len(), entries.len(), "only our entries count");
    assert!(
        matches!(outcome, PinDirRemoval::LeftForeignEntries),
        "a directory with foreign entries must classify as LeftForeignEntries, \
         got {outcome:?}"
    );
    assert!(
        foreign.exists(),
        "the foreign entry must survive the cleanup untouched"
    );
    assert!(
        dir.exists(),
        "the directory must survive while it is not empty"
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn the_cleanup_core_is_idempotent_on_an_already_clean_directory() {
    // Detach on a fresh host, detach on an already-detached host: the
    // second full cleanup removes nothing and the directory outcome is
    // AlreadyAbsent — the same success shape, not an error.
    let dir = fixture_dir("idempotent");
    let entries: Vec<String> = ["control"]
        .iter()
        .map(|n| dir.join(n).to_string_lossy().into_owned())
        .collect();
    std::fs::write(&entries[0], b"pin").expect("fixture entry must create");

    let (first_removed, first_outcome) = remove_entries_and_dir(
        &entries.iter().map(String::as_str).collect::<Vec<_>>(),
        &dir,
    );
    assert_eq!(first_removed.len(), 1);
    assert!(matches!(first_outcome, PinDirRemoval::Removed));

    let (second_removed, second_outcome) = remove_entries_and_dir(
        &entries.iter().map(String::as_str).collect::<Vec<_>>(),
        &dir,
    );
    assert!(
        second_removed.is_empty(),
        "the second cleanup must remove nothing"
    );
    assert!(
        matches!(second_outcome, PinDirRemoval::AlreadyAbsent),
        "the second rmdir must read as AlreadyAbsent, got {second_outcome:?}"
    );
}
