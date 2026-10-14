// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! PROOF 2 — the sample-rate span (night-private-research-2xblade-3).
//! One keystroke per preset at 44.1 / 48 / 88.2 / 96 / 192 kHz, through
//! the real VoicePool render path. Three measured dimensions per
//! preset:
//!
//! (a) the voice lifetime spread across rates — the c^(R/S) law's
//!     EXACT measurand (the envelope retention chain is powf-anchored;
//!     the stability battery rode the same measurand when the
//!     composite ring window had too few clean fits). Target: < 0.5%.
//! (b) tail length spread across rates — the -60 dB end point. The
//!     2xblade-2 stability battery already established, with a control
//!     experiment, that this endpoint is excitation statistics (a
//!     noise-driven resonator's extracted energy scales with its
//!     normalized bandwidth fc/(Q*Fs)), present at the shipped values,
//!     unchanged by the contract expansion, and not a law measurand.
//!     This mode measures it anyway and records it — the number is
//!     real, the interpretation is in docs/ZACTRIX_AUDIT.md. The
//!     composite ring fit is also recorded where the fixed window
//!     yields its 32 maxima; at the shipped values the ring is short
//!     and high rates often fall under the fit floor — never invented.
//! (c) output hash stability per rate — the same seed plus the same
//!     rate must produce the same bytes. Each render is performed
//!     twice (fresh model instance, counter reset, fresh pool) and the
//!     FNV-1a hashes must match in-run; the wrapper script runs the
//!     whole mode twice and diffs the outputs for the cross-run proof.

use crate::profiles::registry_all;
use crate::proof::{
    fnv1a_f32, raw_voice_lifetime_ms, render_mono_pool, ring_decay_fit, tail_end_ms,
};

/// The five rates of the LTS span.
const PROOF_RATES: [u32; 5] = [44_100, 48_000, 88_200, 96_000, 192_000];

/// Render window, seconds at every rate: four seconds covers the
/// longest shipped tail (the ambient class rings ~3.4 s) plus margin.
const PROOF2_WINDOW_S: usize = 4;

/// The law target, percent, verbatim from the pass contract.
const LAW_TARGET_PCT: f64 = 0.5;

/// One per-preset, per-rate measurement.
struct RateProbe {
    /// Voice lifetime in milliseconds (the live output length): the
    /// c^(R/S) law's exact measurand — the envelope retention chain is
    /// powf-anchored, so the LIFETIME must be rate-invariant to f32
    /// rounding.
    voice_ms: f64,
    /// Composite ring decay, dB/s, in the fixed [-25, -50] dB window.
    /// Secondary: recorded where the fit exists, never invented.
    decay_db_s: f64,
    tail_ms: f64,
    hash: u64,
    rehash: u64,
}

fn render_rate_probe(profile: crate::profiles::KeyProfile, rate: u32) -> RateProbe {
    let samples = PROOF2_WINDOW_S * rate as usize;
    let mono = render_mono_pool(profile, rate, samples);
    // Live length of the pool stream (for the fit window bound only).
    let live = mono
        .iter()
        .rposition(|&v| v.abs() > 0.0)
        .map(|i| i + 1)
        .unwrap_or(0);
    RateProbe {
        // The law measurand: the RAW envelope-chain lifetime (see
        // proof::raw_voice_lifetime_ms — the pool stream's last nonzero
        // sample conflates the ring's denormal crossing with the law).
        voice_ms: raw_voice_lifetime_ms(profile, rate, samples),
        decay_db_s: ring_decay_fit(&mono, rate, live),
        tail_ms: tail_end_ms(&mono, rate),
        hash: fnv1a_f32(0xcbf2_9ce4_8422_2325, &mono),
        // In-run determinism: a second full render from fresh state.
        rehash: {
            let again = render_mono_pool(profile, rate, samples);
            fnv1a_f32(0xcbf2_9ce4_8422_2325, &again)
        },
    }
}

