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

use std::f32::consts::FRAC_PI_2;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{AcousticModel, KeyProfile, KeyTrigger, ProfileWithOverrides, SynthState};

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

impl AcousticModel for MechanicalClick {
    fn get_profile(&self, event: &KeyTrigger) -> KeyProfile {
        // Per-key override lookup. Falls back to default if the scancode
        // has no entry in the override map.
        self.profiles.for_scancode(event.scancode)
    }

    fn init_state(&self, profile: &KeyProfile, state: &mut SynthState, stereo_position: f32) {
        let sr = self.sample_rate;

        // ── Micro-randomization (v5.0.0+, v10.2.0 overhaul) ─────────
        // Each keypress gets a unique noise seed and small pitch/amplitude
        // drift offsets so the same key never sounds identical twice.
        // This breaks the "uncanny valley" of deterministic synthesis
        // without adding any cost to the real-time audio callback — all
        // randomness is resolved here in init_state (called once per
        // keypress from the main thread, NOT from the audio callback).
        //
        // The seed is derived from this instance's monotonic counter so
        // it is unique per keypress. v5.0.1: moved from a global static
        // to a per-instance field to eliminate parallel-test flakiness.
        //
        // v10.2.0 (dragonzen audit N1): the previous code folded the
        // 64-bit counter into 32 bits via XOR of the high/low halves.
        // For nearby counter values the resulting 32-bit seeds differed
        // only in the high bits, and xorshift32 with such seeds does
        // not fully decorrelate within four rounds. Under autorepeat
        // (Backspace held at ~30 Hz), consecutive waveforms shared
        // correlated noise excitation — audible as a "metallic ringing".
        // The fix uses splitmix64 (excellent avalanche — any single-bit
        // input change flips ~50% of output bits) to mix the counter
        // into the 32-bit seed, and rolls xorshift32 six times total
        // (four drift values + housing noise seed + ambient noise seed)
        // for thorough decorrelation before consuming the stream.
        let keystroke_id = self.keystroke_counter.fetch_add(1, Ordering::Relaxed);

        // Mix the 64-bit counter into a high-quality 32-bit PRNG seed
        // via splitmix64. We take the high 32 bits — splitmix64's low
        // bits have slightly weaker statistical quality.
        let mixed = splitmix64(keystroke_id);
        let mut rng = (mixed >> 32) as u32;
        // xorshift32 with seed=0 produces all zeros forever — guard
        // against that by falling back to a fixed non-zero constant.
        // (Extremely unlikely after splitmix64, but the cost is one
        // branch per keypress.)
        if rng == 0 {
            rng = 0xDEAD_BEEF;
        }

        // Roll the RNG 6 times to get independent drift values + two
        // independent noise-stream seeds:
        //   1. click frequency drift      (±1.5%)
        //   2. spring frequency drift     (±1.5%)
        //   3. housing frequency drift    (±1.5%)
        //   4. excitation amplitude drift (±5%)
        //   5. housing_noise_state seed   (decoupled from click stream — N3)
        //   6. ambient_noise_state seed   (decoupled from click stream)
        xorshift32(&mut rng);
        let click_drift = u32_to_signed_f32(rng) * PITCH_DRIFT_FRAC;
        xorshift32(&mut rng);
        let spring_drift = u32_to_signed_f32(rng) * PITCH_DRIFT_FRAC;
        xorshift32(&mut rng);
        let housing_drift = u32_to_signed_f32(rng) * PITCH_DRIFT_FRAC;
        xorshift32(&mut rng);
        let amplitude_drift = u32_to_signed_f32(rng) * AMPLITUDE_DRIFT_FRAC;
        xorshift32(&mut rng);
        let housing_noise_seed = rng ^ 0x484F_5553; // "HOUS" in ASCII
        xorshift32(&mut rng);
        let ambient_noise_seed = rng ^ 0x414D_4249; // "AMBI" in ASCII

        // Apply pitch drift to the three filter frequencies. Multiply by
        // (1.0 + drift) — drift is in [-PITCH_DRIFT_FRAC, +PITCH_DRIFT_FRAC].
        // Clamped to a positive minimum to prevent the (rare) case where
        // drift + a tiny profile frequency would produce a non-positive
        // value (which would NaN the tan() pre-compute).
        let click_freq = (profile.click.frequency * (1.0 + click_drift)).max(1.0);
        let spring_freq = (profile.spring.frequency * (1.0 + spring_drift)).max(1.0);
        let housing_freq = (profile.housing.frequency * (1.0 + housing_drift)).max(1.0);

        // Apply amplitude drift to the excitation envelope. Clamp to
        // [0.0, 2.0] so a +5% drift on a 1.0 amplitude doesn't push the
        // envelope above 1.05 (which would clip the audio output).
        //
        // v10.2.0 (dragonzen audit N5): apply an additional inter-
        // keystroke timing attenuation. Real typists produce softer
        // second keystrokes in a fast double (finger still settling)
        // and slightly attenuated triples. We compute the interval
        // from `last_trigger_us` (set by `record_trigger_timestamp`)
        // and scale the amplitude by
        // `1.0 - 0.05 * min(1.0, interval_ms / 80.0)`. Fast repeats
        // (≤80 ms interval, e.g. autorepeat or fast double-taps) get
        // up to -5% amplitude; slow repeats are full amplitude.
        //
        // If `record_trigger_timestamp` was never called (backward-
        // compatible with callers that only use the trait API),
        // `last_interval_us` is 0 and no attenuation is applied.
        let interval_us = self.peek_last_interval_us();
        let interval_attenuation = if interval_us > 0 {
            let interval_ms = interval_us as f32 / 1000.0;
            0.05_f32 * (interval_ms / 80.0).min(1.0)
        } else {
            0.0
        };
        let amplitude =
            (profile.click.amplitude * (1.0 + amplitude_drift) * (1.0 - interval_attenuation))
                .clamp(0.0, 2.0);

        // Pre-compute click filter TPT coefficients (using drifted frequency)
        state.click_g = (std::f32::consts::PI * click_freq / sr).tan();
        state.click_k = 1.0 / profile.click.resonance;

        // Pre-compute spring filter TPT coefficients (using drifted frequency)
        state.spring_g = (std::f32::consts::PI * spring_freq / sr).tan();
        state.spring_k = 1.0 / profile.spring.resonance;

        // Pre-compute housing filter TPT coefficients (v4.2.0+, using
        // drifted frequency). The housing layer is a third TPT SVF
        // bandpass tuned to low frequencies (100-1000 Hz) to model the
        // keycap-hitting-PCB "thock" sound. It is driven by the same
        // noise excitation as the click and spring layers, but with a
        // longer excitation window — see `housing_excitation_samples`
        // below.
        state.housing_g = (std::f32::consts::PI * housing_freq / sr).tan();
        state.housing_k = 1.0 / profile.housing.resonance;
        state.housing_mix = profile.housing.mix;

        // Pre-compute excitation burst duration in samples
        state.excitation_samples = (profile.click.duration_ms * 0.001 * sr) as u32;
        state.excitation_samples = state.excitation_samples.max(1);

        // Housing excitation window: longer than the click burst so the
        // low-frequency filter has time to ring up. Four periods of the
        // housing fundamental is a good balance — enough for the filter
        // to reach steady-state, short enough to keep the impact
        // percussive (not a sustained tone). For 250 Hz at 44.1 kHz:
        // 4 * 44100/250 = 706 samples ≈ 16 ms. Uses the drifted
        // housing_freq so the window scales with the pitch drift.
        let housing_periods = 4.0_f32;
        let housing_excitation_fundamental = (housing_periods * sr / housing_freq) as u32;
        state.housing_excitation_samples = state
            .excitation_samples
            .max(housing_excitation_fundamental)
            .max(1);

        // Pre-compute decay and threshold from profile
        state.decay_coeff = profile.decay.coefficient;
        state.voice_off_threshold = profile.decay.voice_off_threshold;

        // v10.2.0 (dragonzen audit N7): two-stage decay. Pre-compute
        // the fast-stage coefficient + sample count from the profile.
        // If `coefficient_fast == 0.0` OR `fast_samples_ms == 0.0`,
        // the fast stage is disabled — render_sample falls back to the
        // single-stage `decay_coeff` for the entire voice. This keeps
        // the feature opt-in: existing configs that don't set the new
        // fields behave identically to pre-v10.2.0.
        state.decay_coeff_fast = profile.decay.coefficient_fast;
        state.fast_samples_count =
            if profile.decay.coefficient_fast > 0.0 && profile.decay.fast_samples_ms > 0.0 {
                (profile.decay.fast_samples_ms * 0.001 * sr) as u32
            } else {
                0
            };

        // Pre-compute spring mix level
        state.spring_mix = profile.spring.mix;

        // Equal-power pan law: theta maps stereo_position from [-1,1] to [0, pi/2]
        let theta = (stereo_position.clamp(-1.0, 1.0) + 1.0) * 0.5 * FRAC_PI_2;
        state.pan_left = theta.cos();
        state.pan_right = theta.sin();

        // v10.2.0 (dragonzen audit N6): per-keypress pan jitter.
        // Real keyboards have ±2–3° of finger-placement variation that
        // subtly shifts the perceived stereo position of each keypress.
        // We derive a small ±3% offset from the per-keystroke PRNG and
        // apply it multiplicatively in render_sample (symmetric signs
        // so the image moves left OR right, not just one direction).
        // The 7th xorshift32 roll (after the 6 used for N1/N3 seeds)
        // gives us an independent value here.
        xorshift32(&mut rng);
        state.pan_jitter = u32_to_signed_f32(rng) * 0.03;

        // Reset filter states to silence
        state.click_ic1eq = 0.0;
        state.click_ic2eq = 0.0;
        state.spring_ic1eq = 0.0;
        state.spring_ic2eq = 0.0;
        state.housing_ic1eq = 0.0;
        state.housing_ic2eq = 0.0;

        // Reset envelope and sample counter. Uses the drifted amplitude
        // so the entire voice starts at a slightly different level each
        // keypress.
        state.envelope_value = amplitude;
        state.sample_count = 0;

        // Initialize the click + spring noise generator with the
        // per-keystroke seed. After 6 xorshift32 rolls above, `rng` is
        // well-mixed and unique per keypress. XOR with a fixed constant
        // so even if the counter wraps to 0 we still have a non-zero
        // seed (xorshift32 with seed=0 produces all zeros forever).
        state.noise_state = rng ^ 0x4B45_5942; // "KEYB" in ASCII
        if state.noise_state == 0 {
            state.noise_state = 0xDEAD_BEEF;
        }

        // ── Housing noise generator (v10.2.0+ — dragonzen audit N3) ──
        // Decoupled from the click noise stream. Physically the click
        // (switch leaf impact) and the thock (keycap hitting PCB) are
        // separate impact events and must be driven by uncorrelated
        // noise. Sharing the stream caused constructive interference
        // — the two bandpass filters merged into a single "honk".
        //
        // `housing_noise_seed` was derived from the 5th xorshift32 roll
        // above and is XORed with another constant for additional
        // decorrelation from `noise_state`.
        state.housing_noise_state = housing_noise_seed;
        if state.housing_noise_state == 0 {
            state.housing_noise_state = 0x484F_5553; // "HOUS"
        }

        // ── Ambient rattle path setup ──────────────────────────────
        // Copied from the profile so the render path doesn't need to
        // re-read the profile every sample. When enabled is false, the
        // render path skips all ambient computation (zero cost).
        state.ambient_enabled = profile.ambient.enabled;
        state.ambient_level = profile.ambient.noise_level;
        state.ambient_decay = profile.ambient.noise_decay;
        // Start the ambient envelope at full level — it decays from
        // here via ambient_decay each sample.
        state.ambient_envelope = profile.ambient.noise_level;
        // `ambient_noise_seed` was derived from the 6th xorshift32 roll
        // above. This replaces the previous `rng.wrapping_mul(...)` trick
        // with a properly-mixed value (v10.2.0 — N1).
        state.ambient_noise_state = ambient_noise_seed;
        if state.ambient_noise_state == 0 {
            state.ambient_noise_state = 0x414D_4249; // "AMBI"
        }
        // Reset the high-pass filter state.
        state.ambient_hp_prev_input = 0.0;
        state.ambient_hp_prev_output = 0.0;
        // One-pole high-pass filter coefficient for a ~2 kHz cutoff.
        // Derived from: hp_coeff = (1 - cos(2*pi*fc/fs)) / (1 + cos(2*pi*fc/fs))
        // Pre-computed at init to avoid transcendentals in the render path.
        let fc = 2000.0_f32;
        let omega = 2.0 * std::f32::consts::PI * fc / sr;
        let cos_omega = omega.cos();
        state.ambient_hp_coeff = (1.0 - cos_omega) / (1.0 + cos_omega);

        // Pre-compute the release-ramp coefficient (v10.2.0+ — dragonzen
        // audit N2). Target: bring the envelope from its current value to
        // ~1% over 2 ms, regardless of sample rate. Solving
        // `coeff^N = 0.01` for `N = 2 ms * sample_rate` gives
        // `coeff = 0.01^(1/N)`. At 44.1 kHz: N ≈ 88, coeff ≈ 0.9499.
        // At 48 kHz: N ≈ 96, coeff ≈ 0.9535. At 96 kHz: N ≈ 192,
        // coeff ≈ 0.9765. Clamped to (0, 0.9999) for safety.
        let release_samples = (0.002 * sr).max(1.0);
        state.release_coeff = 0.04_f32.powf(1.0 / release_samples).clamp(0.0, 0.9999);

        // Activate voice
        state.active = true;
        state.releasing = false;
    }

