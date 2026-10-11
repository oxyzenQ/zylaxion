// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Default mechanical keyboard click model.
//!
//! [`MechanicalClick`] is the reference [`AcousticModel`](crate::AcousticModel)
//! implementation for standard mechanical key switches (Cherry MX style).
//!
//! # Synthesis Chain
//!
//! The sound is generated through three stages, all using pre-computed
//! coefficients so the per-sample render path contains no division or
//! transcendentals:
//!
//! 1. **Excitation** — A short burst (1–3 ms) of shaped white noise simulates
//!    the physical impact of the keycap stem hitting the switch housing. A
//!    linear fade-out within the burst prevents a hard cutoff artifact.
//!
//! 2. **Dual TPT SVF filtering** — Two independent TPT State Variable Filters
//!    run in parallel:
//!    - *Click filter* (bandpass): extracts the sharp attack transient from the
//!      noise burst. Output is a mix of the highpass (70%) and bandpass (30%)
//!      for a crisp, percussive character.
//!    - *Spring filter* (bandpass): tuned lower to model the spring snapping
//!      back and the keycap housing vibrating. This filter continues to ring
//!      naturally after the excitation ends — the ringing IS the spring sound.
//!
//! 3. **Exponential decay envelope** — A per-sample multiplicative envelope
//!    (`value *= coeff`) models the natural energy dissipation of the mechanical
//!    system. When the amplitude falls below a threshold, the voice deactivates.
//!
//! # Micro-Randomization (v5.0.0+)
//!
//! Real mechanical keyboards never produce identical waveforms from one
//! keypress to the next — spring tolerances, keycap mass variance, and
//! human force variation all introduce micro-changes. Without this
//! variation, synthesized keyboard sound falls into the "uncanny valley"
//! of perceptible determinism (the brain notices the repeats).
//!
//! Since v5.0.0, [`MechanicalClick::init_state`] applies three forms of
//! per-keystroke randomness, all resolved ONCE at trigger time (never in
//! the real-time audio callback):
//!
//! - **Noise seed variation**: the xorshift32 PRNG seed is derived from
//!   a per-instance monotonic counter, so the noise burst is never
//!   identical.
//! - **Pitch drift** (±1.5%): tiny random offsets applied to click /
//!   spring / housing frequencies. Matches real switch tolerance.
//! - **Amplitude drift** (±5%): tiny random offset applied to the
//!   excitation envelope. Matches human force variation.
//!
//! # Why per-instance, not global? (v5.0.1+)
//!
//! v5.0.0 used a global `static AtomicU64` counter. This caused flaky
//! test failures when multiple tests ran in parallel: test A would
//! reset the counter, then test B's `init_state` call would increment
//! it before test A's trigger, giving test A's voice an unexpected
//! seed. v5.0.1 moves the counter to a per-instance field on
//! `MechanicalClick`. In production there is only one instance (held
//! behind an `ArcSwap` in the orchestrator), so behavior is identical.
//! In tests, each instance has its own counter, so they don't interfere.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::{KeyProfile, ProfileWithOverrides};

/// Maximum pitch drift applied to click/spring/housing frequency, as a
/// fraction of the profile value. ±1.5% matches the natural variation
/// of real mechanical switches (spring tolerances, keycap mass variance,
/// temperature-induced stiffness changes).
const PITCH_DRIFT_FRAC: f32 = 0.015;

/// Maximum amplitude drift applied to the excitation envelope, as a
/// fraction of the profile value. ±5% matches the natural variation
/// of how hard a user presses a key from one keystroke to the next
/// (typing cadence, finger fatigue, hand position drift).
const AMPLITUDE_DRIFT_FRAC: f32 = 0.05;

