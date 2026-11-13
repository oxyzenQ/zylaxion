// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

// sig_tests.rs — the commands/sig.rs test pins (the Pattern C test
// tree: every *.rs test file lives under the crate's test/ directory,
// wired via #[path]). The module position is unchanged, so super::*
// still resolves.

use super::*;

#[test]
fn pristine_build_reports_authentic_four_of_four() {
    let (text, verdict) = sig_report();
    assert_eq!(verdict, SigVerdict::Authentic);
    assert!(
        text.contains("SIGNATURE: AUTHENTIC (4/4 depths verified)"),
        "{text}"
    );
    for row in [
        "D0 runtime-identity",
        "D1 embedded-marker",
        "D2 anchor-integrity",
        "D3 binary-self-hash",
    ] {
        assert!(text.contains(row), "missing depth row {row}:\n{text}");
    }
}

#[test]
fn report_names_the_owner_identity() {
    let (text, _) = sig_report();
    assert!(
        text.contains("identity    rezky_nightky (oxyzenQ) 2026"),
        "identity field must materialize the masked layers:\n{text}"
    );
    assert!(
        text.contains("anchor      b3:"),
        "anchor field present:\n{text}"
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "UB-poor whole-image file I/O plus a megabyte-scale hash \
        over the harness binary — the native CI lane owns the D3 probe"
)]
fn self_hash_hashes_the_test_image() {
    // In the test build the "running image" is the test harness
    // binary; the only honest invariant available here is that the
    // hash succeeds and is 64 lowercase hex chars (or fails with a
    // named error on exotic hosts — both are honest outcomes).
    match binary_self_hash() {
        Ok(hex) => {
            assert_eq!(hex.len(), 64);
            assert!(
                hex.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            );
        }
        Err(e) => assert!(!e.is_empty(), "failure must name a reason"),
    }
}

#[test]
fn report_shape_is_stable() {
    let (text, verdict) = sig_report();
    assert!(
        text.starts_with("━━━ zylaxion signature — depth matrix ━━━"),
        "the report opens with the family banner frame:\n{text}"
    );
    assert!(text.contains("verdict: SIGNATURE: "));
    assert!(text.contains("note: the self-hash is a fingerprint"));
    assert!(matches!(
        verdict,
        SigVerdict::Authentic | SigVerdict::Tampered
    ));
}
