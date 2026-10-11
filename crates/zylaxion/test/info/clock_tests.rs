// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// clock_tests.rs — the info/clock.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

/// Every constant verified against `date -u -d @<epoch>` at authoring
/// time (not from memory) — the zelynic verified-constant discipline.
#[test]
fn iso_format_matches_known_unix_epochs() {
    // UNIX epoch: 1970-01-01 00:00:00 UTC.
    assert_eq!(iso_from_unix_secs(0, 0), "1970-01-01T00:00:00.000Z");

    // 2000-01-01 00:00:00 UTC = 946_684_800 seconds since epoch.
    assert_eq!(
        iso_from_unix_secs(946_684_800, 0),
        "2000-01-01T00:00:00.000Z"
    );

    // 2024-02-29 12:34:00 UTC = 1_709_210_040 seconds since epoch.
    // Leap-day boundary check — Feb 29 must not roll to Mar 1.
    assert_eq!(
        iso_from_unix_secs(1_709_210_040, 284),
        "2024-02-29T12:34:00.284Z"
    );

    // Day rollover: 1_709_251_200 = 2024-03-01 00:00:00 UTC — the
    // second after the leap day must land on Mar 1, hour 00.
    assert_eq!(
        iso_from_unix_secs(1_709_251_200, 1),
        "2024-03-01T00:00:00.001Z"
    );
}

#[test]
fn iso_format_handles_negative_seconds_gracefully() {
    // -60 = 1969-12-31 23:59:00 UTC — signed arithmetic, no panic.
    assert_eq!(iso_from_unix_secs(-60, 0), "1969-12-31T23:59:00.000Z");
}

#[test]
fn live_clock_shape_is_parseable() {
    // The live lane must render the exact 24-char stamp shape so
    // downstream log greps stay stable.
    let stamp = iso_utc_millis();
    if stamp == "unknown" {
        return; // pre-epoch host clock: documented degraded lane
    }
    assert_eq!(stamp.len(), 24, "stamp shape drifted: {stamp}");
    assert!(stamp.ends_with('Z'), "stamp must end in Z: {stamp}");
    assert_eq!(&stamp[4..5], "-", "shape: {stamp}");
    assert_eq!(&stamp[10..11], "T", "shape: {stamp}");
}
