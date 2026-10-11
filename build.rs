// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Build script — stamps the compile-time facts the binary reports about
//! itself:
//!
//! - `GIT_HASH` — the short commit hash the crate was built from
//!   (consumed by the version report's `Build:` line).
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
//!
//! Every fallback degrades gracefully — a missing fact renders as
//! "unknown", never a failed build.

use std::process::Command;

fn main() {
    // Re-run this script whenever HEAD (or the work tree state) changes so
    // the embedded hash stays in sync with what was actually compiled.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs");

    let hash = match git_short_hash() {
        Some(h) => h,
        None => "unknown".to_string(),
    };

    println!("cargo:rustc-env=GIT_HASH={}", hash);

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
}

/// Runs `git rev-parse --short HEAD` from the workspace root and returns
/// the trimmed short hash on success.
fn git_short_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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
//   rustc --edition 2024 --test crates/zylaxion/build.rs && ./build
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
}