pub(super) fn run_rates() {
    println!(
        "PROOF 2 (night-private-research-2xblade-3): sample-rate span, one keystroke per preset, window {} s, real VoicePool path",
        PROOF2_WINDOW_S
    );
    println!(
        "law measurand: the voice lifetime (the exact measurand, the shape the stability battery rides) gated at < {LAW_TARGET_PCT}% across rates; the composite ring fit is recorded where the fixed window yields its maxima; the tail endpoint is recorded as excitation statistics (docs/ZACTRIX_AUDIT.md control experiment)"
    );
    println!(
        "| preset | rate kHz | voice ms | ring decay dB/s | tail ms | hash (fnv1a64) | re-render hash equal |"
    );
    println!("|---|---|---|---|---|---|---|");

    let presets = registry_all();
    let mut law_fails: Vec<String> = Vec::new();
    let mut hash_fails: Vec<String> = Vec::new();

    for preset in presets {
        let mut lives = [0.0_f64; 5];
        let mut decays = [0.0_f64; 5];
        let mut tails = [0.0_f64; 5];
        let mut hashes = [0_u64; 5];
        for (ri, &rate) in PROOF_RATES.iter().enumerate() {
            let p = render_rate_probe(preset.profile, rate);
            lives[ri] = p.voice_ms;
            decays[ri] = p.decay_db_s;
            tails[ri] = p.tail_ms;
            hashes[ri] = p.hash;
            let rehash_ok = p.hash == p.rehash;
            if !rehash_ok {
                hash_fails.push(format!("{}@{}k (in-run)", preset.name, rate / 1000));
            }
            println!(
                "| `{}` | {} | {:.2} | {:.2} | {:.2} | {:016x} | {} |",
                preset.name,
                rate / 1000,
                p.voice_ms,
                p.decay_db_s,
                p.tail_ms,
                p.hash,
                if rehash_ok { "yes" } else { "NO" },
            );
        }
        // The law measurand: voice lifetime spread across rates.
        let lmin = lives.iter().cloned().fold(f64::INFINITY, f64::min);
        let lmax = lives.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let life_spread = if lmin > 0.0 {
            (lmax - lmin) / lmin * 100.0
        } else {
            f64::INFINITY
        };
        // The composite ring fit: recorded where it exists.
        let fit_points = decays.iter().filter(|&&d| d > 0.0).count();
        let dmin = decays.iter().cloned().fold(f64::INFINITY, f64::min);
        let dmax = decays.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let decay_spread = if dmin > 0.0 {
            (dmax - dmin) / dmin * 100.0
        } else {
            f64::INFINITY
        };
        let tmin = tails.iter().cloned().fold(f64::INFINITY, f64::min);
        let tmax = tails.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let tail_spread = if tmin > 0.0 {
            (tmax - tmin) / tmin * 100.0
        } else {
            f64::INFINITY
        };
        let law_ok = life_spread < LAW_TARGET_PCT;
        if !law_ok {
            law_fails.push(format!(
                "{} (voice lifetime spread {life_spread:.4}%)",
                preset.name
            ));
        }
        let fit_note = if fit_points >= 3 {
            format!(
                "composite ring fit {fit_points}/5 rates, spread {:.4}%",
                if decay_spread.is_finite() {
                    decay_spread
                } else {
                    f64::NAN
                }
            )
        } else {
            format!(
                "composite ring fit {fit_points}/5 rates (< 3 clean fits: the shipped ring is short against the fixed window at high rates - the lifetime is the exact measurand)"
            )
        };
        println!(
            "| `{}` | spread | lifetime {:.2}-{:.2} ms, spread {life_spread:.4}% ({}; target < {}) | {fit_note} | tail endpoint {:.2}-{:.2} ms, spread {:.2}% (recorded: excitation statistics, not a law measurand) | - | - |",
            preset.name,
            lmin,
            lmax,
            if law_ok { "law PASS" } else { "law FAIL" },
            LAW_TARGET_PCT,
            tmin,
            tmax,
            if tail_spread.is_finite() {
                tail_spread
            } else {
                f64::NAN
            },
        );
        println!();
    }

    println!(
        "PROOF 2 verdict: voice-lifetime law failures: {} of {} presets; in-run re-render hash failures: {}",
        law_fails.len(),
        presets.len(),
        hash_fails.len()
    );
    for f in &law_fails {
        println!("  law FAIL: {f}");
    }
    for f in &hash_fails {
        println!("  hash FAIL: {f}");
    }
    if law_fails.is_empty() && hash_fails.is_empty() {
        println!(
            "PROOF 2 PASS: the c^(R/S) law's exact measurand (the voice lifetime) holds across 44.1/48/88.2/96/192 kHz within {LAW_TARGET_PCT}% on every preset, and every per-rate render is byte-stable in-run (the wrapper's second run must also diff clean for the cross-run proof)"
        );
    }
}
