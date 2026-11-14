// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

// curve_anchor_tests.rs — the night-frontier-3C f64 curve-anchor law
// pins, split from mechanical_tests.rs under the 600-line no-mercy
// LOC law (a pure move; a sibling of the mechanical tests module, so
// it re-imports what it needs and reaches the shared model helper
// through the tests module).

use super::tests::model_at_default_profile;
use crate::profiles::{AcousticModel, KeyTrigger, SynthState};

// ── night-frontier-3C: the f64 curve-anchor law ─────────────────────
//
// The law (src/profiles/mechanical/init.rs `anchor`): every curve
// constant init_state bakes is computed in f64 and quantized ONCE to
// f32 (IEEE round-to-nearest). Three facts make the lane real, all
// measured over the shipped domain (the four release rates and dense
// sweeps over the shipped parameter bands):
//
//   1. the anchor sits within HALF an f32 spacing of the f64 truth —
//      the single-quantization guarantee, checked on every family;
//   2. the pre-law f32 chain measurably loses steps where a rounded
//      intermediate feeds a transcendental (the hp coefficient loses
//      up to ~70 spacings at 192 kHz, the pan pair up to ~100 near
//      the extremes — the wobble the law removes is real, the lane
//      is not a re-brand);
//   3. init_state bakes exactly what the law computes — the state
//      fields and the test's own f64 formulas agree bit for bit.
//
// The f64 libm answers are the reference by construction: their own
// error (~2^-52 relative) is four millionths of one f32 spacing.

/// The distance between an f32 value and an f64 truth, in f32
/// spacings (the ULP proxy; every anchor here is positive or signed
/// but compared in magnitude — spacing is scale-local).
fn anchor_gap(value: f32, truth: f64) -> f64 {
    (f64::from(value) - truth).abs()
        / (f64::from(f32::EPSILON) * f64::from(value.abs()).max(f64::MIN_POSITIVE))
}

/// The rates the release matrix ships.
const ANCHOR_RATES: [f32; 4] = [44_100.0, 48_000.0, 96_000.0, 192_000.0];

#[test]
fn night_frontier_3c_f64_anchor_is_correctly_rounded_on_every_family() {
    // Every family's f64-anchor value must sit within 0.55 f32
    // spacings of the f64 truth across the domain (half a spacing is
    // the correctly-rounded ideal; the margin absorbs the f64 libm's
    // own error, which is four millionths of a spacing).
    let rates = ANCHOR_RATES;
    for sr in rates {
        let srf = f64::from(sr);
        // steps: the two time constants, plus a dense tau sweep.
        for i in 0..100 {
            let tau = (0.001_f64) * (0.5 + f64::from(i) / 99.0);
            let v = f64::exp(-1.0 / (srf * tau)) as f32;
            assert!(anchor_gap(v, f64::exp(-1.0 / (srf * tau))) <= 0.55);
            let tau = (0.005 / 3.0) * (0.5 + f64::from(i) / 99.0);
            let v = f64::exp(-1.0 / (srf * tau)) as f32;
            assert!(anchor_gap(v, f64::exp(-1.0 / (srf * tau))) <= 0.55);
        }
        // housing g: the shipped 100-1000 Hz band, dense.
        for i in 0..100 {
            let f = 100.0_f32 + i as f32 * 9.0;
            let v = (std::f64::consts::PI * f64::from(f) / srf).tan() as f32;
            assert!(anchor_gap(v, (std::f64::consts::PI * f64::from(f) / srf).tan()) <= 0.55);
        }
        // hp coefficient, release coefficient, decay norm.
        let w = 2.0 * std::f64::consts::PI * 2000.0 / srf;
        let hp = ((1.0 - w.cos()) / (1.0 + w.cos())) as f32;
        assert!(anchor_gap(hp, (1.0 - w.cos()) / (1.0 + w.cos())) <= 0.55);
        let rel = 0.04_f64.powf(1.0 / (0.002 * srf)) as f32;
        assert!(anchor_gap(rel, 0.04_f64.powf(1.0 / (0.002 * srf))) <= 0.55);
        let dec = f64::from(0.995_f32).powf(f64::from(crate::profiles::SAMPLE_RATE) / srf) as f32;
        assert!(
            anchor_gap(
                dec,
                f64::from(0.995_f32).powf(f64::from(crate::profiles::SAMPLE_RATE) / srf)
            ) <= 0.55
        );
        // pan pair over the stereo band, dense.
        for i in 0..100 {
            let p = -1.0_f32 + i as f32 / 49.5;
            let theta = (f64::from(p) + 1.0) * 0.5 * std::f64::consts::FRAC_PI_2;
            assert!(anchor_gap(theta.cos() as f32, theta.cos()) <= 0.55);
            assert!(anchor_gap(theta.sin() as f32, theta.sin()) <= 0.55);
        }
    }
}

