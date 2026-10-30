// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Build script — stamps the compile-time facts the binary reports about
//! itself:
//!
//! - `GIT_HASH` — the short commit hash the crate was built from
//!   (consumed by the version report's `Build:` line), resolved by the
//!   three-step commit-sha chain (night-dragon-maintenance-T1, the
//!   zejtron lineage) so a registry install never reports a bare
//!   "unknown":
//!   1. `git rev-parse --short=7 HEAD` — local/git checkouts;
//!   2. `GITHUB_SHA` env — CI environments (full 40-hex sha, normalized
//!      to the same 7-char short shape);
//!   3. `.cargo_vcs_info.json` — registry tarball builds (`cargo
//!      install zylaxion`): cargo materializes this file beside the
//!      manifest when it packs the crate, carrying the sha of the
//!      commit the release was cut from, so builds without a `.git`
//!      directory still recover the real commit.
//!      The chain normalizer accepts only 7..=40 ASCII hex characters —
//!      every source renders one canonical lowercase 7-char shape, and
//!      "unknown" survives only when all three steps fail (a tarball built
//!      from a non-git source, unreachable in the shipping pipeline).
//! - `ZYLAXION_BUILD_TIME` — the UTC build timestamp, computed by Howard
//!   Hinnant's `civil_from_days` algorithm (night-improve-13, the zelynic
//!   NIGHT-hunt-6 lineage) so the version report's `Build-time:` line and
//!   the doctor's BUILD block never need a time crate (chrono/jiff and
//!   friends stay out of the dependency tree entirely).
//! - `ZYLAXION_RUSTC_VERSION` / `ZYLAXION_PROFILE` / `ZYLAXION_OPT_LEVEL`
//!   — the toolchain and profile facts for the doctor's BUILD block (the
//!   zelynic night-improve-66 completeness).
//! - `ZYLAXION_BUILD` — the canonical build label forwarded from the
//!   environment when an alias/CI build stamps one (see `build_info`).
//! - `ZYLAXION_EBPF_LANE` — which lane produced (or skipped) the embedded
//!   eBPF object (night-improve-23): `source-built` (a git checkout with
//!   the ebpf/ tree — the nested nightly cross-build), `registry-prebuilt`
//!   (a registry tarball — the maintainer-built object staged from
//!   ebpf-prebuilt/), or the dormant refusal (the `ebpf` feature off).
//!   The staging machinery lives in build/ebpf.rs (the zelynic build/
//!   module precedent — the 600-LOC no-mercy cap demands the split).
//!   Since night-dinner-4 the source-built lane also self-heals the
//!   ebpf-prebuilt/ mirror: after staging, the generator script's
//!   --check verdict runs and a stale lane regenerates automatically
//!   (the one-writer script does the writing; the parity gate still
//!   enforces the pin at commit time).
//!
//! Every fallback degrades gracefully — a missing fact renders as
//! "unknown", never a failed build.

use std::process::Command;

