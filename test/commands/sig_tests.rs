// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

// sig_tests.rs — the commands/sig.rs test pins (the Pattern C test
// tree: every *.rs test file lives under the crate's test/ directory,
// wired via #[path]). The module position is unchanged, so super::*
// still resolves.

use super::*;

#[test]
#[cfg_attr(
    miri,
    ignore = "sig_report runs the D3 probe (current_exe/readlink) which Miri isolation refuses - the native CI lane owns it"
)]
fn pristine_build_reports_the_stamped_signature_truth() {
    let (text, verdict) = sig_report();
    // The ladder's honesty law (night-netrunner-Z1, the zelynic
    // night-solid-defense-2 shape): the D5 row reports the BUILD's
    // own verified verdict, so a tree on an unsigned HEAD reads FAIL
    // "no GPG signature" (5/6) and a tree whose HEAD is signed by the
    // pinned maintainer key reads AUTHENTIC 6/6 — both are the
    // pristine truth for their build, and the report must never claim
    // otherwise.
    let stamp = crate::info::build::vcs_commit_sig();
    let sig_verified = stamp
        .strip_prefix("verified:")
        .is_some_and(|fpr| fpr == crate::ebpf::attest::PINNED_MAINTAINER_KEY_FPR);
    if sig_verified {
        assert_eq!(verdict, SigVerdict::Authentic, "{text}");
        assert!(
            text.contains("SIGNATURE: AUTHENTIC (6/6 depths verified)"),
            "{text}"
        );
    } else {
        assert_eq!(verdict, SigVerdict::Tampered, "{text}");
        let d5 = text
            .lines()
            .find(|line| line.starts_with("D5 commit-signature"))
            .unwrap_or_else(|| panic!("the D5 row must exist:\n{text}"));
        assert!(
            d5.contains(" FAIL "),
            "an unstamped-truth build must fail D5 by name:\n{text}"
        );
    }
    for row in [
        "D0 runtime-identity",
        "D1 embedded-marker",
        "D2 anchor-integrity",
        "D3 binary-self-hash",
        "D4 tap-object",
        "D5 commit-signature",
    ] {
        assert!(text.contains(row), "missing depth row {row}:\n{text}");
    }
}

#[test]
#[cfg_attr(
    miri,
    ignore = "sig_report runs the D3 probe (current_exe/readlink) which Miri isolation refuses - the native CI lane owns it"
)]
fn report_names_the_owner_identity() {
    let (text, _) = sig_report();
    // night-ux-orion-3: the manifest itself is year-free, so the
    // identity renders year-free verbatim and the contact line
    // materializes the authors layer's bracketed address.
    assert!(
        text.contains("identity    rezky_nightky (oxyzenQ)"),
        "identity field must materialize the masked layers without the claim year:\n{text}"
    );
    assert!(
        !text.contains("(oxyzenQ) 2026"),
        "no line of the report may carry the claim year suffix:\n{text}"
    );
    assert!(
        text.contains("contact     oxyzenQ@proton.me"),
        "contact field must materialize the authors layer's address:\n{text}"
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
#[cfg_attr(
    miri,
    ignore = "sig_report runs the D3 probe (current_exe/readlink) which Miri isolation refuses - the native CI lane owns it"
)]
fn report_shape_is_stable() {
    let (text, verdict) = sig_report();
    assert!(
        text.starts_with("━━━ zylaxion signature — depth matrix ━━━"),
        "the report opens with the family banner frame:\n{text}"
    );
    assert!(text.contains("verdict: SIGNATURE: "));
    assert!(text.contains("note: the self-hash is a fingerprint"));
    // The tail law (night-sig-tail-1): the text ends AT the digest
    // sentence — no trailing newline of its own, the terminator is
    // print_stdout_line's to add (the -V parity: one '\n' total, not
    // two).
    assert!(
        text.ends_with("it cannot embed its own digest."),
        "the report tail must not carry a bare trailing newline:\n{text:?}"
    );
    assert!(matches!(
        verdict,
        SigVerdict::Authentic | SigVerdict::Tampered
    ));
}

/// night-netrunner-Z1: the attest-log field line's three render
/// shapes, pinned over constructed verdicts (the pure renderer, no
/// log file needed at the root-owned path).
#[test]
fn attest_log_line_renders_its_three_honest_shapes() {
    use crate::ebpf::attest::log::ChainVerdict;
    // Absent: the honest "attach has not run on this host".
    let absent = ChainVerdict {
        records: 0,
        head: "0".repeat(64),
        intact: true,
        break_line: None,
        reason: "no attestation log on this host (attach has not run)".to_string(),
    };
    let line = attest_log_line(&absent);
    assert!(
        line.starts_with("attest-log  absent — "),
        "the absent shape:\n{line}"
    );
    // Intact: records, head, the reason.
    let intact = ChainVerdict {
        records: 3,
        head: "a".repeat(64),
        intact: true,
        break_line: None,
        reason: "chain intact".to_string(),
    };
    let line = attest_log_line(&intact);
    assert!(
        line.contains("3 records") && line.contains("chain intact"),
        "the intact shape:\n{line}"
    );
    // Broken: the tamper named by line number.
    let broken = ChainVerdict {
        records: 2,
        head: "b".repeat(64),
        intact: false,
        break_line: Some(3),
        reason: "chain broken at line 3: the record's digest does not recompute".to_string(),
    };
    let line = attest_log_line(&broken);
    assert!(
        line.contains("TAMPERED HISTORY at line 3"),
        "the broken shape names the line:\n{line}"
    );
}
