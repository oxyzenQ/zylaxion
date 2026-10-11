// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The render-path state and the acoustic-model trait.
//!
//! Split from the crate root in night-improve-18 (the no-mercy LOC
//! law). Every path is re-exported at the crate root, so external
//! users see no change.

use crate::{KeyProfile, KeyTrigger};

/// Mutable DSP state for a single voice instance.
///
/// This struct is zero-alloc and designed to live on the stack or within a
/// pre-allocated array. It is passed to [`AcousticModel::render_sample`] on
/// every sample tick and persists across calls.
///
/// All filter coefficients and pre-computed values are stored here so that
/// the render path never needs to call `tan()` or perform division.
#[derive(Debug, Clone)]
pub struct SynthState {
    // --- TPT SVF filter state (click path) ---
    /// First integrator state for the click filter.
    pub click_ic1eq: f32,
    /// Second integrator state for the click filter.
    pub click_ic2eq: f32,
    /// Pre-computed `tan(pi * fc / fs)` for the click filter.
    pub click_g: f32,
    /// Pre-computed `1 / Q` for the click filter.
    pub click_k: f32,

    // --- TPT SVF filter state (spring path) ---
    /// First integrator state for the spring filter.
    pub spring_ic1eq: f32,
    /// Second integrator state for the spring filter.
    pub spring_ic2eq: f32,
    /// Pre-computed `tan(pi * fc / fs)` for the spring filter.
    pub spring_g: f32,
    /// Pre-computed `1 / Q` for the spring filter.
    pub spring_k: f32,

    // --- TPT SVF filter state (housing path, v4.2.0+) ---
    /// First integrator state for the housing filter.
    pub housing_ic1eq: f32,
    /// Second integrator state for the housing filter.
    pub housing_ic2eq: f32,
    /// Pre-computed `tan(pi * fc / fs)` for the housing filter.
    pub housing_g: f32,
    /// Pre-computed `1 / Q` for the housing filter.
    pub housing_k: f32,
    /// Pre-computed housing mix level (from profile).
    pub housing_mix: f32,
    /// Pre-computed housing excitation length in samples.
    ///
    /// The housing filter is tuned to low frequencies (100-1000 Hz)
    /// where the filter's natural response time (`1/fc`) is much
    /// longer than the click excitation burst (1-3 ms). Driving a
    /// 250 Hz bandpass with a 2 ms noise burst produces almost no
    /// output — the filter has no time to ring up.
    ///
    /// To fix this, the housing layer gets its own longer excitation
    /// window, sized as `max(click_excitation, sample_rate / fc * 4)`
    /// — i.e. four periods of the housing fundamental. For a 250 Hz
    /// housing at 44100 Hz, this is `4 * 44100/250 = 706 samples ≈ 16 ms`,
    /// long enough for the filter to ring up to its steady-state Q
    /// gain before the excitation fades.
    pub housing_excitation_samples: u32,

    // --- Excitation noise generator (xorshift32) ---
    /// Internal state of the xorshift32 PRNG used for click + spring noise
    /// excitation. Drives the click path (bandpass) and the spring path
    /// (resonant bandpass) — they share this stream because both model the
    /// same physical switch-leaf impact event.
    pub noise_state: u32,
    /// Internal state of the xorshift32 PRNG used for the housing "thock"
    /// excitation (v10.2.0+ — dragonzen audit N3).
    ///
    /// Previously the housing layer reused `noise_state`, which meant the
    /// click bandpass and the housing bandpass saw **identical** noise
    /// during the burst-overlap window. Constructive interference made the
    /// two layers merge into a single "honk" instead of a layered click +
    /// thock. Physically the click (switch leaf impact) and the thock
    /// (keycap hitting PCB) are separate impact events and should be driven
    /// by uncorrelated noise. This separate state fixes that.
    pub housing_noise_state: u32,