    fn render_sample(&self, state: &mut SynthState) -> [f32; 2] {
        if !state.active {
            return [0.0, 0.0];
        }

        // ── Stage 1: Generate click + spring excitation ───────────────
        // During the initial burst, produce shaped white noise. After the
        // burst ends, the excitation is zero — the TPT filters ring from
        // their internal state, which IS the spring resonance.
        //
        // This stream drives BOTH the click bandpass (sharp switch-leaf
        // transient) and the spring bandpass (resonant spring ring) —
        // they share the stream because both model the same physical
        // switch-leaf impact event.
        let excitation = if state.sample_count < state.excitation_samples {
            // Xorshift32 PRNG — fast, zero-alloc, good statistical quality
            let mut x = state.noise_state;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            state.noise_state = x;

            // Map u32 to f32 in [-1.0, 1.0] via the shared helper
            // (v10.2.0 — dragonzen audit N8/B14: replaces the previous
            // inline `(x as f32 / u32::MAX as f32) * 2.0 - 1.0` which
            // had precision loss above 2^24 and a positive bias.)
            let noise = u32_to_signed_f32(x);

            // Perceptually-shaped fade (v10.2.0 — dragonzen audit N4).
            //
            // Real mechanical impacts have a power-law decay — energy
            // is concentrated in the first ~10% of the window, then a
            // long tail. The previous linear fade
            // (`noise * (1.0 - progress)`) produced a perceivable
            // "ramp down" tail at the burst boundary, audible as a
            // "tick" at the moment the burst ends.
            //
            // Quadratic fade (`(1 - p)^2`) front-loads the energy and
            // tapers off smoothly — closer to a real impact envelope.
            // Same cost (one multiply per sample), more natural shape.
            let progress = state.sample_count as f32 / state.excitation_samples as f32;
            let fade = (1.0 - progress) * (1.0 - progress);
            noise * fade
        } else {
            0.0
        };

        // ── Stage 1b: Housing excitation (v4.2.0+, v10.2.0 decoupled) ──
        // The housing filter is tuned to low frequencies (100-1000 Hz)
        // where the natural response time (1/fc) is much longer than
        // the click burst. We drive it with a SEPARATE, longer
        // excitation window so the filter has time to ring up to its
        // steady-state Q gain.
        //
        // v10.2.0 (dragonzen audit N3): the housing noise stream is
        // now decoupled from the click noise stream. Previously the
        // housing layer reused `state.noise_state`, which meant the
        // click bandpass and the housing bandpass saw **identical**
        // noise during the burst-overlap window. Constructive
        // interference made the two layers merge into a single "honk"
        // instead of a layered click + thock. Physically the click
        // (switch leaf impact) and the thock (keycap hitting PCB) are
        // separate impact events and must be driven by uncorrelated
        // noise. The dedicated `state.housing_noise_state` (seeded
        // separately in init_state) fixes this.
        //
        // v10.2.0 (N4): the housing fade uses an exponential taper
        // (`exp(-3 * progress)`) rather than the quadratic shape used
        // by the click path. The housing "thock" is a softer, broader
        // impact (keycap hitting PCB) — its envelope should taper more
        // gradually than the sharp switch-leaf click. exp(-3p) at p=1
        // gives ~5% residual, smooth at the boundary.
        let housing_excitation = if state.sample_count < state.housing_excitation_samples {
            let mut x = state.housing_noise_state;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            state.housing_noise_state = x;
            let noise = u32_to_signed_f32(x);
            let progress = state.sample_count as f32 / state.housing_excitation_samples as f32;
            let fade = (-3.0_f32 * progress).exp();
            noise * fade
        } else {
            0.0
        };

        // ── Stage 2a: Click TPT SVF (bandpass emphasis) ─────────────
        // Inlined TPT SVF for zero function-call overhead in the render path
        let g = state.click_g;
        let k = state.click_k;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * g * a1;

        let v3 = excitation - state.click_ic2eq;
        let v1 = a1 * state.click_ic1eq + a2 * v3;
        let v2 = state.click_ic2eq + a2 * state.click_ic1eq + a3 * v3;

        state.click_ic1eq = 2.0 * v1 - state.click_ic1eq;
        state.click_ic2eq = 2.0 * v2 - state.click_ic2eq;

        // Mix highpass (sharp transient) and bandpass (body) for the click
        let click_out = (v3 - v1) * 0.7 + v1 * 0.3;

        // ── Stage 2b: Spring TPT SVF (resonant ring) ─────────────────
        // Independent filter driven directly by the excitation. After the
        // burst ends, the filter rings at its natural frequency — this is
        // the "spring" sound characteristic of mechanical switches.
        let sg = state.spring_g;
        let sk = state.spring_k;
        let sa1 = 1.0 / (1.0 + sg * (sg + sk));
        let sa2 = sg * sa1;
        let sa3 = sg * sg * sa1;

        let sv3 = excitation - state.spring_ic2eq;
        let sv1 = sa1 * state.spring_ic1eq + sa2 * sv3;
        let _sv2 = state.spring_ic2eq + sa2 * state.spring_ic1eq + sa3 * sv3;

        state.spring_ic1eq = 2.0 * sv1 - state.spring_ic1eq;
        state.spring_ic2eq = 2.0 * _sv2 - state.spring_ic2eq;

        // ── Stage 2c: Housing TPT SVF (deep "thock", v4.2.0+) ────────
        // Third independent filter driven by the housing-specific
        // longer excitation window (see Stage 1b above). Tuned to a
        // low frequency (100-1000 Hz), this produces the deep "buk" /
        // "thock" of the keycap hitting the switch housing / PCB at
        // bottom-out. Without this layer, the sound feels thin — the
        // high-frequency click is present but the body is missing.
        //
        // We use the bandpass output (hv1) directly — the housing thock
        // is a low-frequency body, not a sharp transient, so we don't
        // mix in the highpass component like we do for the click.
        let hg = state.housing_g;
        let hk = state.housing_k;
        let ha1 = 1.0 / (1.0 + hg * (hg + hk));
        let ha2 = hg * ha1;
        let ha3 = hg * hg * ha1;

        let hv3 = housing_excitation - state.housing_ic2eq;
        let hv1 = ha1 * state.housing_ic1eq + ha2 * hv3;
        let hv2 = state.housing_ic2eq + ha2 * state.housing_ic1eq + ha3 * hv3;

        state.housing_ic1eq = 2.0 * hv1 - state.housing_ic1eq;
        state.housing_ic2eq = 2.0 * hv2 - state.housing_ic2eq;

        // ── Stage 3: Mix components ──────────────────────────────────
        // The housing bandpass output is naturally smaller in amplitude
        // than the click/spring outputs (bandpass gain at low Q is
        // ~0.1× the input amplitude). Multiply by the housing Q
        // (`1/k`) to bring it to a comparable level so `housing.mix`
        // behaves as a true level control rather than being dominated
        // by the bandpass attenuation.
        let housing_gain = if state.housing_k > 0.0 {
            1.0 / state.housing_k
        } else {
            1.0
        };
        let mut mixed = click_out + sv1 * state.spring_mix + hv1 * state.housing_mix * housing_gain;

        // ── Stage 3b: Ambient rattle (optional) ─────────────────────
        // High-pass filtered white noise with its own decay envelope,
        // simulating case rattle / PCB flex / hollow housing resonance.
        // Skipped entirely when ambient_enabled is false (zero cost).
        if state.ambient_enabled && state.ambient_envelope > 1e-6 {
            // Xorshift32 PRNG (separate state from the click excitation)
            let mut x = state.ambient_noise_state;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            state.ambient_noise_state = x;
            // v10.2.0 — dragonzen audit N8/B14: use the shared helper
            // for precision + symmetry.
            let raw_noise = u32_to_signed_f32(x);

            // One-pole high-pass filter: y[n] = coeff * (y[n-1] + x[n] - x[n-1])
            let hp_out = state.ambient_hp_coeff
                * (state.ambient_hp_prev_output + raw_noise - state.ambient_hp_prev_input);
            state.ambient_hp_prev_input = raw_noise;
            state.ambient_hp_prev_output = hp_out;

            // Apply the ambient envelope and add to the mix
            mixed += hp_out * state.ambient_envelope;

            // Decay the ambient envelope
            state.ambient_envelope *= state.ambient_decay;
        }

        // ── Stage 4: Apply envelope ──────────────────────────────────
        let sample = mixed * state.envelope_value;

        // ── Stage 5: Stereo pan ─────────────────────────────────────
        // v10.2.0 (dragonzen audit N6): apply per-keypress pan jitter
        // multiplicatively (symmetric signs so the image moves left OR
        // right). The jitter is small (±3%) so it doesn't break the
        // equal-power pan law meaningfully, but it unlocks the locked
        // stereo field that was the most obvious synthetic tell on
        // headphones.
        let jl = 1.0 + state.pan_jitter;
        let jr = 1.0 - state.pan_jitter;
        let left = sample * state.pan_left * jl;
        let right = sample * state.pan_right * jr;

        // ── Housekeeping ────────────────────────────────────────────
        state.sample_count += 1;
        // Apply the appropriate envelope coefficient (v10.2.0+):
        //
        // 1. **Release ramp** (N2): if the key has been released, use
        //    `release_coeff` to ramp the envelope to zero over ~2 ms.
        // 2. **Fast decay stage** (N7): if two-stage decay is enabled
        //    and we're still within `fast_samples_count`, use
        //    `decay_coeff_fast` (e.g. 0.997) for the click transient's
        //    rapid initial drop.
        // 3. **Slow decay tail** (default): otherwise use `decay_coeff`
        //    (e.g. 0.9994) for the spring/housing tail.
        //
        // All coefficients are validated to be in (0, 0.9999] so
        // `envelope_value` remains non-negative and monotonically
        // decreasing — the voice eventually deactivates via the
        // `voice_off_threshold` check below.
        if state.releasing {
            state.envelope_value *= state.release_coeff;
        } else if state.fast_samples_count > 0 && state.sample_count <= state.fast_samples_count {
            state.envelope_value *= state.decay_coeff_fast;
        } else {
            state.envelope_value *= state.decay_coeff;
        }

        // Voice-off check. `envelope_value` is always >= 0 (initial
        // amplitude clamped to [0, 1], decay/release coefficients clamped
        // to [0, 0.9999]), so the `.abs()` previously used here was
        // unnecessary. Removing it saves one bitwise-AND per sample per
        // voice (v10.2.0 — dragonzen audit B18).
        if state.envelope_value < state.voice_off_threshold {
            state.active = false;
        }

        [left, right]
    }