fn main() {
    // Re-run this script whenever HEAD (or the work tree state) changes so
    // the embedded hash stays in sync with what was actually compiled.
    // night-dragon-lts: the paths carried a `../` prefix — the leftover
    // of the old crates/zylaxion/ layout — which resolved against the
    // PARENT directory's .git, so a HEAD move in this repo never
    // triggered a rebuild and GIT_HASH could go stale until some other
    // trigger fired. The package root IS the repo root; the zejtron
    // reference's paths (.git/HEAD + .git/refs/heads) are the convention.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    // The commit-sha resolution chain (the sibling zejtron lineage),
    // first hit wins — see the module doc for the lane each step
    // serves. One normalizer keeps the three sources in one canonical
    // 7-char lowercase shape, so `-V` reads identically across a dev
    // checkout, a CI run, and a registry install.
    let hash = git_short_sha()
        .or_else(|| env_short_sha("GITHUB_SHA"))
        .or_else(packaged_vcs_sha)
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={}", hash);
    // Watched so a registry extract and a dev checkout never share one
    // stale verdict: the file only exists on tarball builds, and its
    // bytes are the third chain source's whole truth.
    println!("cargo:rerun-if-changed=.cargo_vcs_info.json");

    // The UTC build timestamp (Hinnant civil-from-days, std clock only —
    // no time crate). Consumed by build_info::build_time().
    println!(
        "cargo:rustc-env=ZYLAXION_BUILD_TIME={}",
        format_build_time_utc()
    );

    // The doctor's BUILD facts (zelynic night-improve-66 lineage): the
    // rustc cargo itself drove, the profile mode, and the real
    // optimization level — so a RUSTFLAGS override shows up honestly.
    let rustc_version = rustc_stamp();
    println!("cargo:rustc-env=ZYLAXION_RUSTC_VERSION={rustc_version}");
    println!(
        "cargo:rustc-env=ZYLAXION_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string())
    );
    println!(
        "cargo:rustc-env=ZYLAXION_OPT_LEVEL={}",
        std::env::var("OPT_LEVEL").unwrap_or_else(|_| "?".to_string())
    );

    // Forward the ZYLAXION_BUILD canonical label into the compile-time env
    // consumed by build_info::build_label(). The label is exported by CI
    // / release scripts (the release-matrix platform id, e.g.
    // `linux-amd64-v3-gnu`); plain `cargo build` leaves it unset and the
    // compile-time arch+libc detection answers instead.
    println!("cargo:rerun-if-env-changed=ZYLAXION_BUILD");
    if let Ok(label) = std::env::var("ZYLAXION_BUILD") {
        println!("cargo:rustc-env=ZYLAXION_BUILD={label}");
    }

    // night-improve-23: stage the eBPF object into OUT_DIR for the
    // loader's include_bytes! embedding (the machinery lives in
    // build/ebpf.rs — the zelynic build/ module precedent, the 600-LOC
    // no-mercy cap demands the split).
    self::ebpf::build_ebpf_objects();
}

#[path = "build/ebpf.rs"]
mod ebpf;

// The registry-prebuilt lane (night-dragon-maintenance-0, the zejtron
// build/ split): stages the maintainer-built ebpf-prebuilt/ object
// into OUT_DIR and refuses any extract whose bytes do not match the
// manifest's object blake3 pin. build/ebpf.rs's registry branch
// delegates here.
#[path = "build/prebuilt.rs"]
mod prebuilt;

// The dependency-free BLAKE3 the prebuilt lane's manifest
// verification hashes with (the same vectors the reference b3sum
// binary answers to; the standalone build suite pins both sides).
#[path = "build/blake3.rs"]
mod blake3;

// ── The commit-sha chain (the sibling zejtron build.rs lineage) ─────

/// First step of the commit-sha chain: `git rev-parse --short=7 HEAD`
/// from the workspace root. Returns `None` when git is absent, fails
/// (no `.git` — a registry extract), or answers something other than
/// usable hex.
fn git_short_sha() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    normalize_short_sha(&stdout)
}

/// Second step of the commit-sha chain: a full sha from the CI env
/// (`GITHUB_SHA`), normalized to the same 7-char short shape.
fn env_short_sha(var: &str) -> Option<String> {
    normalize_short_sha(&std::env::var(var).ok()?)
}

/// Parses the `sha1` field out of a `.cargo_vcs_info.json` document.
/// Pure string extraction — build.rs stays dependency-free (std
/// only), so there is no JSON crate here. The document shape is
/// stable: `{"git":{"sha1":"<40 hex>","dirty":bool},"path_in_vcs":""}`
/// (field order is not guaranteed, so the parse anchors on the
/// `"sha1"` key, not on byte offsets).
fn parse_vcs_sha_json(text: &str) -> Option<String> {
    let key = "\"sha1\"";
    let key_at = text.find(key)?;
    let after_key = &text[key_at + key.len()..];
    let colon_at = after_key.find(':')?;
    let value = after_key[colon_at + 1..].trim_start();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    normalize_short_sha(&value[..end])
}

/// Third step of the commit-sha chain: read `.cargo_vcs_info.json`
/// from the package root (cargo materializes the file beside the
/// manifest when it packs the crate). Returns `None` when the file is
/// missing (ordinary dev builds — the normal case) or unusable.
fn packaged_vcs_sha() -> Option<String> {
    let manifest_dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(manifest_dir.join(".cargo_vcs_info.json")).ok()?;
    parse_vcs_sha_json(&text)
}

