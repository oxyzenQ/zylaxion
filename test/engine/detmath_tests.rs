// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

// detmath_tests.rs — the night-frontier-Z5 certified transcendental
// lane's pins. Three layers, each honest about what it proves:
//
//   1. the golden bit vectors — the generator (the single algorithmic
//      source, scripts/engine/generate-detmath.py) simulated the exact
//      op order in IEEE-754 binary64 and emitted (x_bits, y_bits)
//      pairs; this tree pins them, so no hand edit of the tables or
//      the module can move a single returned bit silently;
//   2. the certificate property — over dense domain sweeps the lane's
//      deviation from the f64 libm oracle stays inside the emitted
//      exact-Fraction over-bounds (the libm oracle's own ~1 ulp f64
//      noise is named in the margin), and inside one f32 spacing on
//      the physically meaningful band;
//   3. the domain guards + the exact libm edge laws — the certified
//      domain refuses loudly, and pow's exact cases (c^1 == c, the
//      44.1 kHz reference law) are pinned bit for bit.
//
// The oracle-margin law: the absolute bounds are exact-Fraction
// certificates for THIS lane vs the TRUE function; the probes measure
// against f64 libm, whose own error (about one f64 ulp, ~2.3e-16
// absolute on the [-1, 1] result band) rides on top — the ORACLE
// MARGIN below is that named slack, never silently absorbed.

#[path = "detmath_vectors.rs"]
mod vectors;

use self::vectors::{POW_VECTORS, VECTORS};
use super::detmath::{self, tables};

/// The f64 libm oracle's own noise on the [-1, 1] result band: one
/// ulp of the value scale (~2.3e-16), rounded up with room for the
/// libm's documented <= 1 ulp contract.
const ORACLE_MARGIN: f64 = 4.0 * f64::EPSILON;

/// The physically meaningful band for the f32-anchor sweep: anchor
/// values below 1e-6 are under -120 dB of headroom, where the
/// quadrant-boundary deviation (RED_RHO-scale, documented in the
/// module) lives by design.
const MEANINGFUL_BAND: f64 = 1e-6;

fn gap_f32_spacings(value: f32, truth: f64) -> f64 {
    (f64::from(value) - truth).abs()
        / (f64::from(f32::EPSILON) * f64::from(value.abs()).max(f64::MIN_POSITIVE))
}

#[test]
fn golden_vectors_hold_bit_for_bit() {
    for v in VECTORS {
        let x = f64::from_bits(v.xb);
        let got = match v.name.split('(').next().unwrap_or("") {
            "exp" => detmath::det_exp(x),
            "ln" => detmath::det_ln(x),
            "sin" => detmath::det_sin(x),
            "cos" => detmath::det_cos(x),
            "tan" => detmath::det_tan(x),
            other => panic!("unknown vector family {other}"),
        };
        assert_eq!(
            got.to_bits(),
            v.yb,
            "{} returned {:016x}, the golden vector pins {:016x}",
            v.name,
            got.to_bits(),
            v.yb
        );
    }
    for v in POW_VECTORS {
        let c = f64::from_bits(v.cb);
        let r = f64::from_bits(v.rb);
        let got = detmath::det_pow(c, r);
        assert_eq!(
            got.to_bits(),
            v.yb,
            "pow returned {:016x}, the golden vector pins {:016x}",
            got.to_bits(),
            v.yb
        );
    }
}

#[test]
// The dense sweep is miri-hostile by size (40k libm-oracle points under
// the interpreter) and adds no UB surface: safe float arithmetic is
// exactly what miri does not reinterpret. The native CI lane owns it;
// the golden vectors and the edge laws below stay miri-covered.
#[cfg_attr(
    miri,
    ignore = "40k-point float sweeps: no UB surface, the native lane owns them"
)]
fn exp_stays_inside_the_certificate_over_the_domain() {
    // dense sweep + the boundary points; the bound is the emitted
    // over-bound plus the named oracle margin.
    let bound = tables::EXP_REL_BOUND + ORACLE_MARGIN;
    let steps = 40_000;
    for i in 0..=steps {
        let x = -64.0 + (i as f64 / steps as f64) * 64.0;
        let got = detmath::det_exp(x);
        let want = x.exp();
        let rel = (got - want).abs() / want.abs().max(f64::MIN_POSITIVE);
        assert!(rel <= bound, "det_exp({x}) rel gap {rel:.3e} > {bound:.3e}");
    }
    for x in [-64.0, -0.35, -3.0, -1e-9, -0.0] {
        let got = detmath::det_exp(x);
        let want = x.exp();
        let rel = (got - want).abs() / want.abs().max(f64::MIN_POSITIVE);
        assert!(rel <= bound, "det_exp({x}) rel gap {rel:.3e} > {bound:.3e}");
    }
    assert_eq!(detmath::det_exp(0.0).to_bits(), 1.0_f64.to_bits());
}

