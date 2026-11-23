// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

// signature_tests.rs — the info/signature.rs test pins (the Pattern C
// test tree: every *.rs test file lives under the crate's test/
// directory, wired via #[path]). The module position is unchanged, so
// super::* still resolves.

use super::*;

/// Test oracles: the exact plaintext truths the masked words must
/// materialize. These literals live ONLY in the test build — they are
/// the one legitimate plaintext carrier in the module, because a test
/// that cannot name its expected output proves nothing. The release
/// artifact contains none of them (cfg(test) is stripped from the
/// binary).
const MANIFEST_ORACLE: &str = "Pure eBPF builtin — Official Build by rezky_nightky (oxyzenQ)";
const AUTHORS_ORACLE: &str = "rezky_nightky (oxyzenQ) <oxyzenQ@proton.me>";
/// Generator cross-check values (scripts/release/
/// generate-provenance-words.py prints them). If these diverge from the
/// const eval, the generator and the Rust mirror have drifted.
const MANIFEST_FINGERPRINT_ORACLE: u64 = 0x247D_2835_AC8B_DE6A;
const AUTHORS_FINGERPRINT_ORACLE: u64 = 0x6677_1E5F_273B_9CAE;
/// Generator's printed anchor cross-check (blake3 over the framed
/// payload). The Python mirror and the Rust crate were proven
/// byte-identical on the pinned vectors at integration time.
const DEPTH_ANCHOR_HEX_ORACLE: &str =
    "0f4a2f66ed24b96421509ec4f5e3b4f5688d984e8d6aacdfede18a27db451b0e";

#[test]
fn layers_materialize_to_oracles() {
    let view = provenance_view();
    assert_eq!(view.manifest, MANIFEST_ORACLE);
    assert_eq!(view.authors, AUTHORS_ORACLE);
}

#[test]
fn masked_words_are_strings_invisible() {
    // The at-rest invisibility contract, pinned mechanically:
    // strings(1) reports runs of four or more printable ASCII bytes.
    // Every byte of every masked word (both layers) must stay below
    // that floor, so a thief's grep over the artifact finds nothing.
    let layers: [&[u64]; 2] = [&MASKED_MANIFEST_WORDS, &MASKED_AUTHORS_WORDS];
    for words in layers {
        for word in words.iter() {
            let bytes = word.to_le_bytes();
            let mut run = 0usize;
            for byte in bytes {
                if (0x20..=0x7E).contains(&byte) {
                    run += 1;
                    assert!(run < 4, "masked word {word:#x} leaks an ASCII run");
                } else {
                    run = 0;
                }
            }
        }
    }
}

#[test]
fn const_fingerprints_match_generator_oracles() {
    // The generator's printed cross-check values must equal the Rust
    // const eval — otherwise the Python mask mirror and the Rust const
    // mirror have drifted and the block is corrupt.
    assert_eq!(MANIFEST_FINGERPRINT, MANIFEST_FINGERPRINT_ORACLE);
    assert_eq!(AUTHORS_FINGERPRINT, AUTHORS_FINGERPRINT_ORACLE);
}

#[test]
fn const_fingerprints_match_runtime_fold() {
    // Same traversal contract, checked end-to-end: const eval over the
    // words == runtime fold over the materialized bytes.
    let manifest_fp = fnv1a64_bytes(&unmask_to_bytes(
        &MASKED_MANIFEST_WORDS,
        MANIFEST_SEED,
        MANIFEST_BYTE_LEN,
    ));
    let authors_fp = fnv1a64_bytes(&unmask_to_bytes(
        &MASKED_AUTHORS_WORDS,
        AUTHORS_SEED,
        AUTHORS_BYTE_LEN,
    ));
    assert_eq!(manifest_fp, MANIFEST_FINGERPRINT);
    assert_eq!(authors_fp, AUTHORS_FINGERPRINT);
}

#[test]
fn depth_anchor_matches_generator_oracle() {
    // The embedded anchor equals the generator's printed cross-check
    // value: the paste is the value the Python mirror computed.
    assert_eq!(hex32(&DEPTH_ANCHOR), DEPTH_ANCHOR_HEX_ORACLE);
}

#[test]
fn depth_anchor_matches_runtime_blake3() {
    // The runtime recompute over the materialized bytes equals the
    // embedded anchor on a pristine build — the D3 layer passes.
    let manifest = unmask_to_bytes(&MASKED_MANIFEST_WORDS, MANIFEST_SEED, MANIFEST_BYTE_LEN);
    let authors = unmask_to_bytes(&MASKED_AUTHORS_WORDS, AUTHORS_SEED, AUTHORS_BYTE_LEN);
    assert_eq!(anchor_digest(&manifest, &authors), DEPTH_ANCHOR);
}

#[test]
fn anchor_payload_framing_is_exact() {
    // Domain tag, then length-prefixed layers: byte-exact mirror of the
    // generator's anchor_payload. Pinned against the real layer lengths
    // (63 manifest + 43 authors over the 24-byte domain tag).
    let manifest = unmask_to_bytes(&MASKED_MANIFEST_WORDS, MANIFEST_SEED, MANIFEST_BYTE_LEN);
    let authors = unmask_to_bytes(&MASKED_AUTHORS_WORDS, AUTHORS_SEED, AUTHORS_BYTE_LEN);
    let payload = anchor_payload(&manifest, &authors);
    assert!(payload.starts_with(b"zylaxion/depth-matrix/v1"));
    assert_eq!(&payload[24..32], &63u64.to_le_bytes());
    assert_eq!(&payload[32..95], manifest.as_slice());
    assert_eq!(&payload[95..103], &43u64.to_le_bytes());
    assert_eq!(&payload[103..146], authors.as_slice());
    assert_eq!(payload.len(), 146);
}

