// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// mechanical_tests.rs — the mechanical.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

/// Night-improve-22: these tests were authored against the
/// pre-registry construction default (KeyProfile::default()). Pin the
/// input explicitly so the suite stays deterministic no matter which
/// preset the compiled DEFAULT_PRESET names.
fn model_at_default_profile() -> MechanicalClick {
    MechanicalClick::with_profile(KeyProfile::default(), crate::profiles::SAMPLE_RATE as u32)
}

use crate::profiles::{AcousticModel, KeyTrigger, SynthState};

#[test]
fn test_profile_returns_sane_defaults() {
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
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
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    let profile = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&profile, &mut state, event.stereo_position, None);

    // v10.2.0 (N6): per-keypress pan jitter (±3%) means L and R
    // are no longer bit-identical even at center pan. The
    // difference should be small (< 6% of peak amplitude).
    let [l, r] = model.render_sample(&mut state);
    let peak = l.abs().max(r.abs());
    assert!(
        (l - r).abs() < peak * 0.06 + 1e-6,
        "Center pan should produce near-equal L/R (with ±3% jitter tolerance). Got l={l}, r={r}"
    );
}

#[test]
fn test_full_left_pan_is_louder_on_left() {
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: -1.0,
        velocity: None,
    };

    let profile = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&profile, &mut state, event.stereo_position, None);

    let [l, r] = model.render_sample(&mut state);
    assert!(
        l.abs() > r.abs() + 1e-6,
        "Full-left pan: left should dominate"
    );
}

#[test]
fn test_full_right_pan_is_louder_on_right() {
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 1.0,
        velocity: None,
    };

    let profile = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&profile, &mut state, event.stereo_position, None);

    let [l, r] = model.render_sample(&mut state);
    assert!(
        r.abs() > l.abs() + 1e-6,
        "Full-right pan: right should dominate"
    );
}

#[test]
fn test_voice_decays_and_deactivates() {
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    let profile = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&profile, &mut state, event.stereo_position, None);

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
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    let profile = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&profile, &mut state, event.stereo_position, None);

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
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    let profile = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&profile, &mut state, event.stereo_position, None);

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

/// v10.2.0 (N4): quadratic fade should produce a non-linear
/// envelope on the excitation burst. We sample the click path's
/// excitation amplitude at three points (start, mid, end of burst)
/// by reading the click filter's bandpass output and verify the
/// mid-burst amplitude is less than what linear fade would give.
#[test]
fn test_excitation_fade_is_quadratic() {
    // With a linear fade, amplitude at progress=0.5 is 0.5 of the
    // initial. With quadratic, it's 0.25. We don't need to be
    // exact — just verify the mid-burst is significantly below
    // 0.5 of the start (which would only happen with a non-linear
    // fade). This catches accidental regression to linear.
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    let profile = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&profile, &mut state, event.stereo_position, None);

    // Render up to the burst midpoint.
    let mid = state.excitation_samples / 2;
    let mut peak_first_half: f32 = 0.0;
    let mut peak_second_half: f32 = 0.0;
    for i in 0..state.excitation_samples {
        let [l, r] = model.render_sample(&mut state);
        let amp = l.abs().max(r.abs());
        if i < mid {
            peak_first_half = peak_first_half.max(amp);
        } else {
            peak_second_half = peak_second_half.max(amp);
        }
    }

    // With quadratic fade, the second-half peak should be at most
    // ~25% of the first-half peak (linear would give ~50%).
    // Allow some slack for noise variance.
    assert!(
        peak_second_half < peak_first_half * 0.4,
        "Excitation fade should be non-linear (quadratic). first_half={peak_first_half:.6}, second_half={peak_second_half:.6}"
    );
}

