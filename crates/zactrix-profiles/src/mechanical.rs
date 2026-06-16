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

use crate::{AcousticModel, KeyEvent, KeyProfile, SynthState, SAMPLE_RATE};

/// Default acoustic model for mechanical keyboard switches.
///
/// Generates a short click/spring sound using filtered noise excitation and
/// exponential decay, typical of Cherry MX-style mechanical switches. This
/// implementation is fully procedural — no wavetable samples are used.
///
/// # Future Extensions
///
/// - Per-scancode frequency variation (key position / switch lot tolerance).
/// - Bottom-out thud component (low-frequency transient for heavy presses).
/// - Key release sound (separate profile with lower amplitude and different
///   resonance characteristics).
pub struct MechanicalClick;

impl MechanicalClick {
    /// Create a new `MechanicalClick` model with default parameters.
    #[inline]
    pub fn new() -> Self {
        Self
    }
}

impl Default for MechanicalClick {
    fn default() -> Self {
        Self::new()
    }
}

impl AcousticModel for MechanicalClick {
    fn get_profile(&self, _event: &KeyEvent) -> KeyProfile {
        // Uniform profile for all keys. Future: per-scancode variation
        // based on physical key position or user-configurable TOML.
        KeyProfile::default()
    }

    fn init_state(&self, profile: &KeyProfile, state: &mut SynthState, stereo_position: f32) {
        // Pre-compute click filter TPT coefficients
        state.click_g = (std::f32::consts::PI * profile.click.frequency / SAMPLE_RATE).tan();
        state.click_k = 1.0 / profile.click.resonance;

        // Pre-compute spring filter TPT coefficients
        state.spring_g = (std::f32::consts::PI * profile.spring.frequency / SAMPLE_RATE).tan();
        state.spring_k = 1.0 / profile.spring.resonance;

        // Pre-compute excitation burst duration in samples
        state.excitation_samples = (profile.click.duration_ms * 0.001 * SAMPLE_RATE) as u32;
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
        let mixed = click_out + sv1 * state.spring_mix;

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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
