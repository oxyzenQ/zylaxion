// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

// attest_tests.rs — the ebpf/attest.rs test pins (the Pattern C test
// tree: every *.rs test file lives under the crate's test/ directory,
// wired via #[path]). The module position is unchanged, so super::*
// still resolves.

use super::*;

const PINNED: &str = PINNED_MAINTAINER_KEY_FPR;

/// The pin itself is load-bearing: a 40-hex fingerprint, and the
/// long id's tail matches the key asset the docs name.
#[test]
fn the_pinned_fingerprint_is_the_documented_shape() {
    assert_eq!(PINNED.len(), 40, "a full OpenPGP fingerprint is 40 hex");
    assert!(
        PINNED.bytes().all(|b| b.is_ascii_hexdigit()),
        "hex only: {PINNED}"
    );
    assert!(
        PINNED.ends_with("47A50AEF4B65AAC2"),
        "the long id docs/VERIFY_RELEASE.md publishes: {PINNED}"
    );
}

/// The estate-wide pin discipline: zylaxion's pin file is byte-for-
/// byte the zelynic reference's (the same maintainer signs both
/// projects' commits and releases), so the two policies can never
/// disagree about who the maintainer is.
#[test]
fn the_pin_matches_the_estate_maintainer_key() {
    assert_eq!(
        PINNED, "F5324E0967F104D58CE025F347A50AEF4B65AAC2",
        "the estate primary key docs/VERIFY_RELEASE.md documents"
    );
}

#[test]
fn verified_by_the_pinned_key_passes_naming_the_key() {
    let (passed, detail) = commit_signature_verdict(&format!("verified:{PINNED}"), "089badd");
    assert!(passed, "{detail}");
    assert!(
        detail.contains("089badd") && detail.contains("47A50AEF4B65AAC2"),
        "names the commit and the key: {detail}"
    );
}

/// The signing-subkey world: gpg's VALIDSIG reports the SUBKEY when
/// a subkey signed; build/gpg.rs already resolves to the primary
/// before stamping, so a stamped fpr that is not the pinned primary
/// is a foreign signer — even a well-formed one.
#[test]
fn verified_by_a_foreign_fingerprint_fails_by_name() {
    let foreign = "A".repeat(39) + "2";
    let (passed, detail) = commit_signature_verdict(&format!("verified:{foreign}"), "089badd");
    assert!(!passed, "a foreign signer must fail: {detail}");
    assert!(
        detail.contains("NOT the pinned maintainer key"),
        "names the tamper shape: {detail}"
    );
}

#[test]
fn wrong_key_fails_naming_both_keys() {
    let (passed, detail) = commit_signature_verdict(
        "wrong-key:0123456789abcdef0123456789abcdef01234567",
        "ab12cd3",
    );
    assert!(!passed);
    assert!(
        detail.contains("89abcdef01234567"),
        "the foreign key: {detail}"
    );
    assert!(detail.contains("47A50AEF4B65AAC2"), "the pin: {detail}");
}

#[test]
fn no_key_with_evidence_names_the_foreign_signer() {
    let (passed, detail) =
        commit_signature_verdict("no-key:0123456789abcdef0123456789abcdef01234567", "ab12cd3");
    assert!(!passed);
    assert!(detail.contains("signed by 0123456789abcdef0123456789abcdef01234567"));
    // The bare state (ERRSIG carried no full fpr) still fails.
    let (passed, _) = commit_signature_verdict("no-key", "ab12cd3");
    assert!(!passed);
}

#[test]
fn unsigned_fails_as_the_incomplete_chain() {
    let (passed, detail) = commit_signature_verdict("unsigned", "ab12cd3");
    assert!(!passed);
    assert!(
        detail.contains("NO GPG signature") && detail.contains("ab12cd3"),
        "the row states the gap: {detail}"
    );
}

#[test]
fn degraded_states_fail_their_honest_sentences() {
    for stamp in ["bad-signature", "no-vcs", "unavailable", "unstamped", ""] {
        let (passed, detail) = commit_signature_verdict(stamp, "ab12cd3");
        assert!(!passed, "{stamp} must fail");
        assert!(!detail.is_empty(), "{stamp} must say why");
    }
}
