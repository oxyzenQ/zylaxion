// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The per-key acoustic profile: the validated DSP table and the
//! safety guardrails.
//!
//! Split from the crate root in night-improve-18 (the no-mercy LOC
//! law). Every path is re-exported at the crate root, so external
//! users see no change.

use serde::{Deserialize, Serialize};

use crate::{AmbientParams, ClickParams, DecayParams, HousingParams, SpringParams};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct KeyProfile {
    /// Click transient parameters.
    #[serde(default)]
    pub click: ClickParams,
    /// Spring resonance parameters.
    #[serde(default)]
    pub spring: SpringParams,
    /// Decay envelope parameters.
    #[serde(default)]
    pub decay: DecayParams,
    /// Ambient case-rattle parameters.
    #[serde(default)]
    pub ambient: AmbientParams,
    /// Housing "thock" parameters (v4.2.0+).
    #[serde(default)]
    pub housing: HousingParams,
}

/// Safe DSP parameter ranges.
///
/// These are enforced by [`KeyProfile::validate_and_clamp`] when loading
/// user-supplied TOML profiles. The hardcoded default profile bypasses
/// validation (it is constructed in code and known to be safe).
///
/// # Why these bounds
///
/// - `click_freq` / `spring_freq`: 500–8000 Hz keeps the bandpass within
///   the audible range and prevents the TPT `tan(pi * fc / fs)` pre-compute
///   from saturating at Nyquist.
/// - `resonance` (Q factor): 0.1–10.0. The render path computes
///   `k = 1.0 / resonance`, so:
///   - `resonance = 0.0` would divide by zero (panic).
///   - `resonance < 0.1` produces a very wide filter that lets through
///     too much noise (click loses its percussive character).
///   - `resonance > 10.0` produces a very narrow filter that becomes
///     numerically unstable in single-precision TPT SVF (state variables
///     can grow unbounded).
/// - `decay_coefficient`: 0.0–0.9999. MUST be strictly less than 1.0 to
///   prevent infinite loops (a coefficient >= 1.0 means the envelope never
///   decays, so the voice never deactivates and consumes a polyphony slot
///   forever).
/// - `amplitude` / `mix`: 0.0–1.0 are sane audible ranges.
/// - `duration_ms`: 0.1–10.0 ms covers the shortest mechanical tick to
///   the longest practical spring ring-out burst.
/// - `voice_off_threshold`: 1e-7–1e-2. Too small and voices never
///   deactivate; too large and they cut off audibly.
pub mod ranges {
    pub const CLICK_FREQ_MIN: f32 = 500.0;
    pub const CLICK_FREQ_MAX: f32 = 8000.0;

    pub const SPRING_FREQ_MIN: f32 = 500.0;
    pub const SPRING_FREQ_MAX: f32 = 8000.0;

    /// Minimum resonance (Q). Below this the filter is too wide.
    pub const RESONANCE_MIN: f32 = 0.1;
    /// Maximum resonance (Q). Above this the TPT SVF becomes numerically
    /// unstable in single-precision float.
    pub const RESONANCE_MAX: f32 = 10.0;

    pub const AMPLITUDE_MIN: f32 = 0.0;
    pub const AMPLITUDE_MAX: f32 = 1.0;

    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;

    pub const DURATION_MS_MIN: f32 = 0.1;
    pub const DURATION_MS_MAX: f32 = 10.0;

    pub const DECAY_COEFF_MIN: f32 = 0.0;
    pub const DECAY_COEFF_MAX: f32 = 0.9999;

    /// Two-stage decay fast-stage coefficient (v10.2.0+ — N7).
    /// Same constraints as `DECAY_COEFF`: must be < 1.0 to prevent
    /// infinite loops. A value of 0.0 disables the fast stage
    /// (single-stage decay, backward-compatible).
    pub const DECAY_COEFF_FAST_MIN: f32 = 0.0;
    pub const DECAY_COEFF_FAST_MAX: f32 = 0.9999;

    /// Two-stage decay fast-stage duration in milliseconds (v10.2.0+ —
    /// N7). 0.0 disables the fast stage. Capped at 50 ms — beyond
    /// this the "fast" stage is no longer a transient.
    pub const DECAY_FAST_SAMPLES_MS_MIN: f32 = 0.0;
    pub const DECAY_FAST_SAMPLES_MS_MAX: f32 = 50.0;

    pub const VOICE_OFF_THRESHOLD_MIN: f32 = 1e-7;
    pub const VOICE_OFF_THRESHOLD_MAX: f32 = 1e-2;

