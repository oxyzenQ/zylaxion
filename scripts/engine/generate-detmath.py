#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
"""night-frontier-Z5: the certified bounded-domain transcendental generator.

Generates the coefficient tables, the certified error bounds, and the
golden bit vectors for src/engine/detmath/ -- the lane that removes
every libm transcendental from the zylaxion audio path so the rendered
bytes become machine-independent (same bits on x86-64-v3/v4, arm64,
any glibc, forever).

Design (docs/INNOVATIONS.md entry 47):
  - every function is a Taylor series on a CLOSED, architecture-locked
    domain, evaluated by Horner in plain IEEE-754 f64 mul/add (Rust
    never contracts mul+add into FMA, so the op sequence -- and the
    bits -- are identical on every target);
  - coefficients are exact rationals (1/k! and friends) converted to
    f64 by correctly-rounded division;
  - every bound below is computed EXACTLY with Fractions (truncation
    term plus a conservative Horner rounding accumulation), then
    rounded OUTWARD to a clean over-bound;
  - this script SIMULATES the exact Rust algorithm step by step in
    Python floats (IEEE-754 binary64, same correctly-rounded ops, the
    same round-half-away rounding Rust's f64::round uses) and emits
    (x_bits, y_bits) golden vectors the Rust test tree pins bit for
    bit.

Domain laws (the architecture locks these):
  - det_exp:  x in [-64, 0]        (guard refuses outside)
  - det_ln:   x in (0, 1]          (ln(1) == 0 exactly)
  - det_pow:  c^r, c in [0, 1], r in [0, 64]
  - det_sin / det_cos: x in [0, 4*pi]   (quadrant reduction, k <= 8)
  - det_tan:  x in [0, 0.45*pi]    (ratio form, cos >= 0.1564)

The generator is stdlib-only by the house dependency doctrine.
"""

import math
import struct
from fractions import Fraction

# ── exact arithmetic helpers ──────────────────────────────────────────

U = Fraction(1, 2 ** 53)  # IEEE binary64 unit roundoff (half-spacing rel)


def to_f64(fr: Fraction) -> float:
    """Correctly-rounded Fraction -> f64 (Python's __float__ performs
    one correctly-rounded division -- the IEEE contract)."""
    return float(fr)


def over_bound(fr: Fraction, digits: int = 18) -> Fraction:
    """Round a Fraction UP to a clean decimal over-bound (the emitted
    certified bound must be >= the true bound, never under)."""
    scaled = fr * (10 ** digits)
    bumped = math.ceil(scaled)  # exact ceil on the integer scale
    return Fraction(bumped, 10 ** digits)


def rust_lit(x: float) -> str:
    """Shortest round-trip Rust f64 literal for a Python float."""
    if x != x or x in (float("inf"), float("-inf")):
        raise ValueError("non-finite coefficient")
    return repr(x)


def rust_round(t: float) -> int:
    """Rust's f64::round: round half AWAY from zero (Python's round is
    banker's rounding -- the mismatch would break the golden master)."""
    return math.floor(t + 0.5) if t >= 0.0 else math.ceil(t - 0.5)


def f64_bits(x: float) -> int:
    return struct.unpack(">Q", struct.pack(">d", x))[0]


def bits_f64(b: int) -> float:
    return struct.unpack(">d", struct.pack(">Q", b))[0]


# ── series coefficients (exact rationals) ────────────────────────────

def factorial(n: int) -> int:
    r = 1
    for i in range(2, n + 1):
        r *= i
    return r


# exp core on r in [-EXP_R, EXP_R]: sum r^k / k!, k = 0..EXP_N
EXP_N = 15
EXP_R = Fraction(35, 100)  # 0.35, over ln2/2 = 0.34657...

# atanh core for ln: z = (y-1)/(y+1), |z| <= ATANH_Z; atanh(z) = z * P(z^2)
ATANH_Z = Fraction(172, 1000)  # 0.172, over (sqrt(2)-1)/(sqrt(2)+1) = 0.1716
ATANH_N = 12  # inner degree; truncation term z^(2N+3)/(2N+3)

