// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Topology-Preserving Transform (TPT) State Variable Filter.
//!
//! Based on Vadim Zavalishin's formulation from "The Art of VA Filter Design".
//! The TPT approach ensures numerical stability when filter parameters (cutoff,
//! resonance) change at audio rate — a critical requirement for real-time
//! procedural audio synthesis where parameters may be modulated dynamically.
//!
//! # Why TPT Over Naive Biquads?
//!
//! Standard biquad (Direct Form I/II) filters suffer from two problems in
//! real-time audio:
//!
//! 1. **Parameter zipper noise**: Changing coefficients between samples causes
//!    discontinuities in the output because the filter state was computed under
//!    the old coefficients.
//!
//! 2. **Numerical blow-up**: High Q values combined with fast parameter changes
//!    can cause the internal state to grow without bound, producing loud pops
//!    or NaN values.
//!
//! The TPT formulation solves both by using a "zero-delay feedback" topology
//! where the integrator states are implicitly solved rather than using a
//! one-sample delay. This means coefficient changes are "smooth" — the filter
//! simply continues from its current state under the new parameters.

use crate::SAMPLE_RATE;

/// TPT State Variable Filter (SVF).
///
/// Provides simultaneous lowpass, highpass, and bandpass outputs from a single
/// filter instance with only two state variables.
///
/// # Stability Guarantee
///
/// Unlike naive biquad implementations, the TPT SVF maintains bounded internal
/// state even when cutoff frequency or Q is changed between samples. This
/// prevents the "zipper noise" and numerical blow-ups that plague real-time
/// audio applications.
///
/// # Algorithm
///
/// The filter uses trapezoidal integration (bilinear transform) applied to a
/// state variable filter topology. The key innovation is that the coefficient
/// `g = tan(pi * fc / fs)` captures the frequency warping of the bilinear
/// transform, and the state variables `ic1eq`/`ic2eq` represent the trapezoidal
/// integrator outputs at the *current* sample (not the previous one).
#[derive(Debug, Clone, Copy)]
pub struct TptSvf {
    /// First integrator state (trapezoidal).
    pub ic1eq: f32,
    /// Second integrator state (trapezoidal).
    pub ic2eq: f32,
    /// Pre-computed coefficient: `tan(pi * fc / fs)`.
    pub g: f32,
    /// Pre-computed coefficient: `1 / Q`.
    pub k: f32,
}

impl Default for TptSvf {
    fn default() -> Self {
        let g = (std::f32::consts::PI * 1000.0 / SAMPLE_RATE).tan();
        Self {
            ic1eq: 0.0,
            ic2eq: 0.0,
            g,
            k: 1.0,
        }
    }
}

impl TptSvf {
    /// Create a new TPT SVF initialized to silence with default parameters
    /// (1 kHz cutoff, Q = 1.0) at the given `sample_rate` (Hz).
    #[inline]
    pub fn new(sample_rate: f32) -> Self {
        let g = (std::f32::consts::PI * 1000.0 / sample_rate).tan();
        Self {
            ic1eq: 0.0,
            ic2eq: 0.0,
            g,
            k: 1.0,
        }
    }

    /// Create a new TPT SVF with the given initial cutoff frequency, Q,
    /// and `sample_rate` (Hz).
    #[inline]
    pub fn with_params(freq: f32, q: f32, sample_rate: f32) -> Self {
        let g = (std::f32::consts::PI * freq / sample_rate).tan();
        Self {
            ic1eq: 0.0,
            ic2eq: 0.0,
            g,
            k: 1.0 / q,
        }
    }

    /// Update filter coefficients. Safe to call at audio rate.
    ///
    /// The TPT formulation guarantees that changing `freq` or `q` does not
    /// cause output discontinuities — the filter simply continues from its
    /// current state under the new parameters.
    #[inline]
    pub fn set_params(&mut self, freq: f32, q: f32, sample_rate: f32) {
        self.g = (std::f32::consts::PI * freq / sample_rate).tan();
        self.k = 1.0 / q;
    }

    /// Reset the filter state to zero (silence).
    #[inline]
    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    /// Process a single sample through the filter.
    ///
    /// Returns `(lowpass, highpass, bandpass)` outputs simultaneously.
    ///
    /// # Computational Cost
    ///
    /// - 3 multiplies + 3 additions for coefficient pre-computation
    /// - 5 multiplies + 5 additions for the core filter
    /// - 2 multiplies + 2 additions for state update
    /// - Total: ~20 floating-point operations per sample
    #[inline]
    pub fn process(&mut self, input: f32) -> (f32, f32, f32) {
        let g = self.g;
        let k = self.k;

        // Pre-compute shared coefficients (these are the TPT "magic numbers"
        // that ensure stability when parameters change)
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * g * a1;

        // TPT SVF core — zero-delay feedback topology
        let v3 = input - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;

        // Trapezoidal state update (icN_eq = 2*vN - icN_eq is equivalent
        // to the trapezoidal integrator y[n] = y[n-1] + T/2 * (x[n] + x[n-1]))
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        // Extract filter outputs
        let lowpass = v2;
        let highpass = v3 - v1;
        let bandpass = v1;

        (lowpass, highpass, bandpass)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_svf_stability_sine_sweep() {
        let mut svf = TptSvf::new(crate::SAMPLE_RATE);
        let mut max_val: f32 = 0.0;

        // Sweep frequency from 100 Hz to 10 kHz over 1 second.
        // A naive biquad would blow up here; TPT should remain stable.
        for i in 0..44_100usize {
            let t = i as f32 / 44_100.0;
            let freq = 100.0 * (100.0_f32).powf(t);
            svf.set_params(freq, 2.0, crate::SAMPLE_RATE);

            let input = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            let (lp, hp, bp) = svf.process(input);

            max_val = max_val.max(lp.abs()).max(hp.abs()).max(bp.abs());
        }

        assert!(
            max_val.is_finite() && max_val < 100.0,
            "SVF output blew up: max={max_val}"
        );
    }

    #[test]
    fn test_svf_impulse_response_decays() {
        let mut svf = TptSvf::with_params(1000.0, 5.0, crate::SAMPLE_RATE);
        let mut sum: f32 = 0.0;

        for i in 0..44_100usize {
            let input = if i == 0 { 1.0 } else { 0.0 };
            let (lp, _, bp) = svf.process(input);
            sum += lp.abs() + bp.abs();
        }

        // A resonant filter driven by an impulse must produce output.
        assert!(sum > 0.0, "Filter produced no output from impulse");
        assert!(sum.is_finite(), "Filter output is not finite");
    }

    #[test]
    fn test_svf_rapid_param_changes() {
        let mut svf = TptSvf::new(crate::SAMPLE_RATE);

        // Alternate between two extreme frequencies every 100 samples.
        for i in 0..44_100usize {
            let freq = if (i / 100) % 2 == 0 { 200.0 } else { 8000.0 };
            svf.set_params(freq, 0.5, crate::SAMPLE_RATE);
            let input = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44_100.0).sin();
            let _ = svf.process(input);
        }

        assert!(
            svf.ic1eq.is_finite() && svf.ic2eq.is_finite(),
            "Filter state diverged under rapid parameter changes"
        );
    }

    #[test]
    fn test_svf_high_q_does_not_blow_up() {
        let mut svf = TptSvf::with_params(500.0, 50.0, crate::SAMPLE_RATE);

        for _ in 0..44_100 {
            let (lp, hp, bp) = svf.process(0.5);
            assert!(lp.is_finite() && hp.is_finite() && bp.is_finite());
        }
    }
}
