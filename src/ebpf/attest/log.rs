// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The attestation transparency log (night-netrunner-Z1): the
//! append-only, hash-chained record of what the loader actually put
//! in the kernel.
//!
//! The sig ladder proves the artifact (D0-D2 the claim, D3 the image,
//! D4 the ring-0 object, D5 the source channel) — but every one of
//! those verdicts is computed fresh from the binary's own bytes and
//! leaves no trace of WHAT RAN, WHEN, on WHICH host. The transparency
//! log is that trace: every successful attach appends one record —
//! the loaded object's digest, the commit, the commit-signature
//! state, the chain step that won, the runtime timestamp — sealed
//! into a BLAKE3 hash chain (each record's digest folds the previous
//! record's digest, certificate-transparency style). Afterward,
//! anyone with read access can replay the file and answer:
//!
//!   - which object bytes actually ran (the digest the GATE verified,
//!     not a claim), from which commit, under whose signature;
//!   - whether the history has been edited afterwards — a rewritten
//!     or deleted interior line breaks the chain BY LINE NUMBER;
//!   - how many attaches this host has seen (the record count).
//!
//! # Honest limits, stated plainly
//!
//! This is a LOCAL audit trail, not a public witness network: a
//! sufficiently privileged attacker can truncate the whole file or
//! rewrite it from scratch (rebuilding a coherent chain from the
//! genesis), because nothing external countersigns the head. The
//! chain detects IN-BAND tampering (edit, splice, truncate-from-the-
//! middle) exactly; wholesale replacement is invisible to it. The
//! log is evidence, not a gate: a failed append NEVER blocks attach
//! (the kernel state is the tap's truth, the log is its history) —
//! it warns and the attach proceeds. Single-writer by construction:
//! attach is the one-shot privileged path and the boot unit is
//! serialized; concurrent attaches on one host are not a supported
//! shape (the append rides O_APPEND so a lost race costs a torn line,
//! which the verifier names honestly, never a silent pass).
//!
//! # Record format (one line, grep-friendly, zero dependencies)
//!
//! ```text
//! v1 <timestamp> <commit> <tap-b3> <sig-state> <chain-step> <prev-b3> <record-b3>
//! ```
//!
//! `v1` is the grammar version; the timestamp is the runtime UTC
//! clock's ISO shape (`2026-10-11T07:41:44.284Z`, the zero-time-
//! crates lane in info::clock); `tap-b3` is the digest the anchor
//! gate verified over the bytes that actually loaded; `sig-state`
//! is the build's commit-signature stamp verbatim (evidence rides
//! the log); `chain-step` is the attach chain step that won;
//! `prev-b3` is the previous record's digest (64 zero hex for the
//! genesis line); `record-b3` is the BLAKE3 over the domain tag and
//! the record's own fields plus `prev-b3`. No field ever contains a
//! space, so `split_whitespace` is the whole parser.

use std::io::Write;
use std::path::Path;

/// The log's home: the persistent system state directory. attach runs
/// as root (the boot unit or sudo), so the write side has the
/// privilege; the read side (sig, the doctor) runs unprivileged
/// against the 0644 file the 0755 directory holds.
pub(crate) const ATTEST_LOG_PATH: &str = "/var/lib/zylaxion/attestation.log";

/// Domain separation for the chain's hash material — the same law the
/// depth-matrix anchor rides: this payload is distinct from every
/// other blake3 use in the tree.
const DOMAIN: &str = "zylaxion/attest-log/v1";

/// The genesis link: the `prev-b3` of the first record. All-zero hex
/// — a value no blake3 digest can collide with by construction of
/// the format (it is not a hash output, it is the declared absence
/// of one).
const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// One attestation record's fields (the line's first six fields; the
/// chain fields are derived by the writer).
pub(crate) struct AttestRecord {
    /// Runtime UTC timestamp, the info::clock ISO shape.
    pub timestamp: String,
    /// The commit the binary was built from (`build_hash`, possibly
    /// "unknown" — recorded verbatim, the log never guesses).
    pub commit: String,
    /// The blake3 (hex, no prefix) the anchor gate verified over the
    /// tap object bytes that actually loaded.
    pub tap_b3: String,
    /// The build's commit-signature stamp verbatim
    /// (`verified:<fpr>`, `unsigned`, ... — the D5 row's own input).
    pub sig_state: String,
    /// The attach chain step that won (the TapKind label).
    pub chain_step: String,
}

/// The record's digest: BLAKE3 over the domain tag and the record's
/// own fields plus the previous record's digest — the chain link.
/// Byte-exact mirror of the verifier's recompute; changing one side
/// without the other breaks every existing log.
fn record_digest(prev_b3: &str, rec: &AttestRecord) -> String {
    let payload = format!(
        "{DOMAIN} {} {} {} {} {} {prev_b3}",
        rec.timestamp, rec.commit, rec.tap_b3, rec.sig_state, rec.chain_step
    );
    blake3::hash(payload.as_bytes()).to_hex().to_string()
}