# sin core: sin(red) = red * S(red^2), red in [-SIN_R, SIN_R]
SIN_N = 8
SIN_R = Fraction(8, 10)  # 0.8, over pi/4 = 0.7854

# cos core: cos(red) = Q(red^2)
COS_N = 9
COS_R = SIN_R

EXP_COEFFS = [Fraction(1, factorial(k)) for k in range(EXP_N + 1)]  # ascending k
ATANH_COEFFS = [Fraction(1, 2 * k + 1) for k in range(ATANH_N + 1)]  # ascending
SIN_COEFFS = [Fraction((-1) ** k, factorial(2 * k + 1)) for k in range(SIN_N + 1)]
COS_COEFFS = [Fraction((-1) ** k, factorial(2 * k)) for k in range(COS_N + 1)]

# ── exact constants: ln 2 and pi as Fractions ────────────────────────

_ln2 = Fraction(0)
for k in range(1, 400):
    _ln2 += Fraction(1, 2) ** k / k
LN2 = _ln2


def arctan_inv(x: int) -> Fraction:
    """arctan(1/x) for integer x >= 2, exact to 220+ digits (Machin)."""
    total = Fraction(0)
    k = 0
    xp = Fraction(1, x)
    x2 = Fraction(x * x)
    while True:
        term = Fraction(xp, 2 * k + 1)
        if k & 1:
            term = -term
        total += term
        xp /= x2
        tail = abs(term) * x2 / (x2 - 1)
        if tail < Fraction(1, 10 ** 220):
            break
        k += 1
    return total


PI = 4 * (4 * arctan_inv(5) - arctan_inv(239))
PI2 = PI / 2

# ── Cody-Waite splits (the exact f64 bit patterns the module uses) ───

# pi/2 triple: HI carries 14 trailing zero bits (k*HI exact for
# |k| <= 2^14), LO + LO2 carry the rest.
PI2_HI_F = bits_f64(0x3FF921FB54400000)
PI2_HI = Fraction(PI2_HI_F)
_resid1 = PI2 - PI2_HI
PI2_LO_F = to_f64(_resid1)
PI2_LO = Fraction(PI2_LO_F)
_resid2 = _resid1 - PI2_LO
PI2_LO2_F = to_f64(_resid2)
PI2_LO2 = Fraction(PI2_LO2_F)

# ln 2 split: HI carries 21 trailing zero bits (n*HI exact for
# |n| <= 2^31), LO carries the remainder.
_bits = f64_bits(to_f64(LN2))
_bits &= ~((1 << 21) - 1)
LN2_HI_F = bits_f64(_bits)
LN2_HI = Fraction(LN2_HI_F)
LN2_LO_F = to_f64(LN2 - LN2_HI)
LN2_LO = Fraction(LN2_LO_F)

TWO_OVER_PI_F = to_f64(Fraction(2) / PI)
LOG2E_F = to_f64(Fraction(1) / LN2)

# sanity: the splits must be exact where the laws demand it
assert (int(f64_bits(PI2_HI_F)) & ((1 << 14) - 1)) == 0, "PI2_HI must carry 14 trailing zero bits"
assert (int(f64_bits(LN2_HI_F)) & ((1 << 21) - 1)) == 0, "LN2_HI must carry 21 trailing zero bits"
assert PI2_HI + PI2_LO + PI2_LO2 - PI2 < Fraction(1, 10 ** 130), "pi/2 split must exceed 130 bits"
assert LN2_HI + LN2_LO - LN2 < Fraction(1, 10 ** 130), "ln2 split must exceed 130 bits"

# ── certified bounds (exact Fraction math) ────────────────────────────

exp_trunc = EXP_R ** (EXP_N + 1) / factorial(EXP_N + 1)
exp_min_result = Fraction(7046, 10000)  # under-estimate of e^-0.35 = 0.70468
exp_terms = Fraction(142, 100)  # over-bound of e^0.35 = 1.41906
EXP_REL_BOUND = over_bound(
    exp_trunc / exp_min_result + 2 * EXP_N * U * exp_terms / exp_min_result
)