/// Xorshift32 step. Used both in the render path (existing) and here in
/// `init_state` to derive per-keystroke random offsets. Inlined for
/// consistency with the render path.
#[inline]
fn xorshift32(x: &mut u32) {
    *x ^= *x << 13;
    *x ^= *x >> 17;
    *x ^= *x << 5;
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
/// 1. **Precision loss above 2²⁴.** `x as f32` quantizes any u32 > 16 777
///    215 to a multiple of 2. For PRNG outputs that span the full u32
///    range, the upper half of the distribution became chunky.
/// 2. **Asymmetric range.** `u32::MAX as f32` rounds up to 4 294 967
///    296.0 (2³²), so the division produced a value < 1.0 even for
///    `x = u32::MAX`. The signed result was therefore biased toward
///    the negative side, and `+1.0` was unreachable.
///
/// The fix uses the upper 24 bits (full f32 precision) divided by
/// `8 388 607.0` (2²³ − 1). This produces a symmetric `[-1.0, +1.0]`
/// range with uniform quantization across the entire distribution.
#[inline]
fn u32_to_signed_f32(x: u32) -> f32 {
    ((x >> 8) as f32 / 8_388_607.0) * 2.0 - 1.0
}

/// Splitmix64 step. Used to derive a high-quality 32-bit PRNG seed from
/// the per-instance 64-bit keystroke counter.
///
/// # Why splitmix64 instead of xor-fold (v10.2.0 — dragonzen audit N1)
///
/// The previous code derived the seed by folding the 64-bit counter
/// into 32 bits via XOR of the high and low halves, then rolling
/// xorshift32 four times to derive the four drift values. xorshift32
/// with seeds that differ only in the high bits (which is what the XOR
/// fold produced for nearby counter values) does not fully decorrelate
/// within four rounds — the output bit-patterns remain structurally
/// correlated. Under autorepeat (Backspace held at ~30 Hz), consecutive
/// waveforms shared correlated noise excitation, audible as a
/// "metallic ringing" — exactly the uncanny-valley artifact v5.0.0 was
/// designed to kill.
///
/// splitmix64 is a 64-bit mixing function with excellent avalanche
/// properties: any single-bit change in the input flips ~50% of output
/// bits. We then take the high 32 bits (better statistical quality than
/// the low bits in splitmix64's output) as the xorshift32 seed.
#[inline]
fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Default acoustic model for mechanical keyboard switches.
///
/// Generates a short click/spring sound using filtered noise excitation and
/// exponential decay, typical of Cherry MX-style mechanical switches. This
/// implementation is fully procedural — no wavetable samples are used.
///
/// # Profile lookup
///
/// A `MechanicalClick` holds a [`ProfileWithOverrides`] — a default
/// [`KeyProfile`] plus an optional per-scancode override map. When a key
/// is pressed, [`get_profile`](AcousticModel::get_profile) checks the
/// override map first; if no entry exists for the scancode, the default
/// profile is returned. This allows users to give specific keys (e.g.
/// Space, Enter) distinct sounds via `[[keys]]` blocks in the TOML.
///
/// # Hot-reload
///
/// The model is `Send + Sync` and immutable once constructed. Hot-reload
/// is implemented at the orchestrator level by swapping the entire
/// `MechanicalClick` instance behind an `ArcSwap` — active voices
/// continue rendering with their cached profile (captured at trigger
/// time), while new keypresses pick up the new model automatically.
pub struct MechanicalClick {
    profiles: ProfileWithOverrides,
    /// The actual sample rate of the audio device (e.g. 44100, 48000,
    /// 96000). Used in `init_state` to compute filter coefficients and
    /// excitation burst durations so the DSP math is accurate at any
    /// sample rate — no resampling overhead.
    sample_rate: f32,
    /// Per-instance monotonic counter used to seed the per-keypress
    /// micro-randomization (v5.0.1+).
    ///
    /// Each `init_state` call increments this and uses the new value to
    /// derive a unique noise seed + pitch/amplitude drift offsets. This
    /// breaks the "uncanny valley" of deterministic synthesis — pressing
    /// the same key 10 times produces 10 slightly different waveforms,
    /// just like a real physical keyboard.
    ///
    /// # Why per-instance, not global? (v5.0.1)
    ///
    /// v5.0.0 used a global `static AtomicU64` counter. This caused
    /// flaky test failures when multiple tests ran in parallel: test A
    /// would reset the counter, then test B's `init_state` call would
    /// increment it before test A's trigger, giving test A's voice an
    /// unexpected seed. Moving the counter to a per-instance field
    /// eliminates the race — each `MechanicalClick` instance has its
    /// own counter, so tests creating separate instances don't
    /// interfere. In production there is only one instance (held
    /// behind an `ArcSwap` in the orchestrator), so behavior is
    /// identical to the global-counter approach.
    ///
    /// Uses `Relaxed` ordering because we only need uniqueness, not
    /// synchronization with other memory operations. The counter is u64
    /// so it will not wrap in any realistic runtime (~584 years at 1
    /// billion keypresses per second).
    keystroke_counter: AtomicU64,
    /// Monotonic microsecond timestamp of the previous keypress, used
    /// to compute inter-keystroke interval for the v10.2.0 timing
    /// variation (dragonzen audit N5).
    ///
    /// Initialized to 0 (sentinel for "no previous press"). Each
    /// `init_state` call reads the current value, computes `now - prev`
    /// to get the interval, then stores `now` for the next call. Uses
    /// `Relaxed` ordering — we only need monotonicity of the read+write
    /// pair, not synchronization with other memory operations.
    ///
    /// The orchestrator passes a microsecond timestamp from
    /// `monotonic_us()` (same source as `KeyEvent::timestamp`). The
    /// model itself has no concept of wall-clock time, so the caller
    /// must supply it via `init_state_at` (or accept the default
    /// behavior of `init_state`, which uses 0 — i.e. "no previous
    /// press" — for backward compatibility with the trait API).
    last_trigger_us: AtomicU64,
    /// Inter-keystroke interval (microseconds) recorded by the most
    /// recent `record_trigger_timestamp` call. Read by `init_state` to
    /// attenuate the amplitude of fast repeats (v10.2.0+ — N5).
    ///
    /// This is a side-channel because `init_state`'s trait signature
    /// has no parameter for the interval — we can't change the trait
    /// without breaking other `AcousticModel` implementations.
    last_interval_us: AtomicU64,
}

impl MechanicalClick {
    /// Create a new `MechanicalClick` model with default parameters and
    /// no per-key overrides, using the given `sample_rate` (Hz) for all
    /// DSP coefficient calculations.
    #[inline]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            profiles: ProfileWithOverrides {
                default: KeyProfile::default(),
                overrides: std::collections::HashMap::new(),
            },
            sample_rate: sample_rate as f32,
            keystroke_counter: AtomicU64::new(1),
            last_trigger_us: AtomicU64::new(0),
            last_interval_us: AtomicU64::new(0),
        }
    }

    /// Create a `MechanicalClick` model with a custom default
    /// [`KeyProfile`] and no per-key overrides.
    #[inline]
    pub fn with_profile(profile: KeyProfile, sample_rate: u32) -> Self {
        Self {
            profiles: ProfileWithOverrides {
                default: profile,
                overrides: std::collections::HashMap::new(),
            },
            sample_rate: sample_rate as f32,
            keystroke_counter: AtomicU64::new(1),
            last_trigger_us: AtomicU64::new(0),
            last_interval_us: AtomicU64::new(0),
        }
    }

    /// Create a `MechanicalClick` model from a loaded
    /// [`ProfileWithOverrides`] (default + per-key overrides).
    #[inline]
    pub fn with_overrides(profiles: ProfileWithOverrides, sample_rate: u32) -> Self {
        Self {
            profiles,
            sample_rate: sample_rate as f32,
            keystroke_counter: AtomicU64::new(1),
            last_trigger_us: AtomicU64::new(0),
            last_interval_us: AtomicU64::new(0),
        }
    }

    /// Reset this instance's keystroke counter to 1.
    ///
    /// This is a **test-only** helper. In production code, the counter
    /// should monotonically increase forever — calling this from a real
    /// audio path would make consecutive keypresses produce identical
    /// waveforms, defeating the entire purpose of v5.0.0's
    /// micro-randomization.
    ///
    /// # Why per-instance? (v5.0.1)
    ///
    /// Because the counter is now per-instance (not global), resetting
    /// it only affects THIS `MechanicalClick` instance — other instances
    /// (e.g. in parallel tests) are unaffected. This eliminates the
    /// flaky-test race that v5.0.0's global counter caused.
    #[doc(hidden)]
    pub fn reset_keystroke_counter_for_tests(&self) {
        self.keystroke_counter.store(1, Ordering::Relaxed);
        // Also reset the timing baseline so tests don't see a phantom
        // large interval on the first keypress.
        self.last_trigger_us.store(0, Ordering::Relaxed);
        self.last_interval_us.store(0, Ordering::Relaxed);
    }

    /// Read the inter-keystroke interval recorded by the most recent
    /// `record_trigger_timestamp` call. Used internally by `init_state`.
    /// Returns 0 if `record_trigger_timestamp` was never called (backward-
    /// compatible with callers that only use the trait API).
    #[inline]
    fn peek_last_interval_us(&self) -> u64 {
        self.last_interval_us.load(Ordering::Relaxed)
    }
}

impl Default for MechanicalClick {
    fn default() -> Self {
        Self::new(crate::SAMPLE_RATE as u32)
    }
}

mod render;

#[cfg(test)]
#[path = "../../test/mechanical_tests.rs"]
mod tests;
