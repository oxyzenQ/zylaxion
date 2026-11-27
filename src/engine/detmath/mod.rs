// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! night-frontier-Z5: the certified bounded-domain transcendental lane.
//!
//! Every libm transcendental on the audio path (`tan`, `exp`, `powf`,
//! `cos`, `sin` in `src/profiles/mechanical/{init,render}.rs`) resolves
//! through the host's per-target libm — the measured boundary that kept
//! the golden corpus's hash rows machine-shaped (per-target libm and
//! CPU dispatch, docs/ZACTRIX_AUDIT.md). This lane replaces those call
//! sites with pure IEEE-754 f64 arithmetic: Taylor polynomials on
//! closed, architecture-locked domains, evaluated by Horner in plain
//! mul/add. Rust never contracts mul+add into FMA, so the operation
//! sequence — and therefore the returned bits — is identical on
//! x86-64-v3, x86-64-v4, arm64, and every glibc version, forever.
//!
//! The certificates (an exact-Fraction over-bound per function, plus
//! the golden bit vectors) live in `tables.rs` and
//! `test/engine/detmath_vectors.rs`; the generator that produced both
//! is `scripts/engine/generate-detmath.py` — the single algorithmic
//! source, stdlib-only, which simulates the exact op order below.
//!
//! # The certified domains (the architecture locks these)
//!
//! | function | domain | certificate |
//! |----------|--------|-------------|
//! | [`det_exp`] | `[-64, 0]` | relative `EXP_REL_BOUND` (6.8e-15) |
//! | [`det_ln`] | `(0, 1]` | absolute `LN_ABS_BOUND` (1.4e-15) |
//! | [`det_pow`] | `c^r`, `c in [0, 1]`, `r in [0, 64]` | absolute `POW_ABS_BOUND` (9.8e-14) |
//! | [`det_sin`] / [`det_cos`] | `[0, 4*pi]` | absolute `SIN_ABS_BOUND` / `COS_ABS_BOUND` |
//! | [`det_tan`] | `[0, 0.45*pi]` | absolute `TAN_ABS_BOUND` (8.8e-15) |
//!
//! The domains cover every reachable call site with large margins: the
//! travel-path `tan` arguments are clamped to `[pi*20/sr, 0.44*pi]` by
//! the render clamp law (night-cafe-mclaren-2: the certified ceiling —
//! the former 0.45 fraction let the f32 argument land one ULP past this
//! bound at the 48k/96k/192k device rates), the housing fade argument
//! is `[-3, 0]` by construction, and the decay-power arguments ride the
//! `c^(R/S)` law (`c` clamped to `[0, 0.9999]`, the rate ratio bounded
//! by real device rates). A domain exit is UNREACHABLE by those
//! construction laws; the guards below still refuse loudly instead of
//! computing outside the certificate — the house's clear-refusal law,
//! enforced as a tripwire.
//!
//! # Accuracy at the f32 anchor
//!
//! Every bound above sits orders of magnitude below one f32 spacing
//! (6e-8 relative), so each f32 anchor the init path bakes from these
//! values stays within the 3C law's half-spacing ideal; the one
//! exception is documented, deterministic, and physically irrelevant —
//! at the exact quadrant boundary (`cos` of the rounded `pi/2`) the
//! Cody-Waite reduction returns a clean zero where the true cosine of
//! the rounded constant is ~6.1e-17: an absolute deviation ~140 dB
//! below the -60 dB measurand floor, identical bits on every machine.

pub(crate) mod tables;

/// Horner evaluation, descending coefficients — the exact operation
/// order the generator simulates (`poly_horner` there), so the bits
/// match the golden vectors bit for bit.
fn horner(coeffs: &[f64], x: f64) -> f64 {
    let mut acc = 0.0;
    for c in coeffs {
        acc = acc * x + c;
    }
    acc
}

/// `e^x` for `x` in `[-64, 0]`, certified to `EXP_REL_BOUND` relative.
///
/// Exponent split: `n = round(x * log2(e))` (Rust's round — half away
/// from zero, the same rounding the generator simulates), the residual
/// `r = x - n*LN2_HI - n*LN2_LO` through the exact-split Cody-Waite
/// pair (n*LN2_HI is exact: LN2_HI carries 21 trailing zero bits), the
/// Taylor core on `r in [-0.35, 0.35]`, and the exact `2^n` scaling by
/// bit assembly.
pub fn det_exp(x: f64) -> f64 {
    assert!(
        (-64.0..=0.0).contains(&x) && x.is_finite(),
        "det_exp domain is [-64, 0], got {x}"
    );
    if x == 0.0 {
        return 1.0;
    }
    let nf = (x * tables::LOG2E).round();
    let r = x - nf * tables::LN2_HI;
    let r = r - nf * tables::LN2_LO;
    let p = horner(&tables::EXP_COEFFS, r);
    // 2^n by bit assembly — exact for n in [-1022, 1023]; the domain
    // guard keeps n in [-93, 0].
    let scale = f64::from_bits(((nf as i64 + 1023) as u64) << 52);
    p * scale
}