#[test]
fn tampered_payload_breaks_the_anchor() {
    // Post-compile patching simulation: flipping one byte of the
    // materialized payload must change the digest — the anchor binds
    // the bytes it claims to protect (cryptographic layer above the
    // FNV-64 fingerprints).
    let manifest = unmask_to_bytes(&MASKED_MANIFEST_WORDS, MANIFEST_SEED, MANIFEST_BYTE_LEN);
    let authors = unmask_to_bytes(&MASKED_AUTHORS_WORDS, AUTHORS_SEED, AUTHORS_BYTE_LEN);
    let mut forged_manifest = manifest.clone();
    forged_manifest[5] ^= 0x01;
    assert_ne!(anchor_digest(&forged_manifest, &authors), DEPTH_ANCHOR);
    let mut forged_authors = authors.clone();
    forged_authors[9] ^= 0x80;
    assert_ne!(anchor_digest(&manifest, &forged_authors), DEPTH_ANCHOR);
    // Dropping a layer from the framing (length-prefix mismatch) also
    // breaks it — the framing is part of the seal.
    assert_ne!(anchor_digest(&manifest, &[]), DEPTH_ANCHOR);
}

#[test]
fn tampered_words_break_the_fingerprint() {
    // Post-compile patching simulation: flipping one bit in any masked
    // word must change the digest — the fingerprint binds the data it
    // claims to protect.
    let mut tampered = MASKED_MANIFEST_WORDS;
    tampered[3] ^= 0x1;
    assert_ne!(
        layer_fingerprint(&tampered, MANIFEST_SEED, MANIFEST_BYTE_LEN),
        MANIFEST_FINGERPRINT,
        "patching a D0 word must break the fingerprint"
    );
    let mut tampered_authors = MASKED_AUTHORS_WORDS;
    tampered_authors[2] ^= 0x80;
    assert_ne!(
        layer_fingerprint(&tampered_authors, AUTHORS_SEED, AUTHORS_BYTE_LEN),
        AUTHORS_FINGERPRINT,
        "patching a D1 word must break the fingerprint"
    );
}

#[test]
fn identity_core_extraction_and_binding() {
    let core = identity_core(MANIFEST_ORACLE).expect("marker present");
    assert_eq!(core, "rezky_nightky (oxyzenQ)");
    assert!(AUTHORS_ORACLE.starts_with(core), "layers bind coherently");
    // A rebranded manifest (different identity) breaks the binding: the
    // authors string no longer starts with the extracted core.
    let forged = "Pure eBPF builtin — Official Build by someone_else";
    let forged_core = identity_core(forged).expect("marker present");
    assert!(!AUTHORS_ORACLE.starts_with(forged_core));
    // The retired year-suffix shape (night-ux-orion-3 dropped it) no
    // longer binds either: the core is the tail verbatim, so the year
    // rides inside the core and the authors prefix check rejects it.
    let retired = "Pure eBPF builtin — Official Build by rezky_nightky (oxyzenQ) 2026";
    let retired_core = identity_core(retired).expect("marker present");
    assert!(!AUTHORS_ORACLE.starts_with(retired_core));
    // A missing marker is the tampering signal.
    assert!(identity_core("Pure eBPF builtin").is_none());
}

#[test]
fn provenance_view_shape_is_stable() {
    let view = provenance_view();
    assert!(
        view.manifest.starts_with("Pure eBPF builtin — "),
        "manifest is the D0 display: {}",
        view.manifest
    );
    assert!(
        view.manifest.ends_with("rezky_nightky (oxyzenQ)"),
        "manifest ends with the owner identity, year-free"
    );
    assert!(
        !view.manifest.ends_with("2026"),
        "night-ux-orion-3: the claim carries no year suffix"
    );
    let depth = &view.depth_line;
    assert!(depth.starts_with("Depth-matrix: fp="), "depth line shape");
    assert!(
        depth.contains(" status=verified "),
        "pristine build reports verified"
    );
    assert!(
        depth.contains(
            "layers=d0:masked-words,d1:masked-words,d2:dual-fingerprint,d3:blake3-anchor"
        ),
        "layers token describes the masked + anchored scheme"
    );
    // The anchor rides the -V line in full, b3:-prefixed.
    let anchor_token = depth
        .split(" anchor=b3:")
        .nth(1)
        .and_then(|tail| tail.split(" layers=").next())
        .expect("anchor token present");
    assert_eq!(anchor_token, DEPTH_ANCHOR_HEX_ORACLE);
    assert_eq!(view.anchor_hex, DEPTH_ANCHOR_HEX_ORACLE);
    assert!(view.verified, "pristine view verifies");
    let fp = depth
        .split(" status=")
        .next()
        .and_then(|head| head.strip_prefix("Depth-matrix: fp="))
        .expect("fp= token present");
    assert_eq!(fp.len(), 16);
    assert!(
        fp.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
}

// ── night-depth-hardening-Z1 D2, the tap-object pins ─────────────────
// The tap-object verification tests moved with their core to
// test/ebpf/attest_tests.rs (night-netrunner-Z1): the re-hash lives
// in ebpf/attest.rs now, shared by the sig D4 row and the loader's
// anchor gate, and the pins live beside the code they pin.
