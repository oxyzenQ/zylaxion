// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

// GENERATED FILE -- do not edit by hand. Regenerate with:
//   python3 scripts/engine/generate-detmath.py
// night-frontier-Z5: the certified bounded-domain transcendental
// tables. Coefficients are exact rationals (1/k! and friends)
// converted to f64 by correctly-rounded division; every bound below
// is an exact-Fraction certificate rounded OUTWARD (an over-bound,
// never an under-bound). The companion vectors live in
// test/engine/detmath_vectors.rs and pin the bits of the lane.

/// exp core on r in [-0.35, 0.35]: sum r^k / k!, ascending k
pub(crate) const EXP_COEFFS: [f64; 16] = [
    7.647163731819816e-13,
    1.1470745597729725e-11,
    1.6059043836821613e-10,
    2.08767569878681e-09,
    2.505210838544172e-08,
    2.755731922398589e-07,
    2.7557319223985893e-06,
    2.48015873015873e-05,
    0.0001984126984126984,
    0.001388888888888889,
    0.008333333333333333,
    0.041666666666666664,
    0.16666666666666666,
    0.5,
    1.0,
    1.0,
];

/// atanh core for ln: atanh(z) = z * sum z^2k / (2k+1), ascending k
pub(crate) const ATANH_COEFFS: [f64; 13] = [
    0.04,
    0.043478260869565216,
    0.047619047619047616,
    0.05263157894736842,
    0.058823529411764705,
    0.06666666666666667,
    0.07692307692307693,
    0.09090909090909091,
    0.1111111111111111,
    0.14285714285714285,
    0.2,
    0.3333333333333333,
    1.0,
];

/// sin core: sin(red) = red * sum (red^2)^k * (-1)^k / (2k+1)!, ascending k
pub(crate) const SIN_COEFFS: [f64; 9] = [
    2.8114572543455206e-15,
    -7.647163731819816e-13,
    1.6059043836821613e-10,
    -2.505210838544172e-08,
    2.7557319223985893e-06,
    -0.0001984126984126984,
    0.008333333333333333,
    -0.16666666666666666,
    1.0,
];

/// cos core: cos(red) = sum (red^2)^k * (-1)^k / (2k)!, ascending k
pub(crate) const COS_COEFFS: [f64; 10] = [
    -1.5619206968586225e-16,
    4.779477332387385e-14,
    -1.1470745597729725e-11,
    2.08767569878681e-09,
    -2.755731922398589e-07,
    2.48015873015873e-05,
    -0.001388888888888889,
    0.041666666666666664,
    -0.5,
    1.0,
];

/// Cody-Waite pi/2 triple: HI carries 14 trailing zero bits (k*HI is
/// exact for |k| <= 2^14), LO + LO2 carry the rest; the three parts
/// sum to the correctly-rounded f64 pi/2 beyond 130 bits.
pub(crate) const PI2_HI: f64 = 1.5707963267341256;
pub(crate) const PI2_LO: f64 = 6.077100506506192e-11;
pub(crate) const PI2_LO2: f64 = 3.5215598651832e-27;
/// ln 2 split: HI carries 21 trailing zero bits (n*HI exact for
/// |n| <= 2^31), LO carries the remainder.
pub(crate) const LN2_HI: f64 = 0.6931471803691238;
pub(crate) const LN2_LO: f64 = 1.9082149292705877e-10;
/// 2/pi and log2(e): the std constants are the SAME correctly-rounded
/// f64 values the generator computes from the exact rationals (asserted
/// below), so the lane reads them from std by name.
pub(crate) const TWO_OVER_PI: f64 = std::f64::consts::FRAC_2_PI;
pub(crate) const LOG2E: f64 = std::f64::consts::LOG2_E;

// The certified absolute bounds (exact-Fraction over-bounds). Every
// value the lane returns deviates from the true function by no more
// than its bound on the whole certified domain -- deviation is a
// mathematical property of the polynomials, not a measurement. The
// bounds are enforced by the detmath test tree, so they ship under
// cfg(test): a certificate exists exactly where it is enforced.
#[cfg(test)]
pub(crate) const EXP_REL_BOUND: f64 = 6.713e-15;
#[cfg(test)]
pub(crate) const LN_ABS_BOUND: f64 = 1.338e-15;
#[cfg(test)]
pub(crate) const RED_RHO: f64 = 7.68e-16;
#[cfg(test)]
pub(crate) const SIN_ABS_BOUND: f64 = 3.402e-15;
#[cfg(test)]
pub(crate) const COS_ABS_BOUND: f64 = 5.081e-15;
#[cfg(test)]
pub(crate) const TAN_ABS_BOUND: f64 = 8.706e-15;
#[cfg(test)]
pub(crate) const POW_ABS_BOUND: f64 = 9.7382e-14;