    /// Ambient noise level range. 0.0 = silent, 1.0 = full.
    pub const AMBIENT_NOISE_LEVEL_MIN: f32 = 0.0;
    pub const AMBIENT_NOISE_LEVEL_MAX: f32 = 1.0;

    /// Ambient noise decay. Same constraint as DECAY_COEFF: must be < 1.0
    /// to prevent infinite rattle.
    pub const AMBIENT_NOISE_DECAY_MIN: f32 = 0.0;
    pub const AMBIENT_NOISE_DECAY_MAX: f32 = 0.9999;

    /// Housing "thock" frequency range (v4.2.0+).
    ///
    /// 100 Hz is the lower bound of perceived "body" — below this the
    /// housing impact blends into sub-bass rumble and loses its
    /// percussive character. 1000 Hz is the upper bound — above this
    /// the housing layer starts overlapping with the spring layer's
    /// frequency range and the two become indistinguishable.
    pub const HOUSING_FREQ_MIN: f32 = 100.0;
    pub const HOUSING_FREQ_MAX: f32 = 1000.0;
}

/// Helper: clamp a value into `[min, max]`, treating NaN and infinities
/// as out-of-bounds. Returns `(clamped_value, was_clamped)`.
///
/// - `NaN` is replaced by `min`.
/// - `+inf` is replaced by `max`.
/// - `-inf` is replaced by `min`.
/// - Finite values outside the range are clamped to the nearest bound.
fn clamp_finite(value: f32, min: f32, max: f32) -> (f32, bool) {
    if value.is_nan() {
        return (min, true);
    }
    if value.is_infinite() {
        return (if value > 0.0 { max } else { min }, true);
    }
    if value < min {
        return (min, true);
    }
    if value > max {
        return (max, true);
    }
    (value, false)
}

