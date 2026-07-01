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

use crate::{AcousticModel, KeyEvent, KeyProfile, ProfileWithOverrides, SynthState};

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
/// from the xorshift32 PRNG output.
#[inline]
fn u32_to_signed_f32(x: u32) -> f32 {
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
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
    }
}

impl Default for MechanicalClick {
    fn default() -> Self {
        Self::new(crate::SAMPLE_RATE as u32)
    }
}

impl AcousticModel for MechanicalClick {
    fn get_profile(&self, event: &KeyEvent) -> KeyProfile {
        // Per-key override lookup. Falls back to default if the scancode
        // has no entry in the override map.
        self.profiles.for_scancode(event.scancode)
    }

    fn init_state(&self, profile: &KeyProfile, state: &mut SynthState, stereo_position: f32) {
        let sr = self.sample_rate;

        // ── Micro-randomization (v5.0.0+) ───────────────────────────
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
        let keystroke_id = self.keystroke_counter.fetch_add(1, Ordering::Relaxed);

        // Fold the 64-bit counter into a 32-bit PRNG seed. xor + shift
        // gives good bit-mixing; we don't need cryptographic quality,
        // just uniqueness and uniform distribution.
        let mut rng = (keystroke_id as u32) ^ ((keystroke_id >> 32) as u32);
        // xorshift32 with seed=0 produces all zeros forever — guard
        // against that by falling back to a fixed non-zero constant.
        if rng == 0 {
            rng = 0xDEAD_BEEF;
        }

        // Roll the RNG 4 times to get independent drift values:
        //   1. click frequency drift   (±1.5%)
        //   2. spring frequency drift  (±1.5%)
        //   3. housing frequency drift (±1.5%)
        //   4. excitation amplitude drift (±5%)
        xorshift32(&mut rng);
        let click_drift = u32_to_signed_f32(rng) * PITCH_DRIFT_FRAC;
        xorshift32(&mut rng);
        let spring_drift = u32_to_signed_f32(rng) * PITCH_DRIFT_FRAC;
        xorshift32(&mut rng);
        let housing_drift = u32_to_signed_f32(rng) * PITCH_DRIFT_FRAC;
        xorshift32(&mut rng);
        let amplitude_drift = u32_to_signed_f32(rng) * AMPLITUDE_DRIFT_FRAC;

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
        let amplitude = (profile.click.amplitude * (1.0 + amplitude_drift)).clamp(0.0, 2.0);

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

        // Pre-compute spring mix level
        state.spring_mix = profile.spring.mix;

        // Equal-power pan law: theta maps stereo_position from [-1,1] to [0, pi/2]
        let theta = (stereo_position.clamp(-1.0, 1.0) + 1.0) * 0.5 * FRAC_PI_2;
        state.pan_left = theta.cos();
        state.pan_right = theta.sin();

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

        // Initialize the noise generator with the per-keystroke seed.
        // After 4 xorshift32 rolls above, `rng` is well-mixed and
        // unique per keypress. XOR with a fixed constant so even if the
        // counter wraps to 0 we still have a non-zero seed (xorshift32
        // with seed=0 produces all zeros forever).
        state.noise_state = rng ^ 0x4B45_5942; // "KEYB" in ASCII
        if state.noise_state == 0 {
            state.noise_state = 0xDEAD_BEEF;
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
        // Separate seed from the click noise generator so the two
        // noise streams don't correlate. Also derive from the
        // per-keystroke counter so ambient noise varies per keypress
        // (v5.0.0+).
        state.ambient_noise_state = (rng.wrapping_mul(0x85EB_CA6B)) ^ 0xCAFE_BABE;
        if state.ambient_noise_state == 0 {
            state.ambient_noise_state = 0xCAFEBABE;
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

        // Activate voice
        state.active = true;
    }

    fn render_sample(&self, state: &mut SynthState) -> [f32; 2] {
        if !state.active {
            return [0.0, 0.0];
        }

        // ── Stage 1: Generate excitation ──────────────────────────────
        // During the initial burst, produce shaped white noise. After the
        // burst ends, the excitation is zero — the TPT filters ring from
        // their internal state, which IS the spring resonance.
        let excitation = if state.sample_count < state.excitation_samples {
            // Xorshift32 PRNG — fast, zero-alloc, good statistical quality
            let mut x = state.noise_state;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            state.noise_state = x;

            // Map u32 to f32 in [-1.0, 1.0]
            let noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;

            // Linear fade within the burst to avoid a hard cutoff that
            // would inject an artificial click at the burst boundary
            let progress = state.sample_count as f32 / state.excitation_samples as f32;
            noise * (1.0 - progress)
        } else {
            0.0
        };

        // ── Stage 1b: Housing excitation (v4.2.0+) ───────────────────
        // The housing filter is tuned to low frequencies (100-1000 Hz)
        // where the natural response time (1/fc) is much longer than
        // the click burst. We drive it with a SEPARATE, longer
        // excitation window so the filter has time to ring up to its
        // steady-state Q gain. The PRNG is shared with the click path
        // (no separate seed needed — the windows overlap and the noise
        // stream is the same), but the fade envelope is computed
        // against the housing's own sample count.
        let housing_excitation = if state.sample_count < state.housing_excitation_samples {
            // Reuse the current noise_state without advancing it again
            // — the click path already advanced it this sample. If we
            // are past the click burst, advance it ourselves.
            let noise = if state.sample_count < state.excitation_samples {
                // Click path already advanced noise_state this sample.
                // Re-derive the same noise value from the current state.
                // (Saves a PRNG step and keeps the noise streams
                // correlated — which is fine, since both filters see
                // the same physical impact event.)
                let x = state.noise_state;
                (x as f32 / u32::MAX as f32) * 2.0 - 1.0
            } else {
                // Click burst is over — advance the PRNG ourselves.
                let mut x = state.noise_state;
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                state.noise_state = x;
                (x as f32 / u32::MAX as f32) * 2.0 - 1.0
            };
            let progress = state.sample_count as f32 / state.housing_excitation_samples as f32;
            noise * (1.0 - progress)
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
            let raw_noise = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;

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
        let left = sample * state.pan_left;
        let right = sample * state.pan_right;

        // ── Housekeeping ────────────────────────────────────────────
        state.sample_count += 1;
        state.envelope_value *= state.decay_coeff;

        if state.envelope_value.abs() < state.voice_off_threshold {
            state.active = false;
        }

        [left, right]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_returns_sane_defaults() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyEvent {
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
        let event = KeyEvent {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
        };

        let profile = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&profile, &mut state, event.stereo_position);

        let [l, r] = model.render_sample(&mut state);
        assert!((l - r).abs() < 1e-6, "Center pan should produce equal L/R");
    }

    #[test]
    fn test_full_left_pan_is_louder_on_left() {
        let model = MechanicalClick::new(crate::SAMPLE_RATE as u32);
        let event = KeyEvent {
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
        let event = KeyEvent {
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
        let event = KeyEvent {
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
        let event = KeyEvent {
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
        let event = KeyEvent {
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
}
