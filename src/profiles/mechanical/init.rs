// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The ZactrixEngine init path (the AcousticModel init_state impl).

//! Split from render.rs under the 600-line no-mercy LOC law
//! (night-private-research-2xblade): a pure move, gates green in
//! between. The Zactrix physics shape constants live here - they are
//! the curve anchors init_state bakes into the voice state.

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
//
// night-frontier-3C: the shape constants are f64 — the curve anchors
// are computed in f64 and quantized once to f32 through [`anchor`]
// (see that fn for the law).
/// Impact collapse time constant: 3 tau is the ~3 ms broadband-to-band
/// collapse of the switch-leaf contact.
const TAU_CLICK_S: f64 = 0.001;
/// Spring wake time constant: 3 tau is the ~5 ms click-band-to-spring-
/// band travel.
const TAU_SPRING_S: f64 = 0.005 / 3.0;
/// Broadband start of the click cutoff, as a multiple of the click
/// band: first contact spreads energy across the audible range.
const CLICK_BREADTH: f64 = 2.5;

/// night-frontier-3C, the f64 curve-anchor law: every curve constant
/// init_state bakes (the envelope steps, the curve deltas, the TPT
/// coefficient trio, the pan pair, the high-pass coefficient, the
/// decay/release/ambient powers) is computed in f64 and quantized
/// ONCE, here, to f32 — IEEE round-to-nearest, the single rounding
/// the value ever takes. init_state runs once per keypress, OFF the
/// audio callback, so the wider precision costs nothing at runtime;
/// what it buys is the end of init-time rounding wobble: the old f32
/// chains rounded every intermediate (the tau division inside the
/// exp argument, the tan's frequency ratio, the powf's rate ratio)
/// and let each of those roundings drift the anchor a few ULPs from
/// the value the same mathematics computes exactly. The anchor is now
/// the correctly-rounded f32 of the f64 computation, and the
/// dominance suite (test/profiles/mechanical_tests.rs,
/// `night-frontier-3C` tests) proves per anchor that the new value
/// sits at or closer to the f64 truth than the old f32 chain over the
/// whole shipped-preset domain. Cross-libm byte-identity remains a
/// separate frontier (the f64 libm answers still vary by platform);
/// this lane removes the self-inflicted half of the wobble.
fn anchor(x: f64) -> f32 {
    x as f32
}

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
    // night-frontier-3C: f64 — the resonance curve's k anchors divide
    // by this scale, and the scale now rides the f64 anchor path.
    let velocity_resonance_scale = 1.0 + (1.0 - f64::from(vel)) * 0.2;

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
    //
    // night-frontier-3C: every constant below is computed in f64 and
    // quantized once through `anchor` — the drifted f32 frequency
    // converts exactly, so the only rounding the curve takes is its
    // final one.
    state.click_fc_end = click_freq;
    let f_broad = (f64::from(click_freq) * CLICK_BREADTH).min(0.45 * f64::from(sr));
    state.click_fc_delta = anchor(f_broad - f64::from(click_freq));
    // The k curve's start (e=1) is 4x its end — one f64 source feeds
    // BOTH anchors, so the quarter-resonance law holds before and
    // after the single quantization each value takes.
    let click_k = 1.0 / (f64::from(profile.click.resonance) * velocity_resonance_scale);
    state.click_k = anchor(click_k);
    state.click_k_delta = anchor(3.0 * click_k);
    state.click_step = anchor(f64::exp(-1.0 / (f64::from(sr) * TAU_CLICK_S)));
    state.click_e = 1.0;

    // ── Zactrix stage B (spring): the wake curve ───────────────
    // Physics shape, locked: the cutoff travels from the click
    // band to the spring band with a ~1.67 ms time constant (3 tau
    // = the ~5 ms wake) while Q wakes from a quarter of the preset
    // resonance to it — the spring is born broad and sharpens as
    // it starts to ring. Down in Hz for every shipped preset
    // (spring below click); the shape is the travel, the direction
    // is the preset's numbers.
    // night-frontier-3C: the wake curve's constants ride the same f64
    // anchor law — the travel delta, the k pair from one f64 source,
    // and the wake step, each quantized once.
    state.spring_fc_end = spring_freq;
    state.spring_fc_delta = anchor(f64::from(click_freq) - f64::from(spring_freq));
    let spring_k = 1.0 / f64::from(profile.spring.resonance);
    state.spring_k = anchor(spring_k);
    state.spring_k_delta = anchor(3.0 * spring_k);
    state.spring_step = anchor(f64::exp(-1.0 / (f64::from(sr) * TAU_SPRING_S)));
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
    // night-frontier-3C: g and k are computed in f64 (the tan's
    // frequency ratio no longer rounds in f32 before the
    // transcendental) and every derived constant below quantizes once.
    let housing_g = (std::f64::consts::PI * f64::from(housing_freq) / f64::from(sr)).tan();
    let housing_k = 1.0 / f64::from(profile.housing.resonance);
    state.housing_g = anchor(housing_g);
    state.housing_k = anchor(housing_k);
    state.housing_mix = profile.housing.mix;

    // night-depth-hardening-1: the housing curve is the constant
    // itself (Zactrix stage C), so its full TPT coefficient trio and
    // the bandpass level normalizer are derived HERE, once, instead of
    // being recomputed from the same g and k on every sample. Same
    // expressions, same order as the per-sample block they replaced.
    // night-frontier-3C: the derivation now runs in f64 from the same
    // g/k source above, each value quantized once through `anchor` —
    // the coefficients the render path reads are the correctly-rounded
    // f32 of the exact TPT algebra, not the third rounding of a f32
    // chain.
    // a2/a3 derive from the f64 a1 — the SAME source the stored a1
    // quantizes from — so the trio shares one rounding-free lineage
    // instead of compounding the quantized a1 back into the chain.
    let housing_a1 = 1.0 / (1.0 + housing_g * (housing_g + housing_k));
    state.housing_a1 = anchor(housing_a1);
    state.housing_a2 = anchor(housing_g * housing_a1);
    state.housing_a3 = anchor(housing_g * housing_g * housing_a1);
    state.housing_gain = if housing_k > 0.0 {
        anchor(1.0 / housing_k)
    } else {
        1.0
    };

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
    // night-frontier-3C: the c^(R/S) ratio and the pow run in f64,
    // quantized once — at the 44.1 kHz reference the ratio is still
    // exactly 1.0 and the factor exactly 1.0.
    let decay_rate_norm = f64::from(crate::profiles::SAMPLE_RATE) / f64::from(sr.max(1.0));
    state.decay_coeff = anchor(f64::from(profile.decay.coefficient).powf(decay_rate_norm));
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
    // so the disabled lane is untouched). night-frontier-3C: f64
    // anchor, one quantization.
    state.decay_coeff_fast =
        anchor(f64::from(profile.decay.coefficient_fast).powf(decay_rate_norm));
    state.fast_samples_count =
        if profile.decay.coefficient_fast > 0.0 && profile.decay.fast_samples_ms > 0.0 {
            (profile.decay.fast_samples_ms * 0.001 * sr) as u32
        } else {
            0
        };

    // Pre-compute spring mix level
    state.spring_mix = profile.spring.mix;

    // Equal-power pan law: theta maps stereo_position from [-1,1] to [0, pi/2]
    // night-frontier-3C: the pan pair is a curve constant too — the
    // cos/sin run in f64 from the exactly-converted theta, quantized
    // once each.
    let theta =
        (f64::from(stereo_position.clamp(-1.0, 1.0)) + 1.0) * 0.5 * std::f64::consts::FRAC_PI_2;
    state.pan_left = anchor(theta.cos());
    state.pan_right = anchor(theta.sin());

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

    // Reset the frozen-coefficient lanes (night-depth-hardening-1).
    // The voice pool reuses this state across keypresses, so a stale
    // frozen flag from the previous voice would skip the new curve's
    // ramp entirely. The frozen coefficient slots need no reset: they
    // are only read while the flag is set, and the flag is re-armed
    // here.
    state.click_coeff_frozen = false;
    state.spring_coeff_frozen = false;

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
    // night-frontier-3C: f64 anchor, one quantization.
    state.ambient_decay = anchor(f64::from(profile.ambient.noise_decay).powf(decay_rate_norm));
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
    // night-frontier-3C: the omega/cos/quotient chain runs in f64 and
    // quantizes once — the f32 cos of a f32-rounded omega was the
    // wobble source here.
    let omega = 2.0 * std::f64::consts::PI * f64::from(2000.0_f32) / f64::from(sr);
    let cos_omega = omega.cos();
    state.ambient_hp_coeff = anchor((1.0 - cos_omega) / (1.0 + cos_omega));

    // Pre-compute the release-ramp coefficient (v10.2.0+ — dragonzen
    // audit N2). Target: bring the envelope from its current value to
    // ~1% over 2 ms, regardless of sample rate. Solving
    // `coeff^N = 0.01` for `N = 2 ms * sample_rate` gives
    // `coeff = 0.01^(1/N)`. At 44.1 kHz: N ≈ 88, coeff ≈ 0.9499.
    // At 48 kHz: N ≈ 96, coeff ≈ 0.9535. At 96 kHz: N ≈ 192,
    // coeff ≈ 0.9765. Clamped to (0, 0.9999) for safety.
    // night-frontier-3C: the 1/N ratio and the pow run in f64, the
    // clamp happens before the single quantization.
    let release_samples = (0.002 * f64::from(sr)).max(1.0);
    let release_coeff = 0.04_f64.powf(1.0 / release_samples).clamp(0.0, 0.9999);
    state.release_coeff = anchor(release_coeff);

    // Activate voice
    state.active = true;
    state.releasing = false;
}
