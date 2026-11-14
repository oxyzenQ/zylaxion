// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The build stamp's reproducibility helpers (night-frontier-2B):
//! the epoch law (SOURCE_DATE_EPOCH honored, the wall clock the
//! documented fallback), the lockfile attestation (FNV-1a 64 over
//! the workspace Cargo.lock), and the pure unix-seconds -> stamp
//! formatter. Pure move out of build.rs (the 600-line LOC law); the
//! standalone suite (rustc --edition 2024 --test build.rs) still
//! compiles and runs these tests through the build.rs include.

/// The reproducible-build epoch law (night-frontier-2B): the build
/// timestamp honors `SOURCE_DATE_EPOCH` — the reproducible-builds
/// standard's own variable — when the environment carries a valid
/// value, and falls back to the wall clock otherwise. Returns the
/// seconds AND the clock that answered, so the stamp can name its
/// own source; a machine that pins the epoch renders the same stamp
/// bytes as any other machine pinning it.
pub(super) fn build_epoch_secs() -> (i64, &'static str) {
    if let Some(epoch) = source_date_epoch() {
        return (epoch, "SOURCE_DATE_EPOCH");
    }
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
        .unwrap_or(0);
    (secs, "wall-clock")
}

/// The `SOURCE_DATE_EPOCH` parse (the reproducible-builds contract,
/// the strict reading): decimal digits only — no sign, no spaces, no
/// empty value. An absent or malformed value answers `None` and the
/// wall clock takes over, exactly as the standard prescribes: a
/// malformed pin must never silently shift the date, it must be
/// visibly ignored.
fn source_date_epoch() -> Option<i64> {
    let raw = std::env::var_os("SOURCE_DATE_EPOCH")?;
    let text = raw.to_str()?;
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok()
}

/// The lockfile attestation (night-frontier-2B): reads the workspace
/// `Cargo.lock` and fingerprints it — FNV-1a 64, the same primitive
/// the audit estate already rides — returning the 16-hex stamp.
/// `None` only when the file is absent (a hand-pruned tree), which
/// the consumer reports as "unknown" instead of inventing a value.
pub(super) fn lockfile_hash() -> Option<String> {
    let bytes = std::fs::read("Cargo.lock").ok()?;
    Some(fnv1a64(&bytes))
}

/// FNV-1a 64 over raw bytes — the byte-stream twin of the audit
/// harness's `fnv1a_f32` (same offset/prime constants, std only, no
/// crate in the build script).
fn fnv1a64(bytes: &[u8]) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