/// Shared chain normalizer: a sha is usable when it is 7..=40 ASCII
/// hex characters; the short form is the first 7, lowercased — one
/// canonical shape across all three chain sources. Rejects everything
/// else (`HEAD`, empty, truncated, non-hex noise).
fn normalize_short_sha(s: &str) -> Option<String> {
    let n = s.trim();
    let len = n.len();
    if !(7..=40).contains(&len) || !n.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let short: String = n.chars().take(7).collect();
    Some(short.to_ascii_lowercase())
}

/// Ask the compiler cargo itself drives for its version string
/// ("rustc 1.98.1 (48a229cea 2026-09-01)" shape). "unknown" only when the
/// compiler could not answer `--version` — which could not produce a
/// binary either, so the degraded lane is unreachable in practice.
fn rustc_stamp() -> String {
    let rustc = match std::env::var("RUSTC") {
        Ok(r) => r,
        Err(_) => return "unknown".to_string(),
    };
    let output = match Command::new(rustc).arg("--version").output() {
        Ok(o) => o,
        Err(_) => return "unknown".to_string(),
    };
    if !output.status.success() {
        return "unknown".to_string();
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Read the system clock and render the build time as
/// `M/D/YYYY HH:MM (UTC)`. "unknown" only when the host clock sits
/// before the UNIX epoch (unreachable in practice).
fn format_build_time_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let Ok(secs) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "unknown".to_string();
    };
    let total_secs: i64 = i64::try_from(secs.as_secs()).unwrap_or(0);
    format_unix_secs_as_build_time(total_secs)
}

/// Pure formatting function — takes unix-epoch seconds and returns
/// `M/D/YYYY HH:MM (UTC)`. Separated from `format_build_time_utc` so the
/// algorithm is unit-testable without depending on the wall clock.
///
/// Algorithm: split `total_secs` into days + seconds-of-day, then use
/// Howard Hinnant's `civil_from_days` algorithm
/// (http://howardhinnant.github.io/date_algorithms.html) to convert
/// days-since-epoch to (year, month, day). All arithmetic is on `i64` to
/// avoid unsigned-underflow issues when subtracting the 719468-day shift
/// constant. The runtime twin of this core lives in
/// `src/info/clock.rs` (the daemon log timestamp lane) — the pairing is
/// deliberate: zero time crates on both surfaces.
fn format_unix_secs_as_build_time(total_secs: i64) -> String {
    let days_since_epoch = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;

    let (year, month, day) = civil_from_days(days_since_epoch);

    format!("{month}/{day}/{year} {hour:02}:{minute:02} (UTC)")
}

/// Howard Hinnant's `civil_from_days`: converts days-since-1970-01-01
/// to (year, month, day) in the proleptic Gregorian calendar.
/// http://howardhinnant.github.io/date_algorithms.html#civil_from_days
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32)
}

