// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The MechanicalClick render path (the AcousticModel impl).
//!
//! Split from mechanical.rs in night-improve-18 (the no-mercy LOC
//! law): the struct + defaults stay in the parent module, the
//! per-sample render loop lives here. The parent's private noise
//! helpers are visible to this child module.

use std::f32::consts::FRAC_PI_2;
use std::sync::atomic::Ordering;

use crate::profiles::{AcousticModel, KeyProfile, KeyTrigger, SynthState};

use super::{
    AMPLITUDE_DRIFT_FRAC, MechanicalClick, PITCH_DRIFT_FRAC, splitmix64, u32_to_signed_f32,
    xorshift32,
};

impl AcousticModel for MechanicalClick {
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

        // v11.0.0: Analog velocity support (Hall-effect switches).
        // velocity scales: amplitude (louder hit = louder sound),
        // resonance (harder press = slightly brighter click). None =
        // digital switch, treated as 1.0.
        let vel = velocity.unwrap_or(1.0).clamp(0.0, 1.0);
        let velocity_amplitude = (amplitude * vel).clamp(0.0, 2.0);
        // Harder press → slightly lower Q (brighter, more energy
        // passes through the bandpass). Light tap → higher Q (narrower,
        // more muted). Scale: ±20% of the profile resonance.
        let velocity_resonance_scale = 1.0 + (1.0 - vel) * 0.2;

        // Pre-compute click filter TPT coefficients (using drifted
        // frequency + velocity-modulated resonance)
        state.click_g = (std::f32::consts::PI * click_freq / sr).tan();
        state.click_k = 1.0 / (profile.click.resonance * velocity_resonance_scale);

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

        // Pre-compute decay and threshold from profile.
        //
        // night-master-4: sample-rate-normalized decay. The profile
        // coefficients are per-sample values documented at the crate's
        // canonical 44.1 kHz reference (params::SAMPLE_RATE — the same
        // reference every decay sentence in this tree is written at).
        // Applying them raw made the same preset ring a different
        // length per device rate: the retention-per-second is
        // coeff^sr, so a 96 kHz device decayed the envelope twice as
        // fast as a 44.1 kHz one. Measured before the fix:
        // ibm_model_m tail 1237 ms @ 44.1k vs 568 ms @ 96k;
        // glass_tactile 2113 ms vs 971 ms; technical 426 ms vs 196 ms.
        // The law c_S = c_R^(R/S) preserves the per-second retention
        // exactly: powf(R/S) re-derives the per-sample factor for the
        // device rate. At the 44.1 kHz reference the factor is
        // powf(1.0) = 1.0 — every preset is bit-identical to before.
        // The release ramp (N2) already followed this law; the main
        // and ambient envelopes now do too.
        let decay_rate_norm = crate::profiles::SAMPLE_RATE / sr.max(1.0);
        state.decay_coeff = profile.decay.coefficient.powf(decay_rate_norm);
        state.voice_off_threshold = profile.decay.voice_off_threshold;

        // v10.2.0 (dragonzen audit N7): two-stage decay. Pre-compute
        // the fast-stage coefficient + sample count from the profile.
        // If `coefficient_fast == 0.0` OR `fast_samples_ms == 0.0`,
        // the fast stage is disabled — render_sample falls back to the
        // single-stage `decay_coeff` for the entire voice. This keeps
        // the feature opt-in: existing configs that don't set the new
        // fields behave identically to pre-v10.2.0.
        // night-master-4: the fast-stage coefficient is normalized by
        // the same c^(R/S) law as the slow stage (0.0 powf stays 0.0,
        // so the disabled lane is untouched).
        state.decay_coeff_fast = profile.decay.coefficient_fast.powf(decay_rate_norm);
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
        // v11.0.0: use velocity-scaled amplitude for the envelope.
        state.envelope_value = velocity_amplitude;
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
        // night-master-4: the ambient rattle envelope follows the same
        // c^(R/S) sample-rate law as the main decay — the rattle is a
        // ~1 ms burst at the reference rate and must stay one at every
        // device rate, not four times longer at 96 kHz.
        state.ambient_decay = profile.ambient.noise_decay.powf(decay_rate_norm);
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