impl KeyProfile {
    /// Validate and clamp all DSP parameters to safe ranges.
    ///
    /// This is the **guardrail** between user-supplied TOML configs and
    /// the real-time DSP render path. Without it, a user typing
    /// `decay = 9999` or `decay = infinity` in their profile would
    /// cause the TPT filter to blow up (audio cracking) or the voice
    /// envelope to never decay (CPU hang in an infinite render loop).
    ///
    /// # Behaviour
    ///
    /// - `NaN`, `Infinity`, and out-of-bounds values are silently clamped
    ///   to the nearest safe boundary.
    /// - Each clamped parameter emits a `log::warn!` line identifying
    ///   the field, the offending value, and the clamped replacement.
    /// - The method is idempotent: running it on an already-valid
    ///   profile is a no-op.
    ///
    /// # When to call
    ///
    /// - **Always** when loading from a TOML file (`load_*` functions).
    /// - **Never** on the hardcoded default profile (it is constructed
    ///   in code and already safe — calling this is harmless but
    ///   wasteful).
    pub fn validate_and_clamp(&mut self) {
        // Click parameters
        let (v, c) = clamp_finite(
            self.click.frequency,
            ranges::CLICK_FREQ_MIN,
            ranges::CLICK_FREQ_MAX,
        );
        if c {
            log::warn!(
                "Invalid click.frequency {}, clamping to {}",
                self.click.frequency,
                v
            );
        }
        self.click.frequency = v;

        let (v, c) = clamp_finite(
            self.click.resonance,
            ranges::RESONANCE_MIN,
            ranges::RESONANCE_MAX,
        );
        if c {
            log::warn!(
                "Invalid click.resonance {}, clamping to {}",
                self.click.resonance,
                v
            );
        }
        self.click.resonance = v;

        let (v, c) = clamp_finite(
            self.click.duration_ms,
            ranges::DURATION_MS_MIN,
            ranges::DURATION_MS_MAX,
        );
        if c {
            log::warn!(
                "Invalid click.duration_ms {}, clamping to {}",
                self.click.duration_ms,
                v
            );
        }
        self.click.duration_ms = v;

        let (v, c) = clamp_finite(
            self.click.amplitude,
            ranges::AMPLITUDE_MIN,
            ranges::AMPLITUDE_MAX,
        );
        if c {
            log::warn!(
                "Invalid click.amplitude {}, clamping to {}",
                self.click.amplitude,
                v
            );
        }
        self.click.amplitude = v;

        // Spring parameters
        let (v, c) = clamp_finite(
            self.spring.frequency,
            ranges::SPRING_FREQ_MIN,
            ranges::SPRING_FREQ_MAX,
        );
        if c {
            log::warn!(
                "Invalid spring.frequency {}, clamping to {}",
                self.spring.frequency,
                v
            );
        }
        self.spring.frequency = v;

        let (v, c) = clamp_finite(
            self.spring.resonance,
            ranges::RESONANCE_MIN,
            ranges::RESONANCE_MAX,
        );
        if c {
            log::warn!(
                "Invalid spring.resonance {}, clamping to {}",
                self.spring.resonance,
                v
            );
        }
        self.spring.resonance = v;

        let (v, c) = clamp_finite(self.spring.mix, ranges::MIX_MIN, ranges::MIX_MAX);
        if c {
            log::warn!("Invalid spring.mix {}, clamping to {}", self.spring.mix, v);
        }
        self.spring.mix = v;

        // Decay parameters — the critical ones for infinite-loop prevention
        let (v, c) = clamp_finite(
            self.decay.coefficient,
            ranges::DECAY_COEFF_MIN,
            ranges::DECAY_COEFF_MAX,
        );
        if c {
            log::warn!(
                "Invalid decay.coefficient {}, clamping to {} (must be < 1.0 to prevent infinite loops)",
                self.decay.coefficient,
                v
            );
        }
        self.decay.coefficient = v;

        let (v, c) = clamp_finite(
            self.decay.voice_off_threshold,
            ranges::VOICE_OFF_THRESHOLD_MIN,
            ranges::VOICE_OFF_THRESHOLD_MAX,
        );
        if c {
            log::warn!(
                "Invalid decay.voice_off_threshold {}, clamping to {}",
                self.decay.voice_off_threshold,
                v
            );
        }
        self.decay.voice_off_threshold = v;

        // Two-stage decay fast-stage parameters (v10.2.0+ — N7).
        let (v, c) = clamp_finite(
            self.decay.coefficient_fast,
            ranges::DECAY_COEFF_FAST_MIN,
            ranges::DECAY_COEFF_FAST_MAX,
        );
        if c {
            log::warn!(
                "Invalid decay.coefficient_fast {}, clamping to {} (must be < 1.0 to prevent infinite loops; 0.0 disables the fast stage)",
                self.decay.coefficient_fast,
                v
            );
        }
        self.decay.coefficient_fast = v;

        let (v, c) = clamp_finite(
            self.decay.fast_samples_ms,
            ranges::DECAY_FAST_SAMPLES_MS_MIN,
            ranges::DECAY_FAST_SAMPLES_MS_MAX,
        );
        if c {
            log::warn!(
                "Invalid decay.fast_samples_ms {}, clamping to {} (0.0 disables the fast stage)",
                self.decay.fast_samples_ms,
                v
            );
        }
        self.decay.fast_samples_ms = v;

        // Ambient parameters
        let (v, c) = clamp_finite(
            self.ambient.noise_level,
            ranges::AMBIENT_NOISE_LEVEL_MIN,
            ranges::AMBIENT_NOISE_LEVEL_MAX,
        );
        if c {
            log::warn!(
                "Invalid ambient.noise_level {}, clamping to {}",
                self.ambient.noise_level,
                v
            );
        }
        self.ambient.noise_level = v;

        let (v, c) = clamp_finite(
            self.ambient.noise_decay,
            ranges::AMBIENT_NOISE_DECAY_MIN,
            ranges::AMBIENT_NOISE_DECAY_MAX,
        );
        if c {
            log::warn!(
                "Invalid ambient.noise_decay {}, clamping to {} (must be < 1.0 to prevent infinite rattle)",
                self.ambient.noise_decay,
                v
            );
        }
        self.ambient.noise_decay = v;

        // Housing parameters (v4.2.0+)
        let (v, c) = clamp_finite(
            self.housing.frequency,
            ranges::HOUSING_FREQ_MIN,
            ranges::HOUSING_FREQ_MAX,
        );
        if c {
            log::warn!(
                "Invalid housing.frequency {}, clamping to {}",
                self.housing.frequency,
                v
            );
        }
        self.housing.frequency = v;

        let (v, c) = clamp_finite(
            self.housing.resonance,
            ranges::RESONANCE_MIN,
            ranges::RESONANCE_MAX,
        );
        if c {
            log::warn!(
                "Invalid housing.resonance {}, clamping to {}",
                self.housing.resonance,
                v
            );
        }
        self.housing.resonance = v;

        let (v, c) = clamp_finite(self.housing.mix, ranges::MIX_MIN, ranges::MIX_MAX);
        if c {
            log::warn!(
                "Invalid housing.mix {}, clamping to {}",
                self.housing.mix,
                v
            );
        }
        self.housing.mix = v;
    }
}