    // --- Envelope ---
    /// Current envelope amplitude value. Always `>= 0.0` — initialized from
    /// `ClickParams::amplitude` (clamped to `[0.0, 1.0]`) and decayed
    /// multiplicatively by `decay_coeff` (clamped to `[0.0, 0.9999]`).
    /// The non-negativity invariant is relied upon by the voice-off test
    /// (`envelope_value < voice_off_threshold` — no `.abs()` needed).
    pub envelope_value: f32,
    /// Number of samples rendered since the voice was triggered.
    pub sample_count: u32,
    /// Duration of the noise excitation burst in samples.
    pub excitation_samples: u32,
    /// Pre-computed per-sample decay coefficient (from profile).
    pub decay_coeff: f32,
    /// Pre-computed per-sample decay coefficient for the fast stage
    /// (v10.2.0+ — N7). 0.0 disables the fast stage (single-stage
    /// decay). When > 0.0 and `sample_count < fast_samples_count`,
    /// `render_sample` applies this coefficient instead of `decay_coeff`.
    pub decay_coeff_fast: f32,
    /// Number of samples in the fast (initial) decay stage (v10.2.0+ —
    /// N7). 0 disables the fast stage. After this many samples the
    /// voice switches to `decay_coeff` for the slow tail.
    pub fast_samples_count: u32,
    /// Pre-computed voice-off threshold (from profile).
    pub voice_off_threshold: f32,

    // --- Stereo pan gains (pre-computed) ---
    /// Left channel gain from equal-power pan law.
    pub pan_left: f32,
    /// Right channel gain from equal-power pan law.
    pub pan_right: f32,
    /// Pre-computed spring mix level (from profile).
    pub spring_mix: f32,
    /// Per-keypress stereo pan jitter, in the range [-0.03, +0.03]
    /// (v10.2.0+ — dragonzen audit N6).
    ///
    /// Real keyboards have ±2–3° of finger-placement variation that
    /// subtly shifts the perceived stereo position of each keypress.
    /// Without this, every press of "A" pans to exactly the same
    /// position — the locked stereo field is the most obvious
    /// synthetic tell on headphones.
    ///
    /// `init_state` derives this from the per-keystroke PRNG and
    /// applies it multiplicatively in `render_sample`:
    ///   `pan_left *= (1.0 + pan_jitter)`
    ///   `pan_right *= (1.0 - pan_jitter)`
    /// (note the symmetric signs — jitter moves the image left OR
    /// right, not just one direction).
    pub pan_jitter: f32,

    // --- Ambient rattle path (optional, enabled per-profile) ---
    /// Whether the ambient rattle path is active for this voice.
    /// Copied from `KeyProfile::ambient::enabled` at init time. When
    /// `false`, the render path skips all ambient computation (zero
    /// cost).
    pub ambient_enabled: bool,
    /// Pre-computed peak amplitude of the ambient noise burst (from
    /// `KeyProfile::ambient::noise_level`).
    pub ambient_level: f32,
    /// Pre-computed per-sample decay coefficient for the ambient
    /// envelope (from `KeyProfile::ambient::noise_decay`).
    pub ambient_decay: f32,
    /// Current ambient envelope amplitude value. Decays per-sample via
    /// `ambient_decay`. When it falls below `ambient_level * 1e-4`,
    /// ambient generation stops for this voice.
    pub ambient_envelope: f32,
    /// Separate xorshift32 state for the ambient noise generator so
    /// it doesn't correlate with the click excitation noise.
    pub ambient_noise_state: u32,
    /// High-pass filter state for the ambient noise (one-pole).
    /// Stores the previous input sample for the difference equation
    /// `y[n] = x[n] - x[n-1] + hp_coeff * y[n-1]`.
    pub ambient_hp_prev_input: f32,
    /// High-pass filter state for the ambient noise (one-pole).
    /// Stores the previous output sample.
    pub ambient_hp_prev_output: f32,
    /// Pre-computed high-pass filter coefficient. Derived from a fixed
    /// cutoff (e.g. 2 kHz) at the sample rate.
    pub ambient_hp_coeff: f32,

    // --- Voice lifecycle ---
    /// Whether this voice is currently producing audio.
    pub active: bool,
    /// Whether the key has been released and the voice is in its fast
    /// release-ramp phase (v10.2.0+ — dragonzen audit N2).
    ///
    /// When `false`, the voice is in the normal decay phase (slow
    /// exponential decay via `decay_coeff`). When `true`, the voice
    /// applies a much faster multiplicative coefficient
    /// (`release_coeff`) to ramp the envelope to zero over ~2 ms.
    /// This replaces the previous behavior of hard-cutting
    /// `active = false` on key release, which produced a 1-sample
    /// discontinuity audible as a "click off" tell.
    ///
    /// The release ramp is initialized to a coefficient that brings
    /// the envelope from its current value to ~1% over 2 ms (~88
    /// samples at 44.1 kHz). At the end of the ramp the voice
    /// deactivates naturally via the normal `voice_off_threshold`
    /// check.
    pub releasing: bool,
    /// Pre-computed per-sample release-ramp coefficient (v10.2.0+).
    /// Computed in `init_state` from the sample rate so the release
    /// ramp duration is ~2 ms regardless of the audio device's
    /// sample rate.
    pub release_coeff: f32,
}