/// The full log line for one record (the append format AND the
/// verifier's recomputation source — one serializer, no drift).
fn record_line(prev_b3: &str, rec: &AttestRecord) -> String {
    let digest = record_digest(prev_b3, rec);
    format!(
        "v1 {} {} {} {} {} {prev_b3} {digest}",
        rec.timestamp, rec.commit, rec.tap_b3, rec.sig_state, rec.chain_step
    )
}

/// Append one record to the log, chaining onto the current head.
///
/// Fail-OPEN by design (the honest-limits note above): the log is
/// evidence, not a gate — an unwritable state directory (a read-only
/// host, a stripped container) must never block a legitimate attach.
/// The caller warns on `Err`; the attach proceeds.
pub(crate) fn append(path: &Path, rec: &AttestRecord) -> std::io::Result<()> {
    let prev = current_head(path).unwrap_or_else(|| GENESIS.to_string());
    let line = record_line(&prev, rec);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()
}

/// The current head digest (the last record's `record-b3`), `None`
/// when the file is absent or empty. `None` also answers a final
/// line that does not parse — the append then chains from the
/// genesis, and the verifier below names the torn tail honestly
/// instead of the writer guessing at it.
fn current_head(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let last = text.lines().rev().find(|l| !l.trim().is_empty())?;
    let tokens: Vec<&str> = last.split_whitespace().collect();
    if tokens.len() == 8 && tokens[0] == "v1" {
        Some(tokens[7].to_string())
    } else {
        None
    }
}

/// The replay verdict over a log file: record count, head digest,
/// chain integrity, and the first break's line number plus its
/// reason (both named, the estate's fails-by-name law).
pub(crate) struct ChainVerdict {
    /// How many records the file carries (parsed or not — a torn tail
    /// counts, and names itself in `reason`).
    pub records: usize,
    /// The last VALID record's digest, or the genesis when the file
    /// is empty/absent. On a broken chain this is the digest of the
    /// last record BEFORE the break — the honest head the surviving
    /// prefix proves.
    pub head: String,
    /// Whether every record verified and linked.
    pub intact: bool,
    /// The 1-based line number of the first break, when one exists.
    pub break_line: Option<usize>,
    /// The human sentence: why the chain broke, or "chain intact".
    pub reason: String,
}

/// Replay the whole log and verify the hash chain.
///
/// A missing file is the empty log — `records: 0`, the genesis head,
/// intact, and a reason that says so plainly (a host where attach
/// never ran has nothing to hide and nothing to show). Every
/// mismatch is named by line: a rewritten field (digest drift), a
/// spliced record (prev linkage), a truncated or torn line (shape).
pub(crate) fn verify_chain(path: &Path) -> ChainVerdict {
    let Ok(text) = std::fs::read_to_string(path) else {
        return ChainVerdict {
            records: 0,
            head: GENESIS.to_string(),
            intact: true,
            break_line: None,
            reason: "no attestation log on this host (attach has not run)".to_string(),
        };
    };
    let mut prev = GENESIS.to_string();
    let mut records = 0usize;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        if line.trim().is_empty() {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() != 8 || tokens[0] != "v1" {
            return broken(records, prev, number, "the line is not a v1 record");
        }
        let rec = AttestRecord {
            timestamp: tokens[1].to_string(),
            commit: tokens[2].to_string(),
            tap_b3: tokens[3].to_string(),
            sig_state: tokens[4].to_string(),
            chain_step: tokens[5].to_string(),
        };
        if tokens[6] != prev {
            return broken(
                records,
                prev,
                number,
                "the record's prev link does not match the chain head",
            );
        }
        let expected = record_digest(&prev, &rec);
        if tokens[7] != expected {
            return broken(
                records,
                prev,
                number,
                "the record's digest does not recompute (a field was edited)",
            );
        }
        prev = expected;
        records += 1;
    }
    ChainVerdict {
        records,
        head: prev,
        intact: true,
        break_line: None,
        reason: "chain intact".to_string(),
    }
}

/// The broken-chain verdict constructor (the surviving prefix's
/// facts plus the break's coordinates).
fn broken(records: usize, head: String, line: usize, why: &str) -> ChainVerdict {
    ChainVerdict {
        records,
        head,
        intact: false,
        break_line: Some(line),
        reason: format!("chain broken at line {line}: {why}"),
    }
}

#[cfg(test)]
#[path = "../../../test/ebpf/attest_log_tests.rs"]
mod tests;