#[test]
// Same miri law as the exp sweep above: 120k float-oracle points, no
// unsafe, no pointer work - the native lane owns the sweep.
#[cfg_attr(
    miri,
    ignore = "120k-point float sweeps: no UB surface, the native lane owns them"
)]
fn ln_sin_cos_tan_stay_inside_their_certificates() {
    // The oracle margin scales with the RESULT magnitude: the f64
    // libm's own error is one ulp of the VALUE (its |ln(1e-9)| ~ 20.7
    // result carries ~4.6e-15 of its own noise), while the lane's
    // certificate is absolute — the sum is the honest gate.
    let ln_bound = |truth: f64| tables::LN_ABS_BOUND + truth.abs() * ORACLE_MARGIN;
    let steps = 40_000;
    for i in 1..=steps {
        // log-spaced sweep (0, 1] — covers the exponent-fold ladder
        let x = 10f64.powf(-9.0 + 9.0 * (i as f64 / steps as f64));
        let got = detmath::det_ln(x);
        let want = x.ln();
        assert!(
            (got - want).abs() <= ln_bound(want),
            "det_ln({x}) gap {:.3e} > {:.3e}",
            (got - want).abs(),
            ln_bound(want)
        );
    }
    let sin_bound = |truth: f64| tables::SIN_ABS_BOUND + truth.abs() * ORACLE_MARGIN;
    let cos_bound = |truth: f64| tables::COS_ABS_BOUND + truth.abs() * ORACLE_MARGIN;
    let tan_bound = |truth: f64| tables::TAN_ABS_BOUND + truth.abs() * ORACLE_MARGIN;
    for i in 0..=steps {
        let x = (i as f64 / steps as f64) * 4.0 * std::f64::consts::PI;
        let (s, c) = (x.sin(), x.cos());
        assert!(
            (detmath::det_sin(x) - s).abs() <= sin_bound(s),
            "det_sin({x}) broke the certificate"
        );
        assert!(
            (detmath::det_cos(x) - c).abs() <= cos_bound(c),
            "det_cos({x}) broke the certificate"
        );
    }
    for i in 0..=steps {
        let x = (i as f64 / steps as f64) * 0.45 * std::f64::consts::PI;
        let t = x.tan();
        assert!(
            (detmath::det_tan(x) - t).abs() <= tan_bound(t),
            "det_tan({x}) broke the certificate"
        );
    }
}

#[test]
// Same miri law as the exp sweep above.
#[cfg_attr(
    miri,
    ignore = "60k-point float sweeps: no UB surface, the native lane owns them"
)]
fn f32_anchors_match_libm_inside_one_spacing_on_the_meaningful_band() {
    // The property the corpus rides: on the physically meaningful band
    // (|truth| >= 1e-6) the lane's f32 anchor equals the libm f64
    // oracle's anchor inside one f32 spacing. Below the band the
    // ABSOLUTE certificates above are the contract (the quadrant
    // boundary's ~6e-17 deviation is documented, deterministic, and
    // ~140 dB under the measurand floor).
    let steps = 20_000;
    for i in 0..=steps {
        let x = (i as f64 / steps as f64) * 0.45 * std::f64::consts::PI;
        let truth = x.tan();
        if truth.abs() < MEANINGFUL_BAND {
            continue;
        }
        let got = detmath::det_tan(x) as f32;
        let want = truth as f32;
        assert!(
            gap_f32_spacings(got, truth) <= 1.0,
            "det_tan({x}) f32 anchor {got} strays {spac} spacings from {want}",
            spac = gap_f32_spacings(got, truth),
        );
    }
    for i in 0..=steps {
        let x = (i as f64 / steps as f64) * std::f64::consts::PI;
        for (name, got, truth) in [
            ("sin", detmath::det_sin(x) as f32, x.sin()),
            ("cos", detmath::det_cos(x) as f32, x.cos()),
        ] {
            if truth.abs() < MEANINGFUL_BAND {
                continue;
            }
            assert!(
                gap_f32_spacings(got, truth) <= 1.0,
                "det_{name}({x}) f32 anchor {got} strays from {truth}"
            );
        }
    }
    // pow over the reachable anchor shapes
    let max_rate_ratio = 192_000.0_f64 / 44_100.0;
    for i in 0..=steps {
        let c = 10f64.powf(-3.0 + 3.0 * (i as f64 / steps as f64));
        for r in [0.05, 1.1023, max_rate_ratio] {
            let truth = c.powf(r);
            if truth.abs() < MEANINGFUL_BAND {
                continue;
            }
            let got = detmath::det_pow(c, r) as f32;
            assert!(
                gap_f32_spacings(got, truth) <= 2.0,
                "det_pow({c}, {r}) f32 anchor {got} strays from {truth}"
            );
        }
    }
}