atanh_trunc = 2 * (ATANH_Z ** (2 * ATANH_N + 3) / (2 * ATANH_N + 3))
atanh_terms = Fraction(348, 1000)  # over-bound of 2*atanh(0.172) = 0.34711
LN_ABS_BOUND = over_bound(atanh_trunc + 2 * (ATANH_N + 1) * U * atanh_terms + 3 * U)

t_max = SIN_R ** 2
sin_s_min = Fraction(85, 100)  # |S| >= 1 - t/6 - t^2/120 ... >= 0.85 on t <= 0.64
sin_terms = Fraction(112, 100)  # over-bound of sinh(0.8)/0.8 = 1.1101
SIN_CORE_BOUND = over_bound(
    t_max ** (SIN_N + 1) / factorial(2 * (SIN_N + 1) + 1) / sin_s_min
    + 2 * (SIN_N + 1) * U * sin_terms / sin_s_min
)

cos_q_min = Fraction(69, 100)  # |Q| = cos(red) >= cos(0.8) = 0.69670
cos_terms = Fraction(134, 100)  # over-bound of cosh(0.8) = 1.33742
COS_CORE_BOUND = over_bound(
    t_max ** (COS_N + 1) / factorial(2 * (COS_N + 1)) / cos_q_min
    + 2 * (COS_N + 1) * U * cos_terms / cos_q_min
)

# reduction argument error: k <= 8, split residual beyond 130 bits, the
# k*LO rounding, the three subtractions, and the k-selection rounding.
k_max = 8
split_resid = abs(PI2 - PI2_HI - PI2_LO - PI2_LO2)
RED_RHO = over_bound(
    k_max * split_resid
    + k_max * abs(PI2_LO) * 2 * U
    + k_max * abs(PI2_LO2) * 2 * U
    + 3 * U * Fraction(8, 10)
    + U * Fraction(4513, 1000)  # u * 4pi over-bound (k selection at x = 4pi)
)

SIN_ABS_BOUND = over_bound(SIN_CORE_BOUND + RED_RHO)
COS_ABS_BOUND = over_bound(COS_CORE_BOUND + RED_RHO)
TAN_ABS_BOUND = over_bound(SIN_CORE_BOUND + COS_CORE_BOUND + 2 * RED_RHO + 2 * U)
POW_ABS_BOUND = over_bound(
    # c^r = exp(r * ln c): the ln error scaled by r <= 64, plus the
    # product rounding, plus the exp relative bound on |result| <= 1
    Fraction(64) * LN_ABS_BOUND + 64 * LN2 * U + U
    + (Fraction(1) / Fraction(10000)) * 0  # placeholder keeps the sum exact
    + EXP_REL_BOUND
)
POW_ABS_BOUND = over_bound(Fraction(64) * LN_ABS_BOUND + Fraction(64) * LN2 * U + U + EXP_REL_BOUND)

# ── the algorithm, simulated step by step (golden master) ────────────

EXP_DESC = [to_f64(c) for c in reversed(EXP_COEFFS)]
ATANH_DESC = [to_f64(c) for c in reversed(ATANH_COEFFS)]
SIN_DESC = [to_f64(c) for c in reversed(SIN_COEFFS)]
COS_DESC = [to_f64(c) for c in reversed(COS_COEFFS)]


def poly_horner(coeffs_desc, x):
    """Plain IEEE Horner, descending coefficients -- the exact op order
    the Rust module uses (no FMA anywhere)."""
    acc = 0.0
    for c in coeffs_desc:
        acc = acc * x + c
    return acc


def det_exp(x):
    if not (-64.0 <= x <= 0.0):
        raise ValueError(f"det_exp domain: {x}")
    if x == 0.0:
        return 1.0
    n = rust_round(x * LOG2E_F)
    r = x - n * LN2_HI_F
    r = r - n * LN2_LO_F
    p = poly_horner(EXP_DESC, r)
    scale = bits_f64((n + 1023) << 52)  # 2^n by bit assembly, exact
    return p * scale