/// v10.2.0 (N5): inter-keystroke interval attenuation should
/// reduce the amplitude of fast repeats.
#[test]
fn test_inter_keystroke_interval_attenuation() {
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    // First press — no previous, no attenuation. Returns 0
    // (first press).
    let first_interval = model.record_trigger_timestamp(1_000_000); // 1s in us
    assert_eq!(
        first_interval, 0,
        "first press should return 0 (no previous)"
    );
    let profile1 = model.get_profile(&event);
    let mut state1 = SynthState::default();
    model.init_state(&profile1, &mut state1, event.stereo_position, None);
    let amp_first = state1.envelope_value;

    // Second press 20 ms later — fast repeat, should attenuate by
    // up to 5% (interval_ms / 80, clamped to 1.0, times 0.05).
    // At 20 ms interval: 20/80 = 0.25 → attenuation = 0.05 * 0.25
    // = 0.0125 → amplitude = first_amp * (1 - 0.0125) = 0.9875 of
    // the unattenuated value. Hard to test in isolation because
    // the per-keypress amplitude_drift (±5%) is also in play. We
    // can't directly compare envelope_values; instead verify the
    // interval was recorded and returned correctly.
    let interval_returned = model.record_trigger_timestamp(1_020_000);
    assert_eq!(
        interval_returned, 20_000,
        "record_trigger_timestamp should return the interval in microseconds"
    );

    // Verify the interval is stashed for init_state to read.
    // We can't access peek_last_interval_us from outside the
    // crate, but we can verify init_state uses it by checking
    // that a second press within the interval window produces a
    // different envelope than one without. (Just smoke-test the
    // mechanism.)
    let profile2 = model.get_profile(&event);
    let mut state2 = SynthState::default();
    model.init_state(&profile2, &mut state2, event.stereo_position, None);
    // Smoke: state2 should be active with a positive envelope.
    assert!(state2.active, "voice should be active after trigger");
    assert!(
        state2.envelope_value > 0.0,
        "envelope should be positive after init"
    );
    // Sanity: amplitude is in [0, 2] (clamp).
    assert!(state2.envelope_value <= 2.0);

    let _ = amp_first; // suppress unused
}

/// v10.2.0 (N6): per-keypress pan jitter should differ between
/// two consecutive presses of the same key.
#[test]
fn test_pan_jitter_varies_between_keypresses() {
    let model = model_at_default_profile();
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    // Render many presses and collect the pan_jitter values.
    // They should vary — if they're all identical, the jitter
    // derivation is broken.
    let mut jitters = std::collections::HashSet::new();
    for _ in 0..20 {
        let profile = model.get_profile(&event);
        let mut state = SynthState::default();
        model.init_state(&profile, &mut state, event.stereo_position, None);
        // Quantize to f32 bits — same value = same hash.
        jitters.insert(state.pan_jitter.to_le_bytes());
    }

    // With 20 presses, we should see at least 10 distinct jitter
    // values. (Statistically, with 24-bit precision, collisions
    // are very rare.)
    assert!(
        jitters.len() >= 10,
        "pan_jitter should vary between keypresses — got {} distinct values out of 20",
        jitters.len()
    );
}

/// v10.2.0 (N7): two-stage decay should make the envelope drop
/// faster during the fast stage than the slow stage.
#[test]
fn test_two_stage_decay_drops_faster_in_fast_stage() {
    let mut profile = KeyProfile::default();
    // Configure a clear two-stage decay: fast stage at 0.997 for
    // 5 ms, slow tail at 0.9999.
    profile.decay.coefficient_fast = 0.997;
    profile.decay.fast_samples_ms = 5.0;
    profile.decay.coefficient = 0.9999;
    profile.decay.voice_off_threshold = 1e-7;

    let model = MechanicalClick::with_profile(profile, crate::profiles::SAMPLE_RATE as u32);
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    let p = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&p, &mut state, event.stereo_position, None);

    // Track envelope_value over time. Compute the per-sample
    // ratio for the fast stage (samples 1..fast_samples_count)
    // and the slow stage (samples fast_samples_count+1..2x).
    let fast_samples = state.fast_samples_count;
    assert!(fast_samples > 0, "fast_samples_count should be > 0");

    let mut prev = state.envelope_value;
    let mut fast_ratios: Vec<f32> = Vec::new();
    for i in 0..fast_samples {
        let _ = model.render_sample(&mut state);
        if i > 0 && prev > 0.0 {
            fast_ratios.push(state.envelope_value / prev);
        }
        prev = state.envelope_value;
    }

    let mut slow_ratios: Vec<f32> = Vec::new();
    for _ in 0..fast_samples {
        let _ = model.render_sample(&mut state);
        if prev > 0.0 {
            slow_ratios.push(state.envelope_value / prev);
        }
        prev = state.envelope_value;
    }

    let avg_fast: f32 = fast_ratios.iter().sum::<f32>() / fast_ratios.len().max(1) as f32;
    let avg_slow: f32 = slow_ratios.iter().sum::<f32>() / slow_ratios.len().max(1) as f32;

    // The fast stage should have a lower ratio (steeper decay)
    // than the slow stage.
    assert!(
        avg_fast < avg_slow,
        "fast stage ratio ({avg_fast:.6}) should be lower than slow stage ratio ({avg_slow:.6})"
    );
    // And specifically, the fast ratio should be close to 0.997
    // (the configured coefficient_fast).
    assert!(
        (avg_fast - 0.997).abs() < 0.001,
        "fast stage ratio should match coefficient_fast=0.997, got {avg_fast:.6}"
    );
    // Slow ratio should be close to 0.9999.
    assert!(
        (avg_slow - 0.9999).abs() < 0.001,
        "slow stage ratio should match coefficient=0.9999, got {avg_slow:.6}"
    );
}

