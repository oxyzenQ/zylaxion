// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The `sig` command surface — the six-depth signature report
//! (night-signature-advanced-1, the cosmostrix reference port;
//! night-depth-hardening-Z1 D2 adds the fifth depth, the zejtron
//! night-frontier-Z5 shape; night-netrunner-Z1 adds the sixth, the
//! zelynic night-solid-defense-2 shape).
//!
//! The owner runs sibling tools across the fleet (zejtron and zelynic
//! render the same ladder over their own provenance layers), so
//! `zylaxion sig` renders the same verdict ladder over zylaxion's own
//! provenance layers:
//!
//!   D0  runtime-identity — the masked manifest materializes, carries
//!       the marker and the identity shape.
//!   D1  embedded-marker — the masked authors layer materializes and
//!       binds to the manifest identity (coherent pair).
//!   D2  anchor-integrity — the embedded blake3 anchor recomputes from
//!       the materialized payload (the cryptographic seal).
//!   D3  binary-self-hash — blake3 over the running executable image.
//!   D4  tap-object — the embedded eBPF object re-hashes to the
//!       build-time stamp (night-depth-hardening-Z1 D2): the ring-0
//!       asset joins the self-verification circle.
//!   D5  commit-signature — the commit the binary claims is GPG-signed
//!       by the PINNED maintainer key, verified at build time against
//!       the repo's own pinned public key in an isolated keyring
//!       (night-netrunner-Z1; the policy lives in
//!       [`crate::ebpf::attest`]). This is the row that binds the
//!       provenance claim to the release channel: with the maintainer
//!       key verified out-of-band and releases cut from signed tags,
//!       D5 closes the key → tag → commit → binary chain — the binary
//!       swapped on the wire can no longer replay its claims
//!       unanswered.
//!
//! Numbering note: these D0-D3 are the four VERIFICATION depths
//! (family-parity labels, the owner's cross-tool contract). The `-V`
//! Depth-matrix line's d0:-d3: tokens are the four STORAGE layers
//! (masked manifest, masked authors, FNV fingerprints, blake3 anchor)
//! — a different axis, established in `info::signature` and unchanged.
//! The two numbering systems are documented here so a reader never has
//! to guess which axis a `D2` refers to.
//!
//! The first three depths are the `-V` Depth-matrix line rendered per
//! depth; they reuse `provenance_view()`'s single pass (the invariant
//! documented there — the report shows exactly the bytes the integrity
//! folds verify). D3, D4 and D5 are exclusive to this command: D3
//! reads the whole image, D4 re-hashes the embedded object, D5 reports
//! the build-time commit-signature verdict, and none of the three
//! belongs on the `-V` path.
//!
//! # Honest limits of D3
//!
//! A self-hash cannot embed its own digest (the chicken-and-egg law of
//! hashing), so D3 is a FINGERPRINT, not a self-verifying seal: it is
//! the value to compare against a published release hash (or a
//! previously recorded local hash) to prove the exact image running.
//! `pass` means the image was read in full and hashed; the comparison
//! is the caller's. The docs and the report text below say this
//! plainly — no fake "verified against itself" claim anywhere.
//!
//! # D4 closes D3's gap for the one asset that matters most
//!
//! The chicken-and-egg law applies to the IMAGE, not to the assets it
//! carries: the tap object's digest CAN be embedded, because the build
//! stamps it over the staged object before the bytes are compiled in
//! (build/tapstamp.rs). D4 is therefore a true self-verify, not a
//! fingerprint: the binary re-hashes the object it carries and refuses
//! a post-build patch by name — the ring-0 asset is proven to be the
//! object the build staged (and on the registry lane, the object the
//! manifest pins).
//!
//! # D5 closes the channel gap the artifact's own bytes cannot
//!
//! D4 seals the object INSIDE the binary; D5 binds the binary to the
//! SOURCE CHANNEL it claims. The row reports the BUILD's own verified
//! verdict (not a runtime re-verification — the keyring is a build-time
//! fact), so a tree on an unsigned HEAD reads FAIL "no GPG signature"
//! (5/6) and a tree whose HEAD is signed by the pinned maintainer key
//! reads AUTHENTIC 6/6 — both are the pristine truth for their build,
//! and the report never claims otherwise (the honesty law the tests
//! pin). An AGPL fork built from its own (differently-signed or
//! unsigned) commits fails D5 by name: that is the verdict working,
//! not a false positive — the fork is honestly not the maintainer's
//! build, and its own sig report says so under its own identity
//! layers.

