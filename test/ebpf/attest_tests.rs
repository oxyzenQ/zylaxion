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

// ── the object anchor verification pins (moved with the core from
// test/info/signature_tests.rs, night-netrunner-Z1: the re-hash is
// the loader gate's core now, and the pins live beside it) ──────────

/// The pristine build's real self-check: the stamp this test binary's
/// own build.rs produced recomputes from the very TAP_ELF bytes the
/// binary carries. A build whose object was patched after staging
/// fails HERE, in every lane that compiles the crate — and the
/// loader's anchor gate refuses the same bytes before they reach
/// ring 0.
#[test]
fn the_embedded_tap_object_recomputes_the_build_stamp() {
    // env!, not option_env!.unwrap(): a test binary of this tree
    // cannot exist without the stamp (the build.rs lanes always
    // produce it), and the honest shape is the hard compile error.
    let stamped = env!("ZYLAXION_TAP_OBJECT_B3");
    let (passed, detail) = object_anchor_verdict(Some(stamped), crate::ebpf::loader::TAP_ELF);
    assert!(passed, "{detail}");
    assert!(
        detail.contains("recomputes from the embedded tap object"),
        "{detail}"
    );
    assert!(detail.starts_with("b3:"), "{detail}");
    // The pass detail carries the digest and nothing else - no drift
    // prose on the happy arm (the drift prose is the failure arm's).
    assert!(
        detail.ends_with("recomputes from the embedded tap object"),
        "{detail}"
    );
}

/// The tamper arm, through the real verify path: one flipped byte in
/// the embedded object must fail, naming BOTH digests - the recomputed
/// one and the stamped one. This is the exact verdict the loader's
/// anchor gate refuses the load on (AttachError::ObjectUnsealed).
#[test]
fn a_patched_tap_object_fails_naming_both_digests() {
    let stamped = "e642ace8b8e9a3f549434d21c45413688fc6e3ec3afb7292f078cd0850f996ec";
    let mut patched = crate::ebpf::loader::TAP_ELF.to_vec();
    let middle = patched.len() / 2;
    patched[middle] ^= 0x01;
    let (passed, detail) = object_anchor_verdict(Some(stamped), &patched);
    assert!(!passed, "{detail}");
    assert!(detail.contains("hashes to b3:"), "{detail}");
    assert!(detail.contains(stamped), "{detail}");
    assert!(detail.contains("patched after the build"), "{detail}");
}

/// The absence arm: a build without the stamp (an out-of-tree compile
/// that bypassed build.rs) fails with the reason named - never a
/// guess, never a pass. The loader refuses this shape too: an
/// unsealed camera is an unloaded camera.
#[test]
fn a_build_without_the_stamp_fails_naming_the_absence() {
    let (passed, detail) = object_anchor_verdict(None, crate::ebpf::loader::TAP_ELF);
    assert!(!passed, "{detail}");
    assert!(
        detail.contains("did not stamp the tap object digest"),
        "{detail}"
    );
}