// The build script's own test suite is NOT executed by `cargo test`
// (cargo never runs build-script tests) — it is a standing contract that
// is verified standalone via:
//   rustc --edition 2024 --test build.rs && ./build
// Every constant below was verified against `date -u -d @<epoch>` at
// authoring time (not from memory) — the same discipline the zelynic
// reference build.rs records.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_time_format_matches_known_unix_epochs() {
        // UNIX epoch: 1970-01-01 00:00:00 UTC.
        assert_eq!(format_unix_secs_as_build_time(0), "1/1/1970 00:00 (UTC)");

        // 2000-01-01 00:00:00 UTC = 946_684_800 seconds since epoch.
        assert_eq!(
            format_unix_secs_as_build_time(946_684_800),
            "1/1/2000 00:00 (UTC)"
        );

        // 2024-02-29 12:34:00 UTC = 1_709_210_040 seconds since epoch.
        // Leap-day boundary check — Feb 29 must not roll to Mar 1.
        assert_eq!(
            format_unix_secs_as_build_time(1_709_210_040),
            "2/29/2024 12:34 (UTC)"
        );

        // 2026-08-04 15:30:00 UTC = 1_785_857_400 seconds since epoch.
        assert_eq!(
            format_unix_secs_as_build_time(1_785_857_400),
            "8/4/2026 15:30 (UTC)"
        );
    }

    #[test]
    fn build_time_format_truncates_sub_minute_seconds() {
        // 1_709_210_440 = 2024-02-29 12:40:40 UTC: the 40 sub-minute
        // seconds are dropped (minute precision), never rounded up.
        assert_eq!(
            format_unix_secs_as_build_time(1_709_210_440),
            "2/29/2024 12:40 (UTC)"
        );
    }

    #[test]
    fn build_time_format_handles_negative_seconds_gracefully() {
        // Pre-epoch timestamps (negative seconds) should still produce
        // a valid proleptic Gregorian date via the algorithm's signed
        // arithmetic, not panic or underflow. -60 = 1969-12-31 23:59 UTC.
        let result = format_unix_secs_as_build_time(-60);
        assert!(
            result.ends_with("(UTC)"),
            "negative-epoch result should still be (UTC)-suffixed: {result}"
        );
        assert!(
            result.contains("1969"),
            "negative-epoch result should land in 1969: {result}"
        );
    }

    #[test]
    fn vcs_sha_json_parses_the_canonical_document() {
        // Byte-for-byte the shape cargo writes when it packs a crate
        // from a dirty tree (the `dirty` flag rides along).
        let doc = r#"{"git":{"sha1":"a1adb97b91fbb32bd5f22bd85b8dbf2870ea53c1","dirty":true},"path_in_vcs":""}"#;
        assert_eq!(parse_vcs_sha_json(doc).as_deref(), Some("a1adb97"));
    }

    #[test]
    fn vcs_sha_json_tolerates_field_order_and_dirty_absence() {
        // Clean publishes omit `dirty`; key order is not contractual.
        // Uppercase hex exercises the shared lowercasing too.
        let doc = r#"{"path_in_vcs":"","git":{"dirty":false,"sha1":"A1ADB97B91FBB32BD5F22BD85B8DBF2870EA53C1"}}"#;
        assert_eq!(parse_vcs_sha_json(doc).as_deref(), Some("a1adb97"));
    }

    #[test]
    fn vcs_sha_json_rejects_unusable_payloads() {
        // The empty document, a missing sha1 key, a truncated sha, and
        // non-hex noise all fall through — the chain's next source (or
        // the "unknown" verdict) answers instead of a garbage stamp.
        for doc in [
            "",
            "{}",
            r#"{"git":{"sha":"a1adb97b91fbb32bd5f22bd85b8dbf2870ea53c1"}}"#,
            r#"{"git":{"sha1":"short"}}"#,
            r#"{"git":{"sha1":"zzzzzzzz91fbb32bd5f22bd85b8dbf2870ea53c1"}}"#,
        ] {
            assert!(
                parse_vcs_sha_json(doc).is_none(),
                "expected rejection for: {doc}"
            );
        }
    }

    #[test]
    fn normalize_short_sha_accepts_the_whole_hex_band() {
        // The 7-char floor, a mid-band 40-hex full sha, uppercase
        // input lowercased, and surrounding whitespace trimmed.
        assert_eq!(normalize_short_sha("a1adb97").as_deref(), Some("a1adb97"));
        assert_eq!(
            normalize_short_sha("A1ADB97B91FBB32BD5F22BD85B8DBF2870EA53C1").as_deref(),
            Some("a1adb97")
        );
        assert_eq!(
            normalize_short_sha("  a1adb97b91fbb32bd5f22bd85b8dbf2870ea53c1\n").as_deref(),
            Some("a1adb97")
        );
    }

    #[test]
    fn normalize_short_sha_rejects_out_of_band_shapes() {
        // Below the 7-char floor, above the 40-hex ceiling, the
        // detached-submodule literal `HEAD`, and hex-adjacent noise.
        for candidate in [
            "",
            "a1adb9",
            "HEAD",
            "a1adb97b91fbb32bd5f22bd85b8dbf2870ea53c1ff",
        ] {
            assert!(
                normalize_short_sha(candidate).is_none(),
                "expected rejection for: {candidate}"
            );
        }
    }
}
