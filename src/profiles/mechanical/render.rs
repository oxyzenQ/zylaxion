// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The ZactrixEngine render path (the AcousticModel impl).
//!
//! Split from mechanical.rs in night-improve-18 (the no-mercy LOC
//! law): the struct + defaults stay in the parent module, the
//! per-sample render loop lives here. The parent's private noise
//! helpers are visible to this child module.
use std::sync::atomic::Ordering;

use crate::profiles::{AcousticModel, KeyProfile, KeyTrigger, SynthState};

use super::{ZactrixEngine, init, u32_to_signed_f32};

impl AcousticModel for ZactrixEngine {
    fn get_profile(&self, event: &KeyTrigger) -> KeyProfile {
        // night-improve-22: one compiled preset profile for every key;
        // the per-scancode override map died with the config file.
        // KeyProfile is Copy, so this is a plain table read.
        let _ = event.scancode;
        self.profile
    }

    fn init_state(
        &self,
        profile: &KeyProfile,
        state: &mut SynthState,
        stereo_position: f32,
        velocity: Option<f32>,
    ) {
        // The body lives in the init child module (the 600-line LOC
        // law split); the child reaches this model's private fields
        // directly, so this is a pure delegation.
        init::init_state(self, profile, state, stereo_position, velocity)
    }
    fn render_sample(&self, state: &mut SynthState) -> [f32; 2] {
        if !state.active {
            return [0.0, 0.0];
        }
        let sr = self.sample_rate;

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
        // Zactrix stage A: the coefficients move with the impact-
        // collapse curve — broadband at first contact, collapsed into
        // the click band by ~3 ms. The TPT integrator state carries
        // across coefficient changes (that is what the topology
        // preserves), so per-sample modulation is stable by
        // construction; the clamps are a safety net, not a shaping
        // surface (every preset's curve lives far inside them).
        state.click_e *= state.click_step;
        let fc = (state.click_fc_end + state.click_fc_delta * state.click_e).clamp(20.0, 0.45 * sr);
        let k = (state.click_k + state.click_k_delta * state.click_e).clamp(0.1, 30.0);
        let g = (std::f32::consts::PI * fc / sr).tan();
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
        // Independent filter driven directly by the excitation. Zactrix
        // stage B: the cutoff travels from the click band into the
        // spring band over ~5 ms while Q wakes to the preset resonance
        // — the spring is born broad and sharpens as it starts to
        // ring. After the burst ends, the filter rings at its natural
        // frequency — this is the "spring" sound characteristic of
        // mechanical switches.
        state.spring_e *= state.spring_step;
        let sfc =
            (state.spring_fc_end + state.spring_fc_delta * state.spring_e).clamp(20.0, 0.45 * sr);
        let sk = (state.spring_k + state.spring_k_delta * state.spring_e).clamp(0.1, 30.0);
        let sg = (std::f32::consts::PI * sfc / sr).tan();
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
