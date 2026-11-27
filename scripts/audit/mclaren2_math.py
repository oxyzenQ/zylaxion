#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
"""night-cafe-mclaren-2 math proofs: detmath domain edges, ring budget,
drift bounds. Every number the audit doc cites comes from this run."""
import struct
import math

def f32(x):
    """Round a Python float to the nearest f32 value (exact f64 result)."""
    return struct.unpack('f', struct.pack('f', x))[0]

PI_F64 = math.pi
PI_F32 = f32(math.pi)          # std::f32::consts::PI

print("== A. render tan edge: PI_f32 * fc / sr vs det_tan domain [0, 0.45*PI_f64] ==")
BOUND = 0.45 * PI_F64          # the det_tan f64 assert bound
worst = None
for sr in [44100, 48000, 96000, 192000, 384000]:
    # fc clamp ceiling, computed in f32 like the render path: 0.45 * sr (f32)
    fc_cap = f32(0.45 * sr)
    # the clamp keeps fc <= fc_cap; take fc = fc_cap exactly (worst case)
    # render arg in f32: (PI_f32 * fc) / sr  -- f32 mul then f32 div, both rounded
    mul = f32(PI_F32 * fc_cap)
    arg = f32(mul / sr)
    ok = arg <= BOUND
    margin = BOUND - arg
    if worst is None or margin < worst[0]:
        worst = (margin, sr, fc_cap, arg)
    print(f"  sr={sr:7d} fc_cap={fc_cap!r:>14} arg_f64={arg!r:24} inside={ok}  margin={margin:.3e}")
print(f"  -> tightest: {worst}")
assert all(True for _ in [1])  # continue regardless; verdict printed

print()
print("== B. init housing tan edge: PI_f64 * housing_freq / sr ==")
HOUSING_MAX = 1000.0 * 1.015   # HOUSING_FREQ_MAX * (1 + PITCH_DRIFT_FRAC)
for sr in [44100, 48000, 96000, 192000]:
    arg = PI_F64 * HOUSING_MAX / sr
    print(f"  sr={sr:7d} arg={arg:.6f} inside={arg <= BOUND}  (0.45pi={BOUND:.6f})")

print()
print("== C. det_pow -> det_exp domain: worst r*ln(c) over authored clamps ==")
DECAY_COEFF_MAX = 0.9999
SAMPLE_RATE = 44100  # profiles::SAMPLE_RATE (check below if cited)
for c in [0.9999, 0.04, 1e-3, 1e-6, 1e-9]:
    ln_c = math.log(c) if c > 0 else float('-inf')
    print(f"  c={c:g}  ln(c)={ln_c:.4f}  r_max*ln(c) = {ln_c*1.0:.4f}  (det_exp floor -64)")

print()
print("== D. ring budget ==")
for sr in [44100, 48000, 96000, 192000]:
    cap_ms = 16384 / sr * 1000
    q1024_ms = 1024 / sr * 1000
    print(f"  sr={sr:7d} ring={cap_ms:7.1f} ms  quantum1024={q1024_ms:6.2f} ms  headroom={cap_ms/q1024_ms:5.1f}x")

print()
print("== E. decay_rate_norm (r) bounds ==")
for sr in [44100, 48000, 96000, 192000, 384000]:
    r = 44100 / max(sr, 1)
    print(f"  sr={sr:7d} r={r:.6f}  in (0, 1]: {0 < r <= 1.0}")

print()
print("== G. the FIX: certified ceiling 0.44 (night-cafe-mclaren-2) ==")
print("  arg = f32(f32(PI_f32 * f32(0.44*sr)) / sr) vs det_tan bound 0.45*pi")
FIXED = f32(0.44)
all_inside = True
for sr in [44100, 48000, 96000, 192000, 384000]:
    fc_cap = f32(FIXED * sr)
    arg = f32(f32(PI_F32 * fc_cap) / sr)
    inside = arg <= BOUND
    all_inside &= inside
    print(f"  sr={sr:7d} fc_cap={fc_cap!r:>12} arg_f64={arg!r:22} inside={inside}  margin={BOUND-arg:.4f}")
print(f"  -> every rate inside with a ~0.0314 rad margin (2.2% of the bound): {all_inside}")
assert all_inside, "the certified ceiling leaked"

print()
print("== H. shipped presets cannot reach the ceiling (bit-identity of the golden coefficients) ==")
CLICK_BREADTH = 2.5
max_click = 5500.0  # registry.rs authored click frequencies: 650..5500
for sr in [44100, 48000, 96000, 192000]:
    f_broad_old = min(max_click * CLICK_BREADTH, 0.45 * sr)
    f_broad_new = min(max_click * CLICK_BREADTH, 0.44 * sr)
    print(f"  sr={sr:7d} f_broad old={f_broad_old:.1f} new={f_broad_new:.1f} identical={f_broad_old == f_broad_new}")

print()
print("== F. release coefficient path ==")
for sr in [44100, 48000, 96000, 192000]:
    rel_samples = max(0.002 * sr, 1.0)
    x = 1.0 / rel_samples
    print(f"  sr={sr:7d} release_samples={rel_samples:9.1f}  det_pow(0.04, 1/N) arg x={x:.3e} in [0,64]: {0 <= x <= 64}")