def det_ln(x):
    if not (0.0 < x <= 1.0):
        raise ValueError(f"det_ln domain: {x}")
    if x == 1.0:
        return 0.0
    # exponent fold: x = 2^e * y, y in [sqrt(2)/2, sqrt(2))
    e = 0
    y = x
    while y < 0.7071067811865476:
        y *= 2.0
        e -= 1
    while y >= 1.4142135623730951:
        y *= 0.5
        e += 1
    z = (y - 1.0) / (y + 1.0)
    atanh = z * poly_horner(ATANH_DESC, z * z)
    lh = e * LN2_HI_F  # exact: LN2_HI carries 21 trailing zero bits
    ll = e * LN2_LO_F
    return (lh + ll) + 2.0 * atanh


def _reduce_quadrant(x):
    n = rust_round(x * TWO_OVER_PI_F)
    red = x - n * PI2_HI_F
    red = red - n * PI2_LO_F
    red = red - n * PI2_LO2_F
    return n, red


def det_sin(x):
    if not (0.0 <= x <= 4.0 * math.pi):
        raise ValueError(f"det_sin domain: {x}")
    n, red = _reduce_quadrant(x)
    t = red * red
    s = red * poly_horner(SIN_DESC, t)
    q = poly_horner(COS_DESC, t)
    m = n & 3
    if m == 0:
        return s
    if m == 1:
        return q
    if m == 2:
        return -s
    return -q


def det_cos(x):
    if not (0.0 <= x <= 4.0 * math.pi):
        raise ValueError(f"det_cos domain: {x}")
    n, red = _reduce_quadrant(x)
    t = red * red
    s = red * poly_horner(SIN_DESC, t)
    q = poly_horner(COS_DESC, t)
    m = n & 3
    if m == 0:
        return q
    if m == 1:
        return -s
    if m == 2:
        return -q
    return s


def det_tan(x):
    if not (0.0 <= x <= 0.45 * math.pi):
        raise ValueError(f"det_tan domain: {x}")
    return det_sin(x) / det_cos(x)


def det_pow(c, r):
    if not (0.0 <= c <= 1.0):
        raise ValueError(f"det_pow base domain: {c}")
    if not (0.0 <= r <= 64.0):
        raise ValueError(f"det_pow exponent domain: {r}")
    if c == 0.0:
        return 0.0
    if r == 1.0:
        return c  # the 44.1 kHz reference law: pow(c, 1) == c exactly
    if c == 1.0 or r == 0.0:
        return 1.0
    return det_exp(r * det_ln(c))


# ── measured vs certified: dense probes against f64 libm ─────────────

def measure(fn, oracle, points, name):
    worst = 0.0
    worst_x = 0.0
    for x in points:
        gap = abs(fn(x) - oracle(x))
        if gap > worst:
            worst = gap
            worst_x = x
    print(f"  {name:9s} measured worst |det - f64 libm| = {worst:.3e} at x={worst_x:.6g}")
    return worst


def f32(x: float) -> float:
    return struct.unpack(">f", struct.pack(">f", x))[0]


def main():
    print("night-frontier-Z5 detmath generator: certified bounds")
    for name, b in [
        ("EXP_REL_BOUND", EXP_REL_BOUND),
        ("LN_ABS_BOUND", LN_ABS_BOUND),
        ("RED_RHO", RED_RHO),
        ("SIN_ABS_BOUND", SIN_ABS_BOUND),
        ("COS_ABS_BOUND", COS_ABS_BOUND),
        ("TAN_ABS_BOUND", TAN_ABS_BOUND),
        ("POW_ABS_BOUND", POW_ABS_BOUND),
    ]:
        print(f"  {name:15s} = {float(b):.3e}")
    print("  measured (dense probes vs f64 libm):")
    grid = 50_000
    m = 0.0
    m = max(m, measure(det_exp, math.exp, [-64.0 + i * 64.0 / grid for i in range(grid + 1)], "exp"))
    m = max(m, measure(det_ln, math.log, [10 ** (-9 + 9 * i / grid) for i in range(grid + 1)] + [1.0 - i / grid for i in range(grid)], "ln"))
    m = max(m, measure(det_sin, math.sin, [i * 4 * math.pi / grid for i in range(grid + 1)], "sin"))
    m = max(m, measure(det_cos, math.cos, [i * 4 * math.pi / grid for i in range(grid + 1)], "cos"))
    m = max(m, measure(det_tan, math.tan, [i * 0.45 * math.pi / grid for i in range(grid + 1)], "tan"))
    m = max(m, measure(lambda c: det_pow(c, 1.1023), lambda c: c ** 1.1023,
                       [10 ** (-9 + 4 * i / grid) for i in range(grid + 1)], "pow(c,1.1)"))
    m = max(m, measure(lambda r: det_pow(0.995, r), lambda r: 0.995 ** r,
                       [i * 16.0 / grid for i in range(grid + 1)], "pow(0.995,r)"))
    worst32 = 0.0
    for i in range(grid + 1):
        x = i * 0.45 * math.pi / grid
        worst32 = max(worst32, abs(f32(det_tan(x)) - f32(math.tan(x))))
    print(f"  tan f32-anchor worst |det32 - libm32| = {worst32:.3e}")
    emit_tables()
    emit_vectors()
    print("  emitted: src/engine/detmath/tables.rs, test/engine/detmath_vectors.rs")