#[test]
fn night_frontier_3c_the_pre_law_f32_chain_measurably_loses_steps() {
    // The wobble the law removes is REAL and localized where a rounded
    // f32 intermediate feeds a transcendental. Pinned at the measured
    // worst points (the probe numbers this test documents):
    //   hp coefficient at 192 kHz: the omega is a f32-rounded ratio
    //   whose cos error lands in a near-zero coefficient — the pre-law
    //   chain loses tens of spacings; the anchor loses none.
    let sr = 192_000.0_f32;
    let w64 = 2.0 * std::f64::consts::PI * 2000.0 / f64::from(sr);
    let truth = (1.0 - w64.cos()) / (1.0 + w64.cos());
    let w32 = 2.0 * std::f32::consts::PI * 2000.0_f32 / sr;
    let c32 = w32.cos();
    let old = (1.0 - c32) / (1.0 + c32);
    assert!(
        anchor_gap(old, truth) > 10.0,
        "the pre-law hp chain must measurably lose steps at 192 kHz: {}",
        anchor_gap(old, truth)
    );
    //   pan pair at the +1 extreme (theta -> pi/2, cos -> 0): the
    //   f32 theta's rounding lands in a near-zero cos — measured
    //   6413 spacings at p = 1 - 1/8192 (and ~8.4M at the exact
    //   extreme, where the f32 theta rounds onto pi/2 itself); the
    //   anchor never exceeds half a spacing.
    let p = 1.0_f32 - 1.0 / 8192.0;
    let theta64 = (f64::from(p) + 1.0) * 0.5 * std::f64::consts::FRAC_PI_2;
    let theta32 = (p + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
    assert!(anchor_gap(theta32.cos(), theta64.cos()) > 1000.0);
    //   housing g across the band: measured at 0.95-1.08 spacings
    //   across the four rates — the f32 tan was already nearly
    //   correctly rounded; the anchor still never exceeds half.
    let mut old_worst = 0.0_f64;
    for i in 0..100 {
        let f = 100.0_f32 + i as f32 * 9.0;
        let truth = (std::f64::consts::PI * f64::from(f) / f64::from(sr)).tan();
        let oldv = (std::f32::consts::PI * f / sr).tan();
        old_worst = old_worst.max(anchor_gap(oldv, truth));
    }
    assert!(old_worst > 0.55, "housing g pre-law worst gap: {old_worst}");
    // The recorded honest limit, NOT asserted as a universal law: on
    // the A/B machine's glibc the exp-step and pure-pow families
    // measured at or below one spacing pre-law on this domain — the
    // law gives them one consistent rule, not accuracy there. Miri's
    // own libm then measured the pre-law f32 powf at one-plus spacing
    // on the same expression, which is exactly the point: the pre-law
    // upper bound is a libm property, while the lane's universal
    // guarantee (the new anchor within half a spacing of the f64
    // truth) held under both libms. The Miri measurement is recorded
    // in docs/ZACTRIX_AUDIT.md next to the glibc one; asserting
    // either number as a law would make this suite lie on the other
    // platform.
}

#[test]
fn night_frontier_3c_init_bakes_the_law_the_tests_measured() {
    // The binding check: the real init path's state fields equal the
    // test's own f64 formulas, bit for bit, for every family whose
    // inputs init_state receives unchanged (the steps, the hp and
    // release/decay coefficients, the pan pair — the housing g rides
    // the drifted frequency, judged by the dominance tests above).
    let model = model_at_default_profile();
    let profile = model.get_profile(&KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.375,
        velocity: None,
    });
    let mut state = SynthState::default();
    model.init_state(&profile, &mut state, 0.375, None);

    let sr: u32 = 44_100;
    let srf = f64::from(sr);
    assert_eq!(
        state.click_step.to_bits(),
        (f64::exp(-1.0 / (srf * 0.001)) as f32).to_bits()
    );
    assert_eq!(
        state.spring_step.to_bits(),
        (f64::exp(-1.0 / (srf * (0.005 / 3.0))) as f32).to_bits()
    );
    let w = 2.0 * std::f64::consts::PI * 2000.0 / srf;
    assert_eq!(
        state.ambient_hp_coeff.to_bits(),
        (((1.0 - w.cos()) / (1.0 + w.cos())) as f32).to_bits()
    );
    let rel = 0.04_f64.powf(1.0 / (0.002 * srf)).clamp(0.0, 0.9999);
    assert_eq!(state.release_coeff.to_bits(), (rel as f32).to_bits());
    let theta = (f64::from(0.375_f32) + 1.0) * 0.5 * std::f64::consts::FRAC_PI_2;
    assert_eq!(state.pan_left.to_bits(), (theta.cos() as f32).to_bits());
    assert_eq!(state.pan_right.to_bits(), (theta.sin() as f32).to_bits());
}
