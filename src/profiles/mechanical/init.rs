// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The ZactrixEngine init path (the AcousticModel init_state impl).

//! Split from render.rs under the 600-line no-mercy LOC law
//! (night-private-research-2xblade): a pure move, gates green in
//! between. The Zactrix physics shape constants live here - they are
//! the curve anchors init_state bakes into the voice state.

use std::f32::consts::FRAC_PI_2;
use std::sync::atomic::Ordering;

use crate::profiles::{KeyProfile, SynthState};

use super::{
    AMPLITUDE_DRIFT_FRAC, PITCH_DRIFT_FRAC, ZactrixEngine, splitmix64, u32_to_signed_f32,
    xorshift32,
};

// ── Zactrix physics shape constants (night-private-research-2xblade) ──
// The SHAPE is the actuation physics and is the same for every preset;
// each preset's own numbers (frequencies, resonances) are the curve
// anchors. Verifier-first: docs/ZACTRIX_AUDIT.md holds the measurement
// these curves answer, and scripts/audit/zactrix-verify.sh judges them.
/// Impact collapse time constant: 3 tau is the ~3 ms broadband-to-band
/// collapse of the switch-leaf contact.
const TAU_CLICK_S: f32 = 0.001;
/// Spring wake time constant: 3 tau is the ~5 ms click-band-to-spring-
/// band travel.
const TAU_SPRING_S: f32 = 0.005 / 3.0;
/// Broadband start of the click cutoff, as a multiple of the click
/// band: first contact spreads energy across the audible range.
const CLICK_BREADTH: f32 = 2.5;

/// The trait init_state's body, as a free function: the trait impl
/// lives once, in render.rs (Rust allows one impl block per trait and
/// type), and the child module reaches the model's private fields
/// directly - a pure move under the 600-line LOC law.
pub(super) fn init_state(
    model: &ZactrixEngine,
    profile: &KeyProfile,
    state: &mut SynthState,
    stereo_position: f32,
    velocity: Option<f32>,
) {
    let sr = model.sample_rate;

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
    let keystroke_id = model.keystroke_counter.fetch_add(1, Ordering::Relaxed);

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
    let interval_us = model.peek_last_interval_us();
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

    // ── Zactrix stage A (click): the impact-collapse curve ─────
    // Physics shape, locked: first contact is broadband — the
    // cutoff starts at 2.5x the click band (clamped below 0.45
    // Nyquist so the bilinear prewarp stays honest) and collapses
    // into the click band with a ~1 ms time constant (3 tau = the
    // ~3 ms collapse), while Q starts at a quarter of the preset
    // resonance (a broadband passage, not a narrow ping) and
    // settles to it. The drifted click frequency rides as the
    // anchor, so micro-randomization moves the whole curve exactly
    // as it moved the frozen coefficient before.
    state.click_fc_end = click_freq;
    let f_broad = (click_freq * CLICK_BREADTH).min(0.45 * sr);
    state.click_fc_delta = f_broad - click_freq;
    state.click_k = 1.0 / (profile.click.resonance * velocity_resonance_scale);
    state.click_k_delta = 3.0 * state.click_k;
    state.click_step = (-1.0 / (sr * TAU_CLICK_S)).exp();
    state.click_e = 1.0;

    // ── Zactrix stage B (spring): the wake curve ───────────────
    // Physics shape, locked: the cutoff travels from the click
    // band to the spring band with a ~1.67 ms time constant (3 tau
    // = the ~5 ms wake) while Q wakes from a quarter of the preset
    // resonance to it — the spring is born broad and sharpens as
    // it starts to ring. Down in Hz for every shipped preset
    // (spring below click); the shape is the travel, the direction
    // is the preset's numbers.
    state.spring_fc_end = spring_freq;
    state.spring_fc_delta = click_freq - spring_freq;
    state.spring_k = 1.0 / profile.spring.resonance;
    state.spring_k_delta = 3.0 * state.spring_k;
    state.spring_step = (-1.0 / (sr * TAU_SPRING_S)).exp();
    state.spring_e = 1.0;

    // Pre-compute housing filter TPT coefficients (v4.2.0+, using
    // drifted frequency). The housing layer is a third TPT SVF
    // bandpass tuned to low frequencies (100-1000 Hz) to model the
    // keycap-hitting-PCB "thock" sound. It is driven by the same
    // noise excitation as the click and spring layers, but with a
    // longer excitation window — see `housing_excitation_samples`
    // below. Zactrix stage C: the curve is the constant itself —
    // cutoff stays low, Q stays high, and the long ring stays the
    // existing envelope's job.
    state.housing_g = (std::f32::consts::PI * housing_freq / sr).tan();
    state.housing_k = 1.0 / profile.housing.resonance;
    state.housing_mix = profile.housing.mix;

    // Pre-compute excitation burst duration in samples
    state.excitation_samples = (profile.click.duration_ms * 0.001 * sr) as u32;
    state.excitation_samples = state.excitation_samples.max(1);

    // Housing excitation window. Zactrix stage C drive law
    // (night-private-research-2xblade): the drive window follows
    // the mode. A HIGH-Q housing (Q > 3, the re-tuned body) is
    // kicked by the impact's own contact time and rings free — a
    // longer driven window would let the narrow low band slurp
    // the attack (measured in the first fit pass: the centroid
    // contrast and the modal structure collapsed on exactly the
    // presets whose housing dominated). A LOW-Q housing (Q <= 3,
    // the legacy shape every preset shipped before the re-tune,
    // including topre and the four identity refusals) still needs
    // the four-period driven window to build audible level — the
    // ring-up time of a wide mode is long, and the thock lives in
    // the driven steady state.
    let housing_periods = 4.0_f32;
    let housing_excitation_fundamental = (housing_periods * sr / housing_freq) as u32;
    state.housing_excitation_samples = if profile.housing.resonance > 3.0 {
        state.excitation_samples
    } else {
        state.excitation_samples.max(housing_excitation_fundamental)
    };
    state.housing_excitation_samples = state.housing_excitation_samples.max(1);

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