# ── emitters ─────────────────────────────────────────────────────────

HEADER = """\
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

"""


def _arr(name, coeffs, comment):
    desc = ",\n    ".join(rust_lit(to_f64(c)) for c in reversed(coeffs))
    return (
        f"/// {comment}\n"
        f"pub(crate) const {name}: [f64; {len(coeffs)}] = [\n    {desc},\n];\n\n"
    )


def emit_tables():
    body = HEADER
    body += _arr("EXP_COEFFS", EXP_COEFFS,
                 "exp core on r in [-0.35, 0.35]: sum r^k / k!, ascending k")
    body += _arr("ATANH_COEFFS", ATANH_COEFFS,
                 "atanh core for ln: atanh(z) = z * sum z^2k / (2k+1), ascending k")
    body += _arr("SIN_COEFFS", SIN_COEFFS,
                 "sin core: sin(red) = red * sum (red^2)^k * (-1)^k / (2k+1)!, ascending k")
    body += _arr("COS_COEFFS", COS_COEFFS,
                 "cos core: cos(red) = sum (red^2)^k * (-1)^k / (2k)!, ascending k")
    body += f"""/// Cody-Waite pi/2 triple: HI carries 14 trailing zero bits (k*HI is
/// exact for |k| <= 2^14), LO + LO2 carry the rest; the three parts
/// sum to the correctly-rounded f64 pi/2 beyond 130 bits.
pub(crate) const PI2_HI: f64 = {rust_lit(PI2_HI_F)};
pub(crate) const PI2_LO: f64 = {rust_lit(PI2_LO_F)};
pub(crate) const PI2_LO2: f64 = {rust_lit(PI2_LO2_F)};
/// ln 2 split: HI carries 21 trailing zero bits (n*HI exact for
/// |n| <= 2^31), LO carries the remainder.
pub(crate) const LN2_HI: f64 = {rust_lit(LN2_HI_F)};
pub(crate) const LN2_LO: f64 = {rust_lit(LN2_LO_F)};
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
pub(crate) const EXP_REL_BOUND: f64 = {rust_lit(float(EXP_REL_BOUND))};
#[cfg(test)]
pub(crate) const LN_ABS_BOUND: f64 = {rust_lit(float(LN_ABS_BOUND))};
#[cfg(test)]
pub(crate) const RED_RHO: f64 = {rust_lit(float(RED_RHO))};
#[cfg(test)]
pub(crate) const SIN_ABS_BOUND: f64 = {rust_lit(float(SIN_ABS_BOUND))};
#[cfg(test)]
pub(crate) const COS_ABS_BOUND: f64 = {rust_lit(float(COS_ABS_BOUND))};
#[cfg(test)]
pub(crate) const TAN_ABS_BOUND: f64 = {rust_lit(float(TAN_ABS_BOUND))};
#[cfg(test)]
pub(crate) const POW_ABS_BOUND: f64 = {rust_lit(float(POW_ABS_BOUND))};
"""
    assert TWO_OVER_PI_F == 0.6366197723675814, "2/pi must equal the std constant's bits"
    assert LOG2E_F == 1.4426950408889634, "log2(e) must equal the std constant's bits"
    with open("src/engine/detmath/tables.rs", "w") as f:
        f.write(body)


