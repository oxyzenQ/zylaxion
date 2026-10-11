// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// version_report_tests.rs — the cli/mod.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

/// night-improve-13 contract: the version report carries every
/// documented line exactly once, in the zelynic reference order —
/// Build, then Build-time directly after it, then Copyright,
/// License, Source.
#[test]
fn version_report_carries_the_zelynic_line_contract() {
    let body = version_body();
    for line in [
        "Copyright: (c) 2026 rezky_nightky (oxyzenQ)",
        "License: GPL-3.0-only",
        "Source: https://github.com/oxyzenQ/zylaxion",
    ] {
        let count = body.lines().filter(|l| l.contains(line)).count();
        assert_eq!(count, 1, "line must appear exactly once: {line}");
    }
    // Ordering: Build-time sits directly after the Build line.
    let lines: Vec<&str> = body.lines().collect();
    let build_idx = lines
        .iter()
        .position(|l| l.starts_with("Build: "))
        .expect("Build: line present");
    assert!(
        lines[build_idx + 1].starts_with("Build-time: "),
        "Build-time must directly follow Build: {lines:?}"
    );
}

/// The Build-time stamp carries the Hinnant shape
/// `M/D/YYYY HH:MM (UTC)` (or the documented "unknown" fallback) —
/// never an empty string or a foreign format.
#[test]
fn build_time_stamp_shape_is_m_d_yyyy_hh_mm_utc() {
    let line = version_body()
        .lines()
        .find(|l| l.starts_with("Build-time: "))
        .expect("Build-time line present")
        .trim_start_matches("Build-time: ")
        .to_string();
    if line == "unknown" {
        return; // degraded path, documented
    }
    assert!(line.ends_with(" (UTC)"), "shape: {line}");
    let body = line.trim_end_matches(" (UTC)");
    let (date, time) = body.split_once(' ').expect("M/D/YYYY HH:MM split");
    assert_eq!(date.matches('/').count(), 2, "M/D/YYYY shape: {date}");
    assert_eq!(time.matches(':').count(), 1, "HH:MM shape: {time}");
}

/// The clap surface and the intercepted surface share ONE body —
/// both carry the Build facts, so the two paths can never disagree
/// about provenance.
#[test]
fn long_version_embeds_the_shared_body() {
    let body = version_body();
    for line in body.lines() {
        assert!(
            long_version().contains(line),
            "clap surface must embed the shared body line: {line}"
        );
    }
}
