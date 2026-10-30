// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// version_report_tests.rs — the cli/mod.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

/// Test-local minimal strip for the only escape shapes this crate
/// emits: ESC ... m spans. The header pin rides the de-spanned text so
/// the equality holds on both sides of the color gate.
fn strip_brand_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_span = false;
    for ch in line.chars() {
        if ch == '\x1b' {
            in_span = true;
        } else if in_span {
            if ch == 'm' {
                in_span = false;
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// night-improve-13 contract: the version report carries every
/// documented line exactly once, in the zelynic reference order —
/// Build, then Build-time directly after it, then the Signature, then
/// Copyright, License, Source.
#[test]
fn version_report_carries_the_zelynic_line_contract() {
    let body = version_body();
    for line in [
        "Signature: Pure eBPF builtin — Official Build by rezky_nightky (oxyzenQ)",
        "Copyright: (c) 2026 rezky_nightky (oxyzenQ)",
        "License: GPL-3.0-only",
        "Source: https://github.com/oxyzenQ/zylaxion",
    ] {
        let count = body.lines().filter(|l| l.contains(line)).count();
        assert_eq!(count, 1, "line must appear exactly once: {line}");
    }
    // Ordering: Build-time sits directly after the Build line, the
    // Signature directly after Build-time (the zelynic reference
    // shape), Copyright directly after the Signature.
    let lines: Vec<&str> = body.lines().collect();
    let build_idx = lines
        .iter()
        .position(|l| l.starts_with("Build: "))
        .expect("Build: line present");
    assert!(
        lines[build_idx + 1].starts_with("Build-time: "),
        "Build-time must directly follow Build: {lines:?}"
    );
    assert!(
        lines[build_idx + 2].starts_with("Signature: "),
        "Signature must directly follow Build-time: {lines:?}"
    );
    assert!(
        lines[build_idx + 3].starts_with("Copyright: "),
        "Copyright must directly follow Signature: {lines:?}"
    );
}

/// night-dragon-maintenance-T1: the Signature line rides the zelynic
/// reference shape as ONE contiguous literal (SIGNATURE_LINE) — its
/// exact bytes are the bytes release.yml's verification legs grep out
/// of the binary (`grep -aFq`, no encoding layer). Pin the literal so
/// a reword fails here, at test time, not in a release leg; the
/// release probe's other greppable line stays contiguous too.
#[test]
fn signature_line_is_the_contiguous_release_grep_bytes() {
    assert_eq!(
        SIGNATURE_LINE,
        "Signature: Pure eBPF builtin — Official Build by rezky_nightky (oxyzenQ)"
    );
    let body = version_body();
    assert!(
        body.contains(SIGNATURE_LINE),
        "the body carries the signature verbatim, never a join"
    );
    assert!(
        body.contains("License: GPL-3.0-only"),
        "the release probe's license bytes stay contiguous"
    );
}

/// night-dragon-maintenance-T1: the estate-parity law (the zelynic
/// version_report_body_is_plain_estate_parity shape) — the -V body
/// rides the DEFAULT terminal color: no ANSI escape bytes in the body,
/// ever. The header is the only colored surface this report emits.
#[test]
fn version_report_body_is_plain_estate_parity() {
    let (_header, body) = version_report_payload();
    assert!(
        !body.contains('\x1b'),
        "the -V body must print plain — no color wrap around the multi-line build facts"
    );
}

/// The header payload is exactly the two brand lines — name+version,
/// then the tagline — and each line carries at most ONE brand span
/// (the single-line law: a multi-line span paints every line after
/// the first with it). The equality rides the DE-SPANNED text: the
/// gate is environment law (night-dragon-maintenance-T1, the red-Miri
/// incident — Miri's isatty answer is nightly-dependent), so the
/// spans are stripped before the plain-shape comparison and pinned
/// separately: zero under a captured harness, exactly the one
/// open/reset pair under a TTY probe, never anything else.
#[test]
fn version_report_header_is_two_single_brand_lines() {
    let (header, _body) = version_report_payload();
    let mut lines = header.lines();
    let first = lines.next().expect("the name+version line");
    let second = lines.next().expect("the tagline line");
    assert!(lines.next().is_none(), "the header is exactly two lines");
    for line in [first, second] {
        let spans = line.matches('\x1b').count();
        assert!(
            spans == 0 || spans == 2,
            "one brand span per line, never a multi-line span: {line:?}"
        );
    }
    assert_eq!(
        strip_brand_spans(first),
        format!("zylaxion: v{}", crate::info::build::VERSION)
    );
    assert_eq!(strip_brand_spans(second), ABOUT);
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
