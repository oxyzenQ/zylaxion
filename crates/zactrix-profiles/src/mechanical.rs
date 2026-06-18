// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

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

use std::f32::consts::FRAC_PI_2;

use crate::{AcousticModel, KeyEvent, KeyProfile, ProfileWithOverrides, SynthState};

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
        }
    }

    /// Create a `MechanicalClick` model from a loaded
    /// [`ProfileWithOverrides`] (default + per-key overrides).
    #[inline]
    pub fn with_overrides(profiles: ProfileWithOverrides, sample_rate: u32) -> Self {
        Self {
            profiles,
            sample_rate: sample_rate as f32,
        }
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

        // Pre-compute click filter TPT coefficients
        state.click_g = (std::f32::consts::PI * profile.click.frequency / sr).tan();
        state.click_k = 1.0 / profile.click.resonance;

        // Pre-compute spring filter TPT coefficients
        state.spring_g = (std::f32::consts::PI * profile.spring.frequency / sr).tan();
        state.spring_k = 1.0 / profile.spring.resonance;

        // Pre-compute excitation burst duration in samples
        state.excitation_samples = (profile.click.duration_ms * 0.001 * sr) as u32;
        state.excitation_samples = state.excitation_samples.max(1);

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

        // Reset envelope and sample counter
        state.envelope_value = profile.click.amplitude;
        state.sample_count = 0;

        // Initialize noise generator with a deterministic non-zero seed
        state.noise_state = 0xDEAD_BEEF;

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
        // noise streams don't correlate.
        state.ambient_noise_state = 0xCAFEBABE;
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
        // the "pring" sound characteristic of mechanical switches.
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

        // ── Stage 3: Mix components ──────────────────────────────────
        let mut mixed = click_out + sv1 * state.spring_mix;

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
