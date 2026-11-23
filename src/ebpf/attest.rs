// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The eBPF supply-chain attestation core (night-netrunner-Z1): the
//! runtime half of the GPG commit binding, pure over the build-time
//! stamp so every verdict class is unit-testable without a keyring.
//!
//! The gap this module closes (the supply-chain hole the owner's
//! adversarial review named, the zelynic night-solid-defense-2
//! answer): the depth matrix's D0-D2 prove the ownership claim, D3
//! fingerprints the whole image, D4 seals the embedded tap object —
//! but the `commit` line the sig report carries was a CLAIM, not a
//! bound fact. A binary swapped on the release channel (a MITM'd
//! release page, a hacked account) replays its own claims and the
//! ladder had no row that could disagree. The commit-signature
//! verdict closes exactly that: build/gpg.rs verified the commit the
//! binary was built from against the PINNED maintainer public key
//! (the repo's own tracked copy, imported into an isolated throwaway
//! keyring — never the building user's), and the sig ladder's D5 row
//! plus the loader's attestation record turn that verdict into a
//! reported, bound fact. The out-of-band twin: a consumer who
//! verified the maintainer key (keyservers, GitHub,
//! docs/VERIFY_RELEASE.md) verifies the SIGNED TAG the release was
//! cut from, the tag names the commit, and D5 proves the binary
//! really was built from a commit signed by that same key — the
//! chain from key to tag to commit to binary closes with no channel
//! left to MITM silently.
//!
//! Why this lives in the eBPF estate (and not beside the sig command
//! the way the zelynic reference placed its policy): zylaxion's
//! binding is LOAD-BEARING at the loader, not report-only — the
//! anchor gate and the attestation record the attach lane writes
//! consume this verdict at load time, so the policy sits with the
//! consumers that enforce it. The sig surface imports it from here;
//! one policy, one source of truth, zero drift between the row that
//! reports and the gate that binds.
//!
//! The pinned fingerprint is the SAME file build/gpg.rs reads —
//! `include_str!` on both sides, one source of truth, zero drift
//! between the verifier and the reporter.

/// The maintainer's pinned primary key fingerprint (ed25519,
/// `F5324E0967F104D58CE025F347A50AEF4B65AAC2`, long id
/// `47A50AEF4B65AAC2` — the identity that signs the release archives
/// per docs/VERIFY_RELEASE.md, and whose signing subkey signs the
/// commits this policy binds to). Single-sourced from
/// `assets/gpg/pinned-maintainer-fpr.txt` (no trailing newline: the
/// include is the value).
pub(crate) const PINNED_MAINTAINER_KEY_FPR: &str =
    include_str!("../../assets/gpg/pinned-maintainer-fpr.txt");

/// The commit-signature verdict: `(passed, detail)` for the sig
/// ladder's D5 row and the loader's attestation record. Every state
/// of the build-time stamp maps to an honest sentence; only a
/// verified match against the pinned fingerprint passes — unsigned,
/// foreign-key, broken, missing and unknown all FAIL, naming their
/// shape (the estate's fails-by-name law).
#[must_use]
pub(crate) fn commit_signature_verdict(stamp: &str, commit_sha: &str) -> (bool, String) {
    let (state, evidence) = match stamp.split_once(':') {
        Some((state, evidence)) => (state, Some(evidence)),
        None => (stamp, None),
    };
    match state {
        "verified" => {
            let fpr = evidence.unwrap_or_default();
            if fpr == PINNED_MAINTAINER_KEY_FPR {
                (
                    true,
                    format!(
                        "commit {commit_sha} carries a GPG signature by the pinned \
                         maintainer key {} (verified against the repo's pinned public \
                         key at build time)",
                        short_fpr(fpr)
                    ),
                )
            } else {
                // A VALIDSIG that is not the pin: the signature is
                // real but the signer is not the maintainer — the
                // MITM shape, named by fingerprint.
                (
                    false,
                    format!(
                        "commit {commit_sha} is signed by {} — NOT the pinned \
                         maintainer key {}",
                        short_fpr(fpr),
                        short_fpr(PINNED_MAINTAINER_KEY_FPR)
                    ),
                )
            }
        }
        "wrong-key" => (
            false,
            format!(
                "commit {commit_sha} carries a valid GPG signature by {} — NOT the \
                 pinned maintainer key {}",
                short_fpr(evidence.unwrap_or("an unknown key")),
                short_fpr(PINNED_MAINTAINER_KEY_FPR)
            ),
        ),
        "no-key" => (
            false,
            format!(
                "commit {commit_sha} carries a signature the pinned maintainer key \
                 does not cover{} — not the maintainer's signature",
                evidence
                    .map(|fpr| format!(" (signed by {fpr})"))
                    .unwrap_or_default()
            ),
        ),
        "bad-signature" => (
            false,
            format!(
                "commit {commit_sha} carries a GPG signature that FAILED the \
                 cryptographic check"
            ),
        ),
        "unsigned" => (
            false,
            format!(
                "commit {commit_sha} carries NO GPG signature — the provenance \
                 chain stops at an unbound commit; bind releases to signed \
                 commits/tags"
            ),
        ),
        "no-vcs" => (
            false,
            "built from a source snapshot without VCS evidence — the commit \
             signature cannot be verified here"
                .to_string(),
        ),
        "unavailable" => (
            false,
            "signature evidence exists but the build machine could not verify \
             (git/gpg unavailable at build time)"
                .to_string(),
        ),
        // "unstamped" and anything a future stamp grammar emits but
        // this binary does not know: fail closed, never a silent
        // pass (the D4 missing-stamp law).
        other => (
            false,
            format!("no commit-signature verdict ({other}) — fail-closed"),
        ),
    }
}

/// The display shape for fingerprints: 16-hex long id when the input
/// carries at least that much, else the input as-is (an honest
/// shorter token beats a truncated lie).
fn short_fpr(fpr: &str) -> String {
    if fpr.len() >= 16 {
        fpr[fpr.len() - 16..].to_string()
    } else {
        fpr.to_string()
    }
}

#[cfg(test)]
#[path = "../../test/ebpf/attest_tests.rs"]
mod tests;