/// `ln(x)` for `x` in `(0, 1]`, certified to `LN_ABS_BOUND` absolute.
///
/// Exponent fold: `x = 2^e * y` with `y` in `[sqrt(2)/2, sqrt(2))`
/// (exact power-of-two scaling), then the atanh series
/// `ln(y) = 2*atanh((y-1)/(y+1))` — a same-sign series on
/// `|z| <= 0.1716`, no cancellation anywhere. `ln(1)` returns exactly
/// 0.0.
pub fn det_ln(x: f64) -> f64 {
    assert!(
        (0.0..=1.0).contains(&x) && x > 0.0 && x.is_finite(),
        "det_ln domain is (0, 1], got {x}"
    );
    if x == 1.0 {
        return 0.0;
    }
    let mut e: i64 = 0;
    let mut y = x;
    while y < std::f64::consts::FRAC_1_SQRT_2 {
        y *= 2.0;
        e -= 1;
    }
    while y >= std::f64::consts::SQRT_2 {
        y *= 0.5;
        e += 1;
    }
    let z = (y - 1.0) / (y + 1.0);
    let atanh = z * horner(&tables::ATANH_COEFFS, z * z);
    let lh = (e as f64) * tables::LN2_HI;
    let ll = (e as f64) * tables::LN2_LO;
    (lh + ll) + 2.0 * atanh
}

/// `c^r` for `c` in `[0, 1]`, `r` in `[0, 64]` — the decay law's
/// `c^(R/S)` shape. The exact libm edge cases keep their exact values
/// (`powf(0, r>0) == 0`, `powf(1, r) == 1`, `powf(c, 0) == 1`, and
/// `powf(c, 1) == c` — the last one is the 44.1 kHz reference law:
/// the rate ratio is exactly 1.0 there, and every preset stays
/// bit-identical to its authored coefficient); everything else rides
/// `det_exp(r * det_ln(c))`, certified to `POW_ABS_BOUND` absolute on
/// the `[0, 1]` result band.
pub fn det_pow(c: f64, r: f64) -> f64 {
    assert!(
        (0.0..=1.0).contains(&c) && c.is_finite(),
        "det_pow base domain is [0, 1], got {c}"
    );
    assert!(
        (0.0..=64.0).contains(&r) && r.is_finite(),
        "det_pow exponent domain is [0, 64], got {r}"
    );
    if c == 0.0 {
        return 0.0;
    }
    if r == 1.0 {
        return c;
    }
    if c == 1.0 || r == 0.0 {
        return 1.0;
    }
    det_exp(r * det_ln(c))
}

/// The quadrant reduction: `n = round(x * 2/pi)` (half away from zero,
/// the generator's rounding), then the three-part Cody-Waite residual
/// — `n*PI2_HI` is exact (14 trailing zero bits, valid to `|n| <= 2^14`
/// and the domain keeps `|n| <= 8`), and the split sums beyond 130
/// bits, so `red` carries the argument to the polynomial core with the
/// certified `RED_RHO` absolute error.
fn reduce_quadrant(x: f64) -> (i64, f64) {
    let nf = (x * tables::TWO_OVER_PI).round();
    let red = x - nf * tables::PI2_HI;
    let red = red - nf * tables::PI2_LO;
    let red = red - nf * tables::PI2_LO2;
    (nf as i64, red)
}

/// `sin(x)` for `x` in `[0, 4*pi]`, certified to `SIN_ABS_BOUND`
/// absolute. The quadrant table rides the reduced argument; the core
/// evaluates `red * S(red^2)` — the factored form keeps the relative
/// error bounded all the way to `red = 0`.
pub fn det_sin(x: f64) -> f64 {
    assert!(
        (0.0..=(4.0 * std::f64::consts::PI)).contains(&x) && x.is_finite(),
        "det_sin domain is [0, 4*pi], got {x}"
    );
    let (n, red) = reduce_quadrant(x);
    let t = red * red;
    let s = red * horner(&tables::SIN_COEFFS, t);
    let q = horner(&tables::COS_COEFFS, t);
    match n & 3 {
        0 => s,
        1 => q,
        2 => -s,
        _ => -q,
    }
}

/// `cos(x)` for `x` in `[0, 4*pi]`, certified to `COS_ABS_BOUND`
/// absolute. Same reduction as [`det_sin`]; the quadrant table shifts
/// by one.
pub fn det_cos(x: f64) -> f64 {
    assert!(
        (0.0..=(4.0 * std::f64::consts::PI)).contains(&x) && x.is_finite(),
        "det_cos domain is [0, 4*pi], got {x}"
    );
    let (n, red) = reduce_quadrant(x);
    let t = red * red;
    let s = red * horner(&tables::SIN_COEFFS, t);
    let q = horner(&tables::COS_COEFFS, t);
    match n & 3 {
        0 => q,
        1 => -s,
        2 => -q,
        _ => s,
    }
}

/// `tan(x)` for `x` in `[0, 0.45*pi]`, certified to `TAN_ABS_BOUND`
/// absolute. The ratio form — `det_sin(x) / det_cos(x)` — never meets
/// a division hazard on this domain: the clamp law keeps `x` at most
/// `0.45*pi`, where `cos(x) >= 0.1564` with a huge margin.
pub fn det_tan(x: f64) -> f64 {
    assert!(
        (0.0..=(0.45 * std::f64::consts::PI)).contains(&x) && x.is_finite(),
        "det_tan domain is [0, 0.45*pi], got {x}"
    );
    det_sin(x) / det_cos(x)
}