use std::io::Read;

use super::super::info::signature::provenance_view;

/// Verdict of the depth ladder.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SigVerdict {
    /// Every depth passed.
    Authentic,
    /// At least one depth failed; the report names which.
    Tampered,
}

/// One rendered depth row: `D{n} <name>  {pass|FAIL} — {detail}`.
struct DepthRow {
    name: &'static str,
    passed: bool,
    detail: String,
}

/// blake3 over the running executable image, read in chunks (the
/// release artifact is megabytes; streaming keeps the allocation flat).
/// `Err` names the failure honestly (unreadable image, mid-read error)
/// and fails the depth instead of guessing.
fn binary_self_hash() -> Result<String, String> {
    let path =
        std::env::current_exe().map_err(|e| format!("cannot locate the running image: {e}"))?;
    let mut file =
        std::fs::File::open(&path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0usize;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read error on {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n;
    }
    if total == 0 {
        return Err("image is empty".to_string());
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest.as_bytes() {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(hex)
}

/// Run the six-depth ladder and render the full report block.
///
/// The text is stable by contract (tests pin its shape, like the `-V`
/// report): the purple banner header, six depth rows, an identity
/// field block, then the verdict. Exit code is the dispatcher's job
/// (`sig` exits 2 on [`SigVerdict::Tampered`], the fatal-CLI code this
/// tree already uses), so the report itself stays side-effect free.
#[must_use]
pub(crate) fn sig_report() -> (String, SigVerdict) {
    let view = provenance_view();

    // D0: the manifest materialized and carries the identity shape.
    let identity = view
        .manifest
        .split_once("Official Build by ")
        .map(|(_, tail)| tail.to_string())
        .unwrap_or_default();
    let d0_shape_ok = !identity.is_empty();

    // D1: the authors layer materialized and binds to the identity.
    // night-ux-orion-3: the manifest is year-free, so the identity
    // core is the tail verbatim — the old `" 2026"` strip would read
    // the new shape as a tampering signal.
    let binding_ok = view
        .manifest
        .split_once("Official Build by ")
        .is_some_and(|(_, core)| view.authors.starts_with(core));

    // D2: the cryptographic anchor (already recomputed in the same
    // single pass that produced the display bytes).
    let anchor_ok = view.verified;

    // D3: the running image's own blake3 fingerprint.
    let self_hash = binary_self_hash();

    // D4: the embedded tap object against the build-time stamp
    // (night-depth-hardening-Z1 D2) - the verification now covers the
    // bytes that execute at ring 0, not only the identity claim.
    let tap_object = crate::info::signature::tap_object_digest_check();

    // D5: the commit-signature binding (night-netrunner-Z1). The
    // stamp is the build's own verified verdict over the claimed
    // commit; the policy (pinned fingerprint, honest sentences for
    // every state) lives in ebpf/attest.rs — the same module the
    // loader's attestation record binds through, one policy for the
    // row that reports and the lane that loads.
    let (sig_ok, sig_detail) = crate::ebpf::attest::commit_signature_verdict(
        crate::info::build::vcs_commit_sig(),
        crate::info::build::build_hash(),
    );

    // night-ux-orion-3 (completing the night-ux-orion-2 form): the
    // manifest itself no longer carries the claim year, so the holder
    // renders year-free verbatim — a year belongs to the front of the
    // Copyright line, never as a suffix after the identity. A tampered
    // manifest renders whatever it materializes: the honest evidence
    // stays visible.
    let identity_clean = identity.clone();

    let rows = [
        DepthRow {
            name: "runtime-identity",
            passed: d0_shape_ok,
            // night-depth-infra-1: a D0 failure renders a NAMED reason,
            // never an empty detail — an empty one leaves the row line
            // ending in a trailing space after the em-dash.
            detail: if identity.is_empty() {
                "the manifest identity did not materialize".to_string()
            } else {
                identity_clean.clone()
            },
        },
        DepthRow {
            name: "embedded-marker",
            passed: binding_ok,
            detail: "the masked authors layer binds to the manifest identity".to_string(),
        },
        DepthRow {
            name: "anchor-integrity",
            passed: anchor_ok,
            detail: format!("b3:{} recomputes from the masked payload", view.anchor_hex),
        },
        DepthRow {
            name: "binary-self-hash",
            passed: self_hash.is_ok(),
            detail: match &self_hash {
                Ok(hex) => format!("b3:{hex}"),
                Err(e) => format!("unavailable: {e}"),
            },
        },
        DepthRow {
            // night-depth-hardening-Z1 D2: the embedded tap object
            // re-hashes to the build-time stamp - the ring-0 asset
            // joins the self-verification circle, and a post-build
            // patch of the object is refused by name (both digests in
            // the detail, the exit code hard non-zero below).
            name: "tap-object",
            passed: tap_object.0,
            detail: tap_object.1,
        },
        DepthRow {
            // night-netrunner-Z1: the commit-signature binding. The
            // row reports the BUILD's own verified verdict over the
            // claimed commit — an unsigned HEAD honestly reads FAIL
            // here (the chain stops at an unbound commit), a HEAD
            // signed by the pinned maintainer key reads pass, and a
            // foreign signer is named by fingerprint (the MITM shape).
            name: "commit-signature",
            passed: sig_ok,
            detail: sig_detail,
        },
    ];

    let depths_passed = rows.iter().filter(|r| r.passed).count();
    let verdict = if depths_passed == rows.len() {
        SigVerdict::Authentic
    } else {
        SigVerdict::Tampered
    };

    let mut text = String::new();
    text.push_str(&crate::output::section("zylaxion signature — depth matrix"));
    text.push('\n');
    for (i, row) in rows.iter().enumerate() {
        let status = if row.passed { "pass" } else { "FAIL" };
        text.push_str(&format!(
            "D{} {:<18} {} \u{2014} {}\n",
            i, row.name, status, row.detail
        ));
    }
    text.push('\n');
    // night-ux-orion-2: the field block carries the holder (the year-
    // free claim above) and the contact line extracted from the
    // materialized authors layer's bracketed address. The extraction
    // fails soft and honestly: a tampered authors layer renders
    // "unknown" or omits the contact line, and the depth rows above
    // already name the failure. The address exists at rest only as
    // masked u64 words (night-remake-1), so the artifact's strings(1)
    // sweep still finds no claim to strip.
    let identity_display = if identity.is_empty() {
        "unknown".to_string()
    } else {
        identity_clean
    };
    text.push_str(&format!("identity    {identity_display}\n"));
    let contact = view
        .authors
        .split_once('<')
        .and_then(|(_, tail)| tail.strip_suffix('>'))
        .unwrap_or_default();
    if !contact.is_empty() {
        text.push_str(&format!("contact     {contact}\n"));
    }
    text.push_str(&format!(
        "fingerprint fp={:016x} (fnv1a64, dual-layer)\n",
        super::super::info::signature::manifest_fingerprint()
    ));
    text.push_str(&format!("anchor      b3:{}\n", view.anchor_hex));
    let commit = crate::info::build::build_hash();
    if commit == "unknown" {
        text.push_str("commit      unknown\n");
    } else {
        text.push_str(&format!("commit      {commit}\n"));
    }
    text.push_str(&format!(
        "build-time  {}\n",
        crate::info::build::build_time()
    ));
    text.push('\n');
    text.push_str(&format!(
        "verdict: SIGNATURE: {} ({depths_passed}/{} depths verified)\n",
        if verdict == SigVerdict::Authentic {
            "AUTHENTIC"
        } else {
            "TAMPERED"
        },
        rows.len()
    ));
    // The tail carries NO trailing newline (night-sig-tail-1): the
    // terminator is print_stdout_line's to own, the same convention
    // the -V report rides — sig_report's text ending in '\n' made the
    // printer add a second one, and the report closed with two blank
    // lines where -V closes with none.
    text.push_str("note: the self-hash is a fingerprint to compare against a\n");
    text.push_str("published release hash; it cannot embed its own digest.");

    (text, verdict)
}

/// Print the report (the `sig` dispatch calls this, then exits 2 on
/// tamper so scripts can gate on the exit code).
pub(crate) fn print_sig_report() {
    let (text, verdict) = sig_report();
    crate::output::print_stdout_line(&text);
    if verdict == SigVerdict::Tampered {
        // Flush the report before the hard exit: the verdict IS the
        // error surface, and a swallowed buffer would hide it.
        use std::io::Write;
        let _ = std::io::stdout().flush();
        std::process::exit(2);
    }
}

/// The `zylaxion sig` verb: render the depth-matrix report and exit 2
/// when any depth fails, so release lanes and scripts can gate on it.
pub fn cmd_sig() {
    print_sig_report();
}

#[cfg(test)]
#[path = "../../test/commands/sig_tests.rs"]
mod tests;
