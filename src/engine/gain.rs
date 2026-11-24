// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The gain-law core (night-netrunner-Z2): the volume cell, the
//! percent-of-default gain law, and the PRNG's signed-float mapping,
//! extracted to ONE crate-path-free module so the detached proofs
//! package can include the SAME bytes the binary ships (the fuzz
//! harnesses' include law, one directory over — a law the proofs
//! verify is a law the datapath runs).
//!
//! The extraction is a pure move: every function's body is the exact
//! bytes its former site carried, and every former site now imports
//! from here. The module is std-only and imports nothing from the
//! crate, so `#[path]` inclusion from outside compiles standalone.

/// The compiled laptop default gain (5.5x) — the value 100 percent of
/// the control map's volume cell names. Lives here beside the clamp
/// law so the proofs package's theorem and the binary's constant stay
/// one symbol (the include law), not a restatement.
pub const DEFAULT_MASTER_VOLUME: f32 = 5.5;

/// The pure volume clamp: finite values clamp to [-100, 100],
/// everything else falls back to the compiled default. Stored as
/// f32 bits for the atomic cell. PUB (night-hunter-1) because the
/// player's control-plane watcher retunes through the SAME cell —
/// every writer of the volume atomic must ride this one law, or the
/// startup lane and the live-retune lane disagree about what a map
/// value means (they did: the watcher stored raw bits, so a forced
/// retune above ~1818% outranged what a fresh `on` would have clamped
/// to).
pub fn volume_bits(volume: f32) -> u32 {
    let clamped = if volume.is_finite() {
        volume.clamp(-100.0, 100.0)
    } else {
        DEFAULT_MASTER_VOLUME
    };
    clamped.to_bits()
}

/// The percent-of-default gain law: the control map carries a percent,
/// the pool wants a linear gain. 100 percent means the compiled laptop
/// default (5.5x); 0 percent is silence. Lives here (night-netrunner-Z2)
/// beside the volume cell it feeds, so the detached proofs package
/// verifies the SAME bytes the watcher writes.
pub fn gain_from_percent(percent: u32) -> f32 {
    percent as f32 / 100.0 * DEFAULT_MASTER_VOLUME
}

/// Map a u32 to f32 in `[-1.0, 1.0]`. Used to derive signed drift values
/// from the xorshift32 PRNG output, and to map raw PRNG output to signed
/// noise in the render path.
///
/// # Implementation notes (v10.2.0 — dragonzen audit N8/B14)
///
/// Two defects existed in the previous version
/// (`(x as f32 / u32::MAX as f32) * 2.0 - 1.0`):
///
/// 1. **Precision loss above 2^24.** `x as f32` quantizes any u32 > 16 777
///    215 to a multiple of 2. For PRNG outputs that span the full u32
///    range, the upper half of the distribution became chunky.
/// 2. **Asymmetric range.** `u32::MAX as f32` rounds up to 4 294 967
///    296.0 (2^32), so the division produced a value < 1.0 even for
///    `x = u32::MAX`. The signed result was therefore biased toward
///    the negative side, and `+1.0` was unreachable.
///
/// The fix uses the upper 24 bits (full f32 precision) divided by
/// `16 777 215.0` (2^24 - 1). This produces a symmetric `[-1.0, +1.0]`
/// range with uniform quantization across the entire distribution.
/// (night-hunter-1: the divisor rode `8 388 607.0` (2^23 - 1) with the
/// same `* 2.0 - 1.0` tail — half the needed divisor, so the top of
/// the range landed at +3.0 and every consumer (noise excitation,
/// drift, pan jitter) carried a +1.0 DC bias. The divisor is the
/// domain's own max: 2^24 - 1, exactly representable in f32, endpoint
/// +1.0 exact.)
///
/// night-netrunner-Z2 moved the function here from
/// `profiles::mechanical` (a pure move) so the proofs package's
/// theorem rides the included bytes; the Kani harnesses' visibility
/// comment (pub(crate) for `src/engine/kani_proofs.rs`) is retired —
/// the module is `pub` now, the detached package's posture.
pub fn u32_to_signed_f32(x: u32) -> f32 {
    ((x >> 8) as f32 / 16_777_215.0) * 2.0 - 1.0
}