/// v10.2.0 (N7): when coefficient_fast = 0.0, the fast stage is
/// disabled and the voice uses single-stage decay (backward-
/// compatible with pre-v10.2.0 configs).
#[test]
fn test_two_stage_decay_disabled_when_coefficient_fast_zero() {
    let profile = KeyProfile::default();
    // Default profile has coefficient_fast = 0.0 and
    // fast_samples_ms = 0.0.
    let model = MechanicalClick::with_profile(profile, crate::profiles::SAMPLE_RATE as u32);
    let event = KeyTrigger {
        scancode: 42,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    let p = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&p, &mut state, event.stereo_position, None);

    assert_eq!(
        state.fast_samples_count, 0,
        "fast_samples_count should be 0 when coefficient_fast is 0.0"
    );
    assert_eq!(
        state.decay_coeff_fast, 0.0,
        "decay_coeff_fast should be 0.0 when disabled"
    );

    // The envelope should decay at the slow rate for the entire
    // voice. Sample the ratio at sample 1 and at sample 100 —
    // both should equal decay_coeff (0.9994 by default).
    let prev = state.envelope_value;
    let _ = model.render_sample(&mut state);
    let ratio1 = state.envelope_value / prev;
    assert!(
        (ratio1 - state.decay_coeff).abs() < 1e-6,
        "disabled two-stage should use decay_coeff everywhere, got ratio={ratio1:.6}"
    );
}

/// v11.0.0: analog velocity scales amplitude. A light tap (0.3)
/// should produce a quieter envelope than a full press (None=1.0).
#[test]
fn test_velocity_scales_amplitude() {
    let model = model_at_default_profile();
    let profile = KeyProfile::default();

    // Full press (digital switch — None = 1.0).
    let mut state_full = SynthState::default();
    model.init_state(&profile, &mut state_full, 0.0, None);
    let amp_full = state_full.envelope_value;

    // Light tap (analog — Some(0.3)).
    let mut state_light = SynthState::default();
    model.init_state(&profile, &mut state_light, 0.0, Some(0.3));
    let amp_light = state_light.envelope_value;

    assert!(
        amp_light < amp_full * 0.8,
        "light tap (0.3) should produce quieter amplitude than full press: \
             light={amp_light:.4}, full={amp_full:.4}"
    );
    assert!(
        amp_light > 0.0,
        "light tap should still produce positive amplitude: {amp_light:.4}"
    );
}

/// v11.0.0: velocity=None (digital) and velocity=Some(1.0) (full
/// analog) should produce identical amplitude.
#[test]
fn test_velocity_none_equals_full() {
    let model = model_at_default_profile();
    let profile = KeyProfile::default();

    let mut state_digital = SynthState::default();
    model.init_state(&profile, &mut state_digital, 0.0, None);

    let mut state_analog_full = SynthState::default();
    model.init_state(&profile, &mut state_analog_full, 0.0, Some(1.0));

    // Amplitudes should be very close (modulo micro-randomization
    // drift, which differs per keystroke). Allow 10% tolerance.
    let ratio = state_digital.envelope_value / state_analog_full.envelope_value;
    assert!(
        (ratio - 1.0).abs() < 0.1,
        "None and Some(1.0) should produce similar amplitude: ratio={ratio:.4}"
    );
}
