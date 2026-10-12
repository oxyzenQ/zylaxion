// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Runtime UTC clock lane — the daemon log timestamp formatter.
//!
//! The zelynic clock contract (night-improve-13): any wall-clock need is
//! answered by the std clock plus Howard Hinnant's `civil_from_days`
//! algorithm (<http://howardhinnant.github.io/date_algorithms.html>) —
//! never by a time crate. env_logger's own timestamp lane (the
//! `humantime` feature) transitively pulls the jiff crate tree, which is
//! the exact dependency bloat this module exists to refuse: the logger
//! formats its stamps through here instead.
//!
//! The compile-time twin of this core lives in `build.rs`
//! (`ZYLAXION_BUILD_TIME` — the `M/D/YYYY HH:MM (UTC)` build stamp). The
//! pairing is deliberate: zero time crates on both surfaces.
//!
//! Shape: `2026-10-11T07:41:44.284Z` — the same millisecond-precision UTC
//! instant env_logger's timestamp lane rendered, so journal consumers and
//! log parsers keep their contract.

/// The current UTC instant in the log stamp shape. "unknown" only when
/// the host clock sits before the UNIX epoch (unreachable in practice).
pub(crate) fn iso_utc_millis() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let Ok(d) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "unknown".to_string();
    };
    let secs = i64::try_from(d.as_secs()).unwrap_or(0);
    iso_from_unix_secs(secs, d.subsec_millis())
}

/// Pure formatter — unix-epoch seconds + sub-second millis to
/// `YYYY-MM-DDTHH:MM:SS.mmmZ`. Separated from the clock read so the
/// algorithm is unit-testable without depending on the wall clock.
pub(crate) fn iso_from_unix_secs(total_secs: i64, millis: u32) -> String {
    let days_since_epoch = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;

    let (year, month, day) = civil_from_days(days_since_epoch);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Howard Hinnant's `civil_from_days`: converts days-since-1970-01-01
/// to (year, month, day) in the proleptic Gregorian calendar. All
/// arithmetic is on `i64` — the signed core keeps pre-epoch inputs
/// well-defined instead of panicking or underflowing.
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
#[path = "../../test/info/clock_tests.rs"]
mod tests;
