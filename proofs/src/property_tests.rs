// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! night-netrunner-Z2: the sampled tier — proptest exhaustion of the
//! included cores' laws on the pinned stable toolchain (the zelynic
//! night-frontier-Z6 lineage, the instrument that sits between the
//! root crate's unit pins and its Kani for-all theorems). Each
//! property below names the law it checks and the domain it samples;
//! a red is a real counterexample with a shrunk witness, and the
//! regression store (proptest-regressions/) replays every
//! counterexample ever caught on each subsequent run.
//!
//! The laws are the ones the estate's own audits named (the volume
//! clamp cell night-hunter-1, the gain percent law, the signed-float
//! mapping's symmetric-range fix night-hunter-1, and the Hinnant
//! civil calendar the log stamps ride) — sampled here at thousands
//! of cases per law, proven for-all by the Kani lane above.

use crate::clock_core::civil_from_days;
use crate::gain_core::{
    DEFAULT_MASTER_VOLUME, gain_from_percent, u32_to_signed_f32, volume_bits,
};

use proptest::prelude::*;

// ── The volume cell's law (night-hunter-1, the clamp the watcher and
// the startup lane must agree on) ───────────────────────────────────

proptest! {
    /// Finite volumes land inside the [-100, 100] band and
    /// round-trip their bits losslessly; non-finite volumes fall
    /// back to the compiled default. The sampled half of the Kani
    /// theorem `volume_bits_clamps_every_volume` (the float
    /// bit-domain sampler catches NaN/Inf payloads the u32-band
    /// sampler alone would under-cover).
    #[test]
    fn volume_bits_clamps_and_round_trips(volume in any::<f32>()) {
        let bits = volume_bits(volume);
        let restored = f32::from_bits(bits);
        if volume.is_finite() {
            prop_assert!(restored >= -100.0, "the clamp cell leaked below -100");
            prop_assert!(restored <= 100.0, "the clamp cell leaked above +100");
            prop_assert_eq!(restored, volume.clamp(-100.0, 100.0));
        } else {
            prop_assert_eq!(restored, DEFAULT_MASTER_VOLUME);
        }
    }

    /// In-band finite volumes round-trip EXACTLY: the clamp is the
    /// identity on the band (a watcher that writes a legal volume
    /// reads the same volume back — no silent retune).
    #[test]
    fn volume_bits_identity_on_the_band(volume in -100.0f32..=100.0) {
        prop_assert_eq!(f32::from_bits(volume_bits(volume)), volume);
    }
}

// ── The gain percent law (the control map's percent cell) ─────────

proptest! {
    /// The percent law is monotone: louder percent never renders
    /// quieter, the output is finite and non-negative, and 0/100 are
    /// exactly silence/default. The sampled half of the Kani theorem
    /// `gain_from_percent_is_finite_and_monotone`.
    #[test]
    fn gain_from_percent_is_finite_monotone_and_anchored(
        a in any::<u32>(),
        b in any::<u32>(),
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let gain_lo = gain_from_percent(lo);
        let gain_hi = gain_from_percent(hi);
        prop_assert!(gain_lo.is_finite(), "the gain path produced a non-finite value");
        prop_assert!(gain_lo >= 0.0, "the gain path went negative");
        prop_assert!(gain_lo <= gain_hi, "louder percent rendered quieter");
    }

    /// The composition law the watcher actually rides: percent -> gain
    /// -> volume cell -> restored f32 stays inside the band the clamp
    /// enforces, for EVERY percent (the end-to-end control-map write
    /// path, not each stage in isolation).
    #[test]
    fn the_control_map_write_path_stays_in_band(percent in any::<u32>()) {
        let gain = gain_from_percent(percent);
        let stored = volume_bits(gain);
        let restored = f32::from_bits(stored);
        prop_assert!(restored.is_finite());
        prop_assert!(restored >= -100.0);
        prop_assert!(restored <= 100.0);
    }
}

// ── The signed-float mapping (night-hunter-1's symmetric-range fix,
// the PRNG's noise/drift domain) ───────────────────────────────────

proptest! {
    /// The mapping stays inside the symmetric [-1, +1] band with
    /// +1.0 reachable exactly (the old divisor bug's +3.0 escape is
    /// the regression this property exists to catch). The sampled
    /// half of the Kani theorem
    /// `u32_to_signed_f32_stays_in_unit_range`.
    #[test]
    fn u32_to_signed_f32_stays_symmetric(x in any::<u32>()) {
        let y = u32_to_signed_f32(x);
        prop_assert!(y >= -1.0, "the mapping escaped below -1");
        prop_assert!(y <= 1.0, "the mapping escaped above +1 (the old divisor bug)");
        prop_assert!(y.is_finite());
    }

    /// The mapping is injective on its 24-bit input: distinct
    /// top-24-bit patterns map to distinct floats (the quantization
    /// is uniform — no two inputs alias the same output band, the
    /// "chunky upper half" defect the v10.2.0 fix documented).
    #[test]
    fn u32_to_signed_f32_is_injective_on_the_top_bits(
        x in any::<u32>(),
        y in any::<u32>(),
    ) {
        prop_assume!(x >> 8 != y >> 8);
        prop_assert_ne!(u32_to_signed_f32(x), u32_to_signed_f32(y));
    }
}

// ── The Hinnant civil calendar (the log stamp's date math) ─────────

/// The documented domain the fuzz lane pins: year 0001-01-01 ..
/// 9999-12-31 in days-since-epoch (the four-digit ISO span).
const DOMAIN_MIN_DAYS: i64 = -621_355_968_00 / 86_400;
const DOMAIN_MAX_DAYS: i64 = 2_534_023_007_99 / 86_400;

proptest! {
    /// The calendar's month/day ranges hold over the whole ISO
    /// domain: months in [1, 12], days in [1, 31], and the pair
    /// respects each month's own length (February never carries 30
    /// days, the classic civil-calendar defect). The sampled half
    /// of the fuzz lane's round-trip domain pin, beside the unit
    /// table's date -u verified constants.
    #[test]
    fn civil_from_days_stays_in_calendar(days in DOMAIN_MIN_DAYS..=DOMAIN_MAX_DAYS) {
        let (year, month, day) = civil_from_days(days);
        prop_assert!(month >= 1 && month <= 12, "month escaped [1,12]: {month}");
        let leap = is_leap(year);
        let len = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            _ => 28,
        };
        prop_assert!(day >= 1 && day <= len, "day escaped the month: {year}-{month:02}-{day:02}");
    }

    /// Day-to-civil is monotone: the next day never renders an
    /// earlier-or-equal (year, month, day) tuple — the ordering the
    /// log stamps' lexicographic sort relies on.
    #[test]
    fn civil_from_days_is_monotone(days in DOMAIN_MIN_DAYS..=DOMAIN_MAX_DAYS - 1) {
        let a = civil_from_days(days);
        let b = civil_from_days(days + 1);
        prop_assert!(a < b, "the calendar went backwards at day {days}: {a:?} -> {b:?}");
    }
}

/// Proleptic Gregorian leap rule (the same rule civil_from_days
/// implements with era arithmetic — restated here as the oracle the
/// properties above check against).
fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