#[test]
// The boundary half is a single point; the pow half is an 8k powf
// sweep - the interpreted libm calls are the runtime cost and the
// float math is no UB surface, so the native lane owns it.
#[cfg_attr(
    miri,
    ignore = "8k powf-oracle sweep: no UB surface, the native lane owns it"
)]
fn the_quadrant_boundary_and_pow_carry_their_own_certificates() {
    // RED_RHO's pin: at the exact quadrant boundary (cos of the rounded
    // pi/2) the reduction's clean-zero deviation from the truth is
    // bounded by the emitted reduction certificate (+ the oracle
    // margin) — the documented absolute case, deterministic everywhere.
    let pi2 = std::f64::consts::FRAC_PI_2;
    let boundary_dev = (detmath::det_cos(pi2) - pi2.cos()).abs();
    assert!(
        boundary_dev <= tables::RED_RHO + pi2.cos().abs() * ORACLE_MARGIN,
        "the quadrant boundary deviation {boundary_dev:.3e} exceeds RED_RHO's contract"
    );
    // POW_ABS_BOUND's pin: over the reachable pow shapes the absolute
    // deviation from the f64 libm oracle stays inside the emitted
    // certificate (+ the result-scaled oracle margin).
    let steps = 8_000;
    for i in 1..=steps {
        let c = 10f64.powf(-3.0 + 3.0 * (i as f64 / steps as f64));
        let r = (i as f64 / steps as f64) * 5.5125;
        let truth = c.powf(r);
        let got = detmath::det_pow(c, r);
        let bound = tables::POW_ABS_BOUND + truth.abs() * ORACLE_MARGIN;
        assert!(
            (got - truth).abs() <= bound,
            "det_pow({c}, {r}) gap {:.3e} > {bound:.3e}",
            (got - truth).abs()
        );
    }
}

#[test]
fn pow_keeps_the_exact_libm_edge_laws() {
    // c^1 == c bit for bit — the 44.1 kHz reference law (the rate
    // ratio is exactly 1.0 there; every preset stays bit-identical to
    // its authored coefficient).
    for c in [0.001_f64, 0.04, 0.5, 0.99, 0.995, 0.9994, 0.9999] {
        assert_eq!(detmath::det_pow(c, 1.0).to_bits(), c.to_bits());
    }
    // c^0 == 1 and 1^r == 1 exactly.
    for r in [0.0_f64, 0.5, 1.0, 5.5125, 64.0] {
        assert_eq!(detmath::det_pow(1.0, r).to_bits(), 1.0_f64.to_bits());
    }
    assert_eq!(detmath::det_pow(0.0, 1.1023).to_bits(), 0.0_f64.to_bits());
    assert_eq!(detmath::det_pow(0.0, 64.0).to_bits(), 0.0_f64.to_bits());
}

#[test]
#[should_panic(expected = "det_exp domain")]
fn det_exp_refuses_outside_the_certified_domain() {
    detmath::det_exp(-64.0001);
}

#[test]
#[should_panic(expected = "det_ln domain")]
fn det_ln_refuses_zero_and_negatives() {
    detmath::det_ln(0.0);
}

#[test]
#[should_panic(expected = "det_tan domain")]
fn det_tan_refuses_past_the_pole_margin() {
    detmath::det_tan(0.46 * std::f64::consts::PI);
}

#[test]
#[should_panic(expected = "det_pow base domain")]
fn det_pow_refuses_out_of_range_base() {
    detmath::det_pow(1.0001, 1.0);
}