/// Pure formatting function — takes unix-epoch seconds and returns
/// `YYYY-MM-DD HH:MMZ` (ISO 8601, UTC; night-pro-1 aligns the stamp
/// with the flagship's format, replacing the US-ambiguous
/// `M/D/YYYY HH:MM (UTC)` shape). Separated from `format_build_time_utc` so the
/// algorithm is unit-testable without depending on the wall clock.
///
/// Algorithm: split `total_secs` into days + seconds-of-day, then use
/// Howard Hinnant's `civil_from_days` algorithm
/// (http://howardhinnant.github.io/date_algorithms.html) to convert
/// days-since-epoch to (year, month, day). All arithmetic is on `i64` to
/// avoid unsigned-underflow issues when subtracting the 719468-day shift
/// constant. The runtime twin of this core lives in
/// `src/info/clock.rs` (the log timestamp lane) — the pairing is
/// deliberate: zero time crates on both surfaces.
pub(super) fn format_unix_secs_as_build_time(total_secs: i64) -> String {
    let days_since_epoch = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;

    let (year, month, day) = civil_from_days(days_since_epoch);

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}Z")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_time_format_matches_known_unix_epochs() {
        // UNIX epoch: 1970-01-01 00:00:00 UTC.
        assert_eq!(format_unix_secs_as_build_time(0), "1970-01-01 00:00Z");

        // 2000-01-01 00:00:00 UTC = 946_684_800 seconds since epoch.
        assert_eq!(
            format_unix_secs_as_build_time(946_684_800),
            "2000-01-01 00:00Z"
        );

        // 2024-02-29 12:34:00 UTC = 1_709_210_040 seconds since epoch.
        // Leap-day boundary check — Feb 29 must not roll to Mar 1.
        assert_eq!(
            format_unix_secs_as_build_time(1_709_210_040),
            "2024-02-29 12:34Z"
        );

        // 2026-08-04 15:30:00 UTC = 1_785_857_400 seconds since epoch.
        assert_eq!(
            format_unix_secs_as_build_time(1_785_857_400),
            "2026-08-04 15:30Z"
        );
    }

    #[test]
    fn build_time_format_truncates_sub_minute_seconds() {
        // 1_709_210_440 = 2024-02-29 12:40:40 UTC: the 40 sub-minute
        // seconds are dropped (minute precision), never rounded up.
        assert_eq!(
            format_unix_secs_as_build_time(1_709_210_440),
            "2024-02-29 12:40Z"
        );
    }

    #[test]
    fn build_time_format_handles_negative_seconds_gracefully() {
        // Pre-epoch timestamps (negative seconds) should still produce
        // a valid proleptic Gregorian date via the algorithm's signed
        // arithmetic, not panic or underflow. -60 = 1969-12-31 23:59 UTC.
        let result = format_unix_secs_as_build_time(-60);
        assert!(
            result.ends_with('Z'),
            "negative-epoch result should still be Z-suffixed: {result}"
        );
        assert!(
            result.contains("1969"),
            "negative-epoch result should land in 1969: {result}"
        );
    }

    #[test]
    fn source_date_epoch_parses_decimal_digits_only() {
        // The vector rides the build through env-var injection, and
        // this suite runs with a neutral environment, so the parse
        // helper is exercised through a temporary override.
        // SAFETY: single-threaded test binary, no other reader of the
        // variable in this process.
        unsafe { std::env::set_var("SOURCE_DATE_EPOCH", "1_785_857_400") };
        assert_eq!(source_date_epoch(), None);
        unsafe { std::env::set_var("SOURCE_DATE_EPOCH", "1785857400") };
        assert_eq!(source_date_epoch(), Some(1_785_857_400));
        // The strict reading: a sign, a space, an empty value — the
        // standard's malformed-input law is "ignore, never shift the
        // date".
        for hostile in ["-1", " 1785857400", "1785857400 ", "", "+5"] {
            unsafe { std::env::set_var("SOURCE_DATE_EPOCH", hostile) };
            assert_eq!(source_date_epoch(), None, "hostile value: {hostile}");
        }
        // A value past i64 is malformed by the same law.
        unsafe { std::env::set_var("SOURCE_DATE_EPOCH", "9".repeat(30)) };
        assert_eq!(source_date_epoch(), None);
        unsafe { std::env::remove_var("SOURCE_DATE_EPOCH") };
        assert_eq!(source_date_epoch(), None);
    }

    #[test]
    fn fnv1a64_matches_the_reference_vectors() {
        // FNV-1a 64 test vectors computed with the reference algorithm
        // (offset 0xcbf29ce484222325, prime 0x100000001b3) — the
        // empty-bytes identity and three short literals.
        assert_eq!(fnv1a64(b""), "cbf29ce484222325");
        assert_eq!(fnv1a64(b"a"), "af63dc4c8601ec8c");
        assert_eq!(fnv1a64(b"Cargo"), "6f6097a17b8e18c7");
        assert_eq!(fnv1a64(b"lock"), "ce14f5ad70f6ece2");
    }

    #[test]
    fn lockfile_hash_answers_the_workspace_lock() {
        // The real tree's Cargo.lock sits beside this build script —
        // the stamp must resolve against it (16 lowercase hex) and be
        // stable across two reads of the same bytes.
        let first = lockfile_hash().expect("the workspace carries Cargo.lock");
        let second = lockfile_hash().expect("the file did not move mid-test");
        assert_eq!(first, second, "one file, one fingerprint");
        assert_eq!(first.len(), 16, "FNV-1a 64 renders as 16 hex chars");
        assert!(
            first
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
    }
}
