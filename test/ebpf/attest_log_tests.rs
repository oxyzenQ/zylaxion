// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

// attest_log_tests.rs — the ebpf/attest/log.rs pins (the Pattern C
// test tree). The chain is exercised through real files in the
// system temp dir: append, replay, tamper, truncate — every arm the
// honest-limits doc names.

use super::*;
use std::path::PathBuf;

/// One temp log path per test (the append/verify round-trips must
/// never share state through a fixed name).
fn temp_log(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "zylaxion-attest-log-test-{tag}-{}-{}",
        std::process::id(),
        line!()
    ))
}

/// A sample record with stable fields (the digests in the pinned
/// expectations below derive from exactly these values — changing
/// the fixture changes every expectation, on purpose: the chain is
/// byte-exact or it is nothing).
fn sample_record(commit: &str, tap_b3: &str) -> AttestRecord {
    AttestRecord {
        timestamp: "2026-11-23T16:30:00.000Z".to_string(),
        commit: commit.to_string(),
        tap_b3: tap_b3.to_string(),
        sig_state: "verified:F5324E0967F104D58CE025F347A50AEF4B65AAC2".to_string(),
        chain_step: "fentry:input_event".to_string(),
    }
}

#[test]
#[cfg_attr(
    miri,
    ignore = "real file I/O in the system temp dir - the native CI lane owns it"
)]
fn the_genesis_log_verifies_as_empty_and_honest() {
    let path = temp_log("genesis");
    let _ = std::fs::remove_file(&path);
    let verdict = verify_chain(&path);
    assert!(verdict.intact, "{}", verdict.reason);
    assert_eq!(verdict.records, 0);
    assert_eq!(verdict.head, "0".repeat(64));
    assert!(
        verdict.reason.contains("no attestation log"),
        "the empty verdict states the absence:\n{}",
        verdict.reason
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "real file I/O in the system temp dir - the native CI lane owns it"
)]
fn append_then_replay_answers_one_intact_record() {
    let path = temp_log("one");
    let _ = std::fs::remove_file(&path);
    let rec = sample_record("089badd", &"a".repeat(64));
    append(&path, &rec).expect("append works in a writable temp dir");
    let verdict = verify_chain(&path);
    assert!(verdict.intact, "{}", verdict.reason);
    assert_eq!(verdict.records, 1);
    assert_eq!(verdict.break_line, None);
    // The head is the record's digest — recomputed independently by
    // the verifier, so the append and the replay agree by construction
    // only when both sides serialize identically.
    let expected = record_digest(&"0".repeat(64), &sample_record("089badd", &"a".repeat(64)));
    assert_eq!(verdict.head, expected);
    let _ = std::fs::remove_file(&path);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "real file I/O in the system temp dir - the native CI lane owns it"
)]
fn the_second_record_chains_onto_the_firsts_digest() {
    let path = temp_log("chain");
    let _ = std::fs::remove_file(&path);
    append(&path, &sample_record("089badd", &"a".repeat(64))).expect("first append");
    let first_head = verify_chain(&path).head;
    append(&path, &sample_record("81f24b6", &"b".repeat(64))).expect("second append");
    let verdict = verify_chain(&path);
    assert!(verdict.intact, "{}", verdict.reason);
    assert_eq!(verdict.records, 2);
    // The second record's line names the first record's digest as its
    // prev link — the chain property, read straight off the file.
    let text = std::fs::read_to_string(&path).expect("the log exists");
    let second = text.lines().nth(1).expect("two records");
    assert!(
        second.contains(&first_head),
        "the second line links the first head:\n{text}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "real file I/O in the system temp dir - the native CI lane owns it"
)]
fn an_edited_field_breaks_the_chain_by_line_number() {
    let path = temp_log("tamper");
    let _ = std::fs::remove_file(&path);
    append(&path, &sample_record("089badd", &"a".repeat(64))).expect("first append");
    append(&path, &sample_record("81f24b6", &"b".repeat(64))).expect("second append");
    // The tamper: rewrite the FIRST record's commit (a splice that
    // tries to make history say a different commit loaded).
    let text = std::fs::read_to_string(&path).expect("the log exists");
    let tampered = text.replace("089badd", "deadbee");
    assert_ne!(tampered, text, "the tamper must change something");
    std::fs::write(&path, tampered).expect("write the tampered copy");
    let verdict = verify_chain(&path);
    assert!(!verdict.intact, "an edited field must break the chain");
    assert_eq!(verdict.break_line, Some(1), "the break names line 1");
    assert!(
        verdict.reason.contains("does not recompute"),
        "the reason names the drift:\n{}",
        verdict.reason
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "real file I/O in the system temp dir - the native CI lane owns it"
)]
fn a_spliced_record_breaks_the_prev_linkage() {
    let path = temp_log("splice");
    let _ = std::fs::remove_file(&path);
    append(&path, &sample_record("089badd", &"a".repeat(64))).expect("first append");
    // The splice: a second line whose prev link is NOT the first
    // record's digest (a record lifted from another host's log).
    let forged = format!(
        "v1 2026-11-23T17:00:00.000Z deadbee {} unsigned kprobe:input_event {} {}\n",
        "c".repeat(64),
        "0".repeat(64),
        "d".repeat(64)
    );
    let mut text = std::fs::read_to_string(&path).expect("the log exists");
    text.push_str(&forged);
    std::fs::write(&path, text).expect("write the spliced log");
    let verdict = verify_chain(&path);
    assert!(!verdict.intact, "a spliced record must break the chain");
    assert_eq!(verdict.break_line, Some(2));
    assert!(
        verdict.reason.contains("prev link"),
        "the reason names the linkage break:\n{}",
        verdict.reason
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "real file I/O in the system temp dir - the native CI lane owns it"
)]
fn a_torn_tail_line_is_named_not_swallowed() {
    let path = temp_log("torn");
    let _ = std::fs::remove_file(&path);
    append(&path, &sample_record("089badd", &"a".repeat(64))).expect("append");
    // The torn tail: a partial line, the shape a lost race or a full
    // disk leaves behind. The verifier names it by line number — the
    // surviving prefix's facts stay readable.
    let mut text = std::fs::read_to_string(&path).expect("the log exists");
    text.push_str("v1 2026-11-23T18:00:00.000Z half");
    std::fs::write(&path, text).expect("write the torn log");
    let verdict = verify_chain(&path);
    assert!(!verdict.intact, "a torn line must not verify");
    assert_eq!(verdict.break_line, Some(2));
    assert_eq!(verdict.records, 1, "the surviving prefix still counts");
    assert!(
        verdict.reason.contains("not a v1 record"),
        "the reason names the shape:\n{}",
        verdict.reason
    );
    let _ = std::fs::remove_file(&path);
}

/// The digest is domain-separated and byte-exact: the same fields
/// under a different previous head answer a different digest (the
/// chain property's pure core, no file needed).
#[test]
fn the_digest_folds_the_previous_head() {
    let rec = sample_record("089badd", &"a".repeat(64));
    let genesis = record_digest(&"0".repeat(64), &rec);
    let linked = record_digest(&genesis, &rec);
    assert_ne!(genesis, linked, "the prev head must change the digest");
    assert_eq!(genesis.len(), 64);
    assert!(
        genesis
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
}