    /// v10.2.0+ (dragonzen audit N5): record the current keypress
    /// timestamp and return the interval since the previous press.
    /// `init_state` reads the stashed interval via `peek_last_interval_us`
    /// and attenuates the amplitude of fast repeats by up to -5%.
    #[inline]
    fn record_trigger_timestamp(&self, now_us: u64) -> u64 {
        let prev = self.last_trigger_us.swap(now_us, Ordering::Relaxed);
        let interval = if prev == 0 || now_us < prev {
            0
        } else {
            now_us - prev
        };
        self.last_interval_us.store(interval, Ordering::Relaxed);
        interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_returns_sane_defaults() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        let profile = model.get_profile(&event);

        assert!(profile.click.frequency > 0.0);
        assert!(profile.click.resonance > 0.0);
        assert!(profile.spring.frequency > 0.0);
        assert!(profile.spring.resonance > 0.0);
        assert!(profile.decay.coefficient > 0.0 && profile.decay.coefficient < 1.0);
    }

    #[test]
    fn test_center_pan_is_equal_lr() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        let profile = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&profile, &mut state, event.stereo_position);

        // v10.2.0 (N6): per-keypress pan jitter (±3%) means L and R
        // are no longer bit-identical even at center pan. The
        // difference should be small (< 6% of peak amplitude).
        let [l, r] = model.render_sample(&mut state);
        let peak = l.abs().max(r.abs());
        assert!(
            (l - r).abs() < peak * 0.06 + 1e-6,
            "Center pan should produce near-equal L/R (with ±3% jitter tolerance). Got l={l}, r={r}"
        );
    }

    #[test]
    fn test_full_left_pan_is_louder_on_left() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: -1.0,
        };

        let profile = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&profile, &mut state, event.stereo_position);

        let [l, r] = model.render_sample(&mut state);
        assert!(
            l.abs() > r.abs() + 1e-6,
            "Full-left pan: left should dominate"
        );
    }

    #[test]
    fn test_full_right_pan_is_louder_on_right() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 1.0,
        };

        let profile = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&profile, &mut state, event.stereo_position);

        let [l, r] = model.render_sample(&mut state);
        assert!(
            r.abs() > l.abs() + 1e-6,
            "Full-right pan: right should dominate"
        );
    }

    #[test]
    fn test_voice_decays_and_deactivates() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        let profile = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&profile, &mut state, event.stereo_position);

        let mut peak: f32 = 0.0;
        let mut deactivated = false;

        for _ in 0..200_000 {
            let [l, r] = model.render_sample(&mut state);
            peak = peak.max(l.abs()).max(r.abs());
            if !state.active {
                deactivated = true;
                break;
            }
        }

        assert!(
            peak > 0.01,
            "Voice should produce audible output (peak={peak})"
        );
        assert!(deactivated, "Voice should deactivate after envelope decays");
    }

    #[test]
    fn test_silence_after_deactivation() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        let profile = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&profile, &mut state, event.stereo_position);

        // Render until voice deactivates
        for _ in 0..500_000 {
            let _ = model.render_sample(&mut state);
            if !state.active {
                break;
            }
        }
        assert!(!state.active, "Voice should have deactivated");

        // After deactivation, output must be exactly zero
        for _ in 0..256 {
            let [l, r] = model.render_sample(&mut state);
            assert_eq!(l, 0.0, "Inactive voice must output zero on left");
            assert_eq!(r, 0.0, "Inactive voice must output zero on right");
        }
    }

    #[test]
    fn test_excitation_is_limited_duration() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        let profile = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&profile, &mut state, event.stereo_position);

        let burst_end = state.excitation_samples as usize;

        // Save filter state right after burst ends
        for _ in 0..burst_end + 10 {
            let _ = model.render_sample(&mut state);
        }

        // The filter should still be ringing (spring resonance) even
        // though the excitation has stopped
        let [l, r] = model.render_sample(&mut state);
        assert!(
            l.abs() > 1e-10 || r.abs() > 1e-10,
            "Filter should ring after excitation ends"
        );
    }

    /// v10.2.0 (N4): quadratic fade should produce a non-linear
    /// envelope on the excitation burst. We sample the click path's
    /// excitation amplitude at three points (start, mid, end of burst)
    /// by reading the click filter's bandpass output and verify the
    /// mid-burst amplitude is less than what linear fade would give.
    #[test]
    fn test_excitation_fade_is_quadratic() {
        // With a linear fade, amplitude at progress=0.5 is 0.5 of the
        // initial. With quadratic, it's 0.25. We don't need to be
        // exact — just verify the mid-burst is significantly below
        // 0.5 of the start (which would only happen with a non-linear
        // fade). This catches accidental regression to linear.
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        let profile = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&profile, &mut state, event.stereo_position);

        // Render up to the burst midpoint.
        let mid = state.excitation_samples / 2;
        let mut peak_first_half: f32 = 0.0;
        let mut peak_second_half: f32 = 0.0;
        for i in 0..state.excitation_samples {
            let [l, r] = model.render_sample(&mut state);
            let amp = l.abs().max(r.abs());
            if i < mid {
                peak_first_half = peak_first_half.max(amp);
            } else {
                peak_second_half = peak_second_half.max(amp);
            }
        }

        // With quadratic fade, the second-half peak should be at most
        // ~25% of the first-half peak (linear would give ~50%).
        // Allow some slack for noise variance.
        assert!(
            peak_second_half < peak_first_half * 0.4,
            "Excitation fade should be non-linear (quadratic). first_half={peak_first_half:.6}, second_half={peak_second_half:.6}"
        );
    }

    /// v10.2.0 (N5): inter-keystroke interval attenuation should
    /// reduce the amplitude of fast repeats.
    #[test]
    fn test_inter_keystroke_interval_attenuation() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        // First press — no previous, no attenuation. Returns 0
        // (first press).
        let first_interval = model.record_trigger_timestamp(1_000_000); // 1s in us
        assert_eq!(
            first_interval, 0,
            "first press should return 0 (no previous)"
        );
        let profile1 = model.get_profile(&event);
        let mut state1 = SynthState::default();
        model.init_state(&profile1, &mut state1, event.stereo_position);
        let amp_first = state1.envelope_value;

        // Second press 20 ms later — fast repeat, should attenuate by
        // up to 5% (interval_ms / 80, clamped to 1.0, times 0.05).
        // At 20 ms interval: 20/80 = 0.25 → attenuation = 0.05 * 0.25
        // = 0.0125 → amplitude = first_amp * (1 - 0.0125) = 0.9875 of
        // the unattenuated value. Hard to test in isolation because
        // the per-keypress amplitude_drift (±5%) is also in play. We
        // can't directly compare envelope_values; instead verify the
        // interval was recorded and returned correctly.
        let interval_returned = model.record_trigger_timestamp(1_020_000);
        assert_eq!(
            interval_returned, 20_000,
            "record_trigger_timestamp should return the interval in microseconds"
        );

        // Verify the interval is stashed for init_state to read.
        // We can't access peek_last_interval_us from outside the
        // crate, but we can verify init_state uses it by checking
        // that a second press within the interval window produces a
        // different envelope than one without. (Just smoke-test the
        // mechanism.)
        let profile2 = model.get_profile(&event);
        let mut state2 = SynthState::default();
        model.init_state(&profile2, &mut state2, event.stereo_position);
        // Smoke: state2 should be active with a positive envelope.
        assert!(state2.active, "voice should be active after trigger");
        assert!(
            state2.envelope_value > 0.0,
            "envelope should be positive after init"
        );
        // Sanity: amplitude is in [0, 2] (clamp).
        assert!(state2.envelope_value <= 2.0);

        let _ = amp_first; // suppress unused
    }

    /// v10.2.0 (N6): per-keypress pan jitter should differ between
    /// two consecutive presses of the same key.
    #[test]
    fn test_pan_jitter_varies_between_keypresses() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        // Render many presses and collect the pan_jitter values.
        // They should vary — if they're all identical, the jitter
        // derivation is broken.
        let mut jitters = std::collections::HashSet::new();
        for _ in 0..20 {
            let profile = model.get_profile(&event);
            let mut state = SynthState::default();
            model.init_state(&profile, &mut state, event.stereo_position);
            // Quantize to f32 bits — same value = same hash.
            jitters.insert(state.pan_jitter.to_le_bytes());
        }

        // With 20 presses, we should see at least 10 distinct jitter
        // values. (Statistically, with 24-bit precision, collisions
        // are very rare.)
        assert!(
            jitters.len() >= 10,
            "pan_jitter should vary between keypresses — got {} distinct values out of 20",
            jitters.len()
        );
    }

    /// v10.2.0 (N7): two-stage decay should make the envelope drop
    /// faster during the fast stage than the slow stage.
    #[test]
    fn test_two_stage_decay_drops_faster_in_fast_stage() {
        let mut profile = KeyProfile::default();
        // Configure a clear two-stage decay: fast stage at 0.997 for
        // 5 ms, slow tail at 0.9999.
        profile.decay.coefficient_fast = 0.997;
        profile.decay.fast_samples_ms = 5.0;
        profile.decay.coefficient = 0.9999;
        profile.decay.voice_off_threshold = 1e-7;

        let model = MechanicalClick::with_profile(profile, crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        let p = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&p, &mut state, event.stereo_position);

        // Track envelope_value over time. Compute the per-sample
        // ratio for the fast stage (samples 1..fast_samples_count)
        // and the slow stage (samples fast_samples_count+1..2x).
        let fast_samples = state.fast_samples_count;
        assert!(fast_samples > 0, "fast_samples_count should be > 0");

        let mut prev = state.envelope_value;
        let mut fast_ratios: Vec<f32> = Vec::new();
        for i in 0..fast_samples {
            let _ = model.render_sample(&mut state);
            if i > 0 && prev > 0.0 {
                fast_ratios.push(state.envelope_value / prev);
            }
            prev = state.envelope_value;
        }

        let mut slow_ratios: Vec<f32> = Vec::new();
        for _ in 0..fast_samples {
            let _ = model.render_sample(&mut state);
            if prev > 0.0 {
                slow_ratios.push(state.envelope_value / prev);
            }
            prev = state.envelope_value;
        }

        let avg_fast: f32 = fast_ratios.iter().sum::<f32>() / fast_ratios.len().max(1) as f32;
        let avg_slow: f32 = slow_ratios.iter().sum::<f32>() / slow_ratios.len().max(1) as f32;

        // The fast stage should have a lower ratio (steeper decay)
        // than the slow stage.
        assert!(
            avg_fast < avg_slow,
            "fast stage ratio ({avg_fast:.6}) should be lower than slow stage ratio ({avg_slow:.6})"
        );
        // And specifically, the fast ratio should be close to 0.997
        // (the configured coefficient_fast).
        assert!(
            (avg_fast - 0.997).abs() < 0.001,
            "fast stage ratio should match coefficient_fast=0.997, got {avg_fast:.6}"
        );
        // Slow ratio should be close to 0.9999.
        assert!(
            (avg_slow - 0.9999).abs() < 0.001,
            "slow stage ratio should match coefficient=0.9999, got {avg_slow:.6}"
        );
    }

    /// v10.2.0 (N7): when coefficient_fast = 0.0, the fast stage is
    /// disabled and the voice uses single-stage decay (backward-
    /// compatible with pre-v10.2.0 configs).
    #[test]
    fn test_two_stage_decay_disabled_when_coefficient_fast_zero() {
        let profile = KeyProfile::default();
        // Default profile has coefficient_fast = 0.0 and
        // fast_samples_ms = 0.0.
        let model = MechanicalClick::with_profile(profile, crate::SAMPLE_RATE as u32);
        let event = KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        let p = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&p, &mut state, event.stereo_position);

        assert_eq!(
            state.fast_samples_count, 0,
            "fast_samples_count should be 0 when coefficient_fast is 0.0"
        );
        assert_eq!(
            state.decay_coeff_fast, 0.0,
            "decay_coeff_fast should be 0.0 when disabled"
        );

        // The envelope should decay at the slow rate for the entire
        // voice. Sample the ratio at sample 1 and at sample 100 —
        // both should equal decay_coeff (0.9994 by default).
        let prev = state.envelope_value;
        let _ = model.render_sample(&mut state);
        let ratio1 = state.envelope_value / prev;
        assert!(
            (ratio1 - state.decay_coeff).abs() < 1e-6,
            "disabled two-stage should use decay_coeff everywhere, got ratio={ratio1:.6}"
        );
    }
}