def emit_vectors():
    """Golden master vectors: the Python simulation IS the Rust
    algorithm (same IEEE ops, same order, same round-half-away), so
    its bits are the bits the Rust module must produce."""
    single = []  # (name, x_bits, y_bits)
    power = []  # (c_bits, r_bits, y_bits)
    for x in [0.0, -1e-12, -0.001, -0.005208333, -0.0226757, -0.0231,
              -0.125, -0.3466, -0.35, -1.0, -3.0, -2.9999999,
              -8.0, -32.0, -63.999, -64.0]:
        single.append(("exp", f64_bits(x), f64_bits(det_exp(x))))
    for x in [1.0, 0.9999, 0.9994, 0.995, 0.99, 0.5, 0.47, 0.04,
              0.7071067811865476, 0.7071067811865475,
              1.4142135623730951 / 2, 1.4142135623730951 / 4,
              1e-8, 2.0 ** -30]:
        single.append(("ln", f64_bits(x), f64_bits(det_ln(x))))
    pi2 = PI2_HI_F
    for x in [0.0, 1e-12, 1e-6, 0.1, 0.2849, 0.7853981633974483,
              pi2 - 1e-12, pi2, pi2 + 1e-12, 1.413716694115407,
              math.pi, 1.5 * pi2, 2.0 * math.pi, 3.0 * pi2,
              4.0 * math.pi, 4.0 * math.pi - 1e-9]:
        single.append(("sin", f64_bits(x), f64_bits(det_sin(x))))
        single.append(("cos", f64_bits(x), f64_bits(det_cos(x))))
    for x in [0.0, 1e-9, 0.7853981633974483, 1.0, 1.2, 1.4,
              1.413716694115407, 0.45 * math.pi]:
        single.append(("tan", f64_bits(x), f64_bits(det_tan(x))))
    for c, r in [(0.0, 1.1023), (1.0, 5.5125), (0.9994, 1.0),
                 (0.9994, 4.3537414965986395), (0.995, 1.0884),
                 (0.99, 2.75625), (0.04, 0.0226757),
                 (0.04, 1.0 / 88.2), (0.5, 5.5125), (0.9999, 4.3537)]:
        power.append((f64_bits(c), f64_bits(r), f64_bits(det_pow(c, r))))

    lines = [HEADER,
             "// The golden bit vectors: (x_bits, y_bits) pairs the test tree\n"
             "// pins bit for bit, plus the two-input det_pow table. Produced\n"
             "// by the generator's own simulation of the algorithm (same\n"
             "// IEEE-754 ops, same order, same round-half-away).\n\n",
             "/// One single-input golden vector: the input's bit pattern and\n"
             "/// the exact bits the lane must return.\n"
             "pub struct DetVector {\n    pub name: &'static str,\n    pub xb: u64,\n    pub yb: u64,\n}\n\n"
             "/// One det_pow golden vector: base bits, exponent bits, result bits.\n"
             "pub struct PowVector {\n    pub cb: u64,\n    pub rb: u64,\n    pub yb: u64,\n}\n\n",
             "pub(crate) const VECTORS: &[DetVector] = &[\n"]
    for name, xb, yb in single:
        x = bits_f64(xb)
        lines.append(f'    DetVector {{ name: "{name}({rust_lit(x)})", xb: {xb:#018x}, yb: {yb:#018x} }},\n')
    lines.append("];\n\npub(crate) const POW_VECTORS: &[PowVector] = &[\n")
    for cb, rb, yb in power:
        c = bits_f64(cb)
        r = bits_f64(rb)
        lines.append(f'    PowVector {{ cb: {cb:#018x}, rb: {rb:#018x}, yb: {yb:#018x} }}, // pow({rust_lit(c)}, {rust_lit(r)})\n')
    lines.append("];\n")
    with open("test/engine/detmath_vectors.rs", "w") as f:
        f.write("".join(lines))


if __name__ == "__main__":
    main()