impl Default for SynthState {
    fn default() -> Self {
        Self {
            click_ic1eq: 0.0,
            click_ic2eq: 0.0,
            click_g: 0.0,
            click_k: 0.0,
            spring_ic1eq: 0.0,
            spring_ic2eq: 0.0,
            spring_g: 0.0,
            spring_k: 0.0,
            housing_ic1eq: 0.0,
            housing_ic2eq: 0.0,
            housing_g: 0.0,
            housing_k: 0.0,
            housing_mix: 0.4,
            housing_excitation_samples: 0,
            noise_state: 1,
            housing_noise_state: 1,
            envelope_value: 0.0,
            sample_count: 0,
            excitation_samples: 0,
            decay_coeff: 0.9994,
            decay_coeff_fast: 0.0,
            fast_samples_count: 0,
            voice_off_threshold: 1e-5,
            pan_left: std::f32::consts::FRAC_1_SQRT_2,
            pan_right: std::f32::consts::FRAC_1_SQRT_2,
            spring_mix: 0.6,
            pan_jitter: 0.0,
            ambient_enabled: false,
            ambient_level: 0.0,
            ambient_decay: 0.99,
            ambient_envelope: 0.0,
            ambient_noise_state: 1,
            ambient_hp_prev_input: 0.0,
            ambient_hp_prev_output: 0.0,
            ambient_hp_coeff: 0.0,
            active: false,
            releasing: false,
            release_coeff: 0.9,
        }
    }
}

/// Trait for defining how a keyboard key sounds.
///
/// Implementers provide the DSP logic that transforms key events into audio
/// samples. The trait is designed for zero-allocation render paths: all
/// mutable state lives in [`SynthState`], which is allocated once per voice
/// and reused across sample ticks.
///
/// # Lifecycle
///
/// 1. [`get_profile`](Self::get_profile) is called once when a voice is triggered.
/// 2. [`init_state`](Self::init_state) pre-computes coefficients from the profile.
/// 3. [`render_sample`](Self::render_sample) is called per-sample until the
///    voice deactivates itself (sets `state.active = false`).
pub trait AcousticModel: Send + Sync {
    /// Return the acoustic profile for a given key event.
    ///
    /// Called once when a voice is triggered. The returned [`KeyProfile`] is
    /// cached by the voice pool and used to initialize the voice state.
    fn get_profile(&self, event: &KeyTrigger) -> KeyProfile;

    /// Initialize the synthesis state from a profile.
    ///
    /// Called once before the first [`render_sample`](Self::render_sample) call.
    /// Implementers should pre-compute all filter coefficients, pan gains, and
    /// timing values here so that `render_sample` is free of division and
    /// transcendentals.
    fn init_state(
        &self,
        profile: &KeyProfile,
        state: &mut SynthState,
        stereo_position: f32,
        velocity: Option<f32>,
    );

    /// Render a single stereo sample.
    ///
    /// Called once per sample tick for each active voice. **Must not allocate.**
    ///
    /// # Returns
    /// A stereo sample `[left, right]` in `[-1.0, 1.0]`.
    fn render_sample(&self, state: &mut SynthState) -> [f32; 2];

    /// Record the monotonic timestamp of a keypress and return the
    /// interval since the previous keypress (v10.2.0+ — dragonzen
    /// audit N5).
    ///
    /// The orchestrator should call this exactly once per keypress,
    /// BEFORE `init_state`. The returned interval (microseconds) is
    /// used by `init_state` to attenuate the amplitude of fast repeats
    /// (≤80 ms interval) by up to -5%, simulating the natural softening
    /// of repeated keypresses when the finger is still settling.
    ///
    /// The default implementation is a no-op (returns 0) — models that
    /// don't care about inter-keystroke timing don't need to override
    /// it. `MechanicalClick` overrides it to track `last_trigger_us`.
    ///
    /// # Arguments
    ///
    /// * `monotonic_us` — current keypress timestamp in microseconds,
    ///   from the same clock source as `KeyEvent::timestamp`
    ///   (CLOCK_MONOTONIC on Linux).
    ///
    /// # Returns
    ///
    /// The interval in microseconds since the previous keypress, or 0
    /// if there was no previous keypress (first press after construction
    /// or after a baseline reset).
    #[inline]
    fn record_trigger_timestamp(&self, _monotonic_us: u64) -> u64 {
        0
    }
}
