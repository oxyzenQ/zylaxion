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

// --- The object anchor verification (the loader gate's core) ---------
//
// night-netrunner-Z1: the ONE verification core both consumers ride —
// the sig ladder's D4 tap-object row REPORTS it, the loader's anchor
// gate REFUSES on it. Until this move the re-hash lived in
// info/signature.rs and the loader loaded TAP_ELF unverified: a
// binary whose embedded object was patched after the build failed
// the sig report but ATTACHED anyway — the ring-0 payload ran on the
// strength of bytes the build never staged. The gate (loader/
// orchestrate.rs) now refuses the load on the same verdict the
// report renders, so the two surfaces can never disagree about
// whether the object is sealed.
//
// The black_box barrier rides the same law the anchor recompute in
// info/signature.rs documents: the embedded bytes are compile-time
// constants, so the fold must be forced to read the artifact's real
// memory — a constant-folded recompute would verify a patched binary
// as happily as a pristine one. The pure function takes (stamped,
// embedded) so every arm is pinnable in the test tree without
// patching a real artifact: the pristine build feeds the real stamp
// and the real bytes.
//
// Honest limits (the same shape the D3 self-hash row documents): an
// attacker who fully understands the stamp can re-stamp a rebuilt
// binary; the verdict proves the embedded object is exactly what the
// build staged, not what the author intended. The commit-signature
// binding above and the release checksum files remain the outer
// circles.

/// The object anchor verification: recompute the blake3 over the
/// embedded tap object bytes (through the black_box barrier) and
/// compare it to the build-time stamp. Three arms, all named: absent
/// stamp (the check cannot run — an honest fail, never a pass),
/// match (the embedded object is exactly what the build staged),
/// drift (both digests named — the tap was patched after the build).
#[must_use]
pub(crate) fn object_anchor_verdict(stamped: Option<&str>, embedded: &[u8]) -> (bool, String) {
    use std::hint::black_box;
    let Some(stamped) = stamped else {
        return (
            false,
            "the build did not stamp the tap object digest".to_string(),
        );
    };
    let recomputed = hex32(blake3::hash(black_box(embedded)).as_bytes());
    if recomputed == stamped {
        (
            true,
            format!("b3:{recomputed} recomputes from the embedded tap object"),
        )
    } else {
        (
            false,
            format!(
                "the embedded tap object hashes to b3:{recomputed}, the build stamped \
                 b3:{stamped} - the tap was patched after the build"
            ),
        )
    }
}

/// Lowercase hex for the `b3:` renderings (the D4 row and the gate's
/// refusal detail name both digests in this shape).
fn hex32(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
#[path = "../../test/ebpf/attest_tests.rs"]
mod tests;
