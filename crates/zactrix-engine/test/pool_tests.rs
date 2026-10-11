// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// pool_tests.rs — the pool.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;
use zactrix_profiles::{MechanicalClick, SAMPLE_RATE};

#[test]
fn test_pool_instantiation() {
    let pool = VoicePool::new();
    assert_eq!(pool.polyphony(), MAX_POLYPHONY);
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn test_single_trigger_activates_one_voice() {
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );
    assert_eq!(pool.active_count(), 1);
}

#[test]
fn test_release_deactivates_voice() {
    // v10.2.0 (dragonzen audit N2): release no longer hard-cuts
    // active=false. Instead it sets `releasing=true` and the voice
    // ramps its envelope down to zero over ~2 ms via the
    // `release_coeff` coefficient before naturally deactivating
    // through the `voice_off_threshold` check. The test must now
    // render samples until the voice actually goes silent.
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );
    assert_eq!(pool.active_count(), 1);

    pool.release(30);
    // Voice is still active immediately after release — it's now in
    // the fast release-ramp phase.
    assert_eq!(
        pool.active_count(),
        1,
        "voice should still be active during release ramp"
    );

    // Render until the release ramp completes. 10 ms of audio at
    // 44.1 kHz = 441 samples, which is ~5x the 2 ms release window —
    // generous headroom.
    let mut samples_rendered = 0;
    for _ in 0..500 {
        let _ = pool.process_sample(&model);
        samples_rendered += 1;
        if pool.active_count() == 0 {
            break;
        }
    }
    assert_eq!(
        pool.active_count(),
        0,
        "voice should deactivate after release ramp (rendered {samples_rendered} samples)"
    );
}

#[test]
fn test_release_nonexistent_scancode_is_noop() {
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );
    pool.release(999); // non-existent
    assert_eq!(
        pool.active_count(),
        1,
        "Releasing a non-existent scancode should be a no-op"
    );
}

#[test]
fn test_release_ramp_is_smooth_no_click_artifact() {
    // v10.2.0 (dragonzen audit N2): the release ramp must NOT
    // produce a discontinuity at the moment of release. The
    // pre-release sample and the first post-release sample should
    // differ by a small amount (envelope multiplied by
    // release_coeff instead of decay_coeff), not by a large jump
    // to zero. We assert the max sample-to-sample delta around the
    // release boundary stays well below the "click" threshold.
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );

    // Render enough samples for the click transient to peak and
    // start decaying (~5 ms = 220 samples at 44.1 kHz).
    for _ in 0..220 {
        let _ = pool.process_sample(&model);
    }

    // Capture the last pre-release sample.
    let pre = pool.process_sample(&model);
    let pre_amp = pre[0].abs().max(pre[1].abs());

    // Release and capture the first post-release sample.
    pool.release(30);
    let post = pool.process_sample(&model);
    let post_amp = post[0].abs().max(post[1].abs());

    // The post-release amplitude should be CLOSE to the pre-release
    // amplitude — the only difference is the decay coefficient
    // (slow) vs the release coefficient (fast). For typical
    // configs (decay=0.9994, release≈0.95 at 44.1 kHz), the ratio
    // post/pre is roughly 0.95/0.9994 ≈ 0.95 — a 5% drop, not a
    // hard cut to zero.
    assert!(
        post_amp > pre_amp * 0.5,
        "release ramp should be smooth: pre={pre_amp:.6}, post={post_amp:.6} (post should be > 50% of pre, not a hard cut to zero)"
    );
    assert!(
        post_amp < pre_amp * 1.05,
        "release should not INCREASE amplitude: pre={pre_amp:.6}, post={post_amp:.6}"
    );
}

#[test]
fn test_voice_stealing_replaces_oldest() {
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    // Fill all 16 voice slots
    for i in 0..MAX_POLYPHONY as u32 {
        pool.trigger(
            &model,
            &KeyTrigger {
                scancode: 10 + i,
                pressed: true,
                stereo_position: 0.0,
                velocity: None,
            },
        );
    }
    assert_eq!(pool.active_count(), MAX_POLYPHONY);

    // Trigger one more — should steal the oldest (scancode 10, timestamp 0)
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 999,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );
    assert_eq!(pool.active_count(), MAX_POLYPHONY);

    let has_999 = pool
        .voices
        .iter()
        .any(|v| v.scancode == 999 && v.is_active());
    assert!(has_999, "Stolen slot should now hold scancode 999");
}

#[test]
fn test_voice_stealing_priority() {
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    // Fill all voices
    for i in 0..MAX_POLYPHONY as u32 {
        pool.trigger(
            &model,
            &KeyTrigger {
                scancode: 10 + i,
                pressed: true,
                stereo_position: 0.0,
                velocity: None,
            },
        );
    }

    // Steal twice: first steals scancode 10, second steals scancode 11
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 901,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 902,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );

    let has_901 = pool.voices.iter().any(|v| v.scancode == 901);
    let has_902 = pool.voices.iter().any(|v| v.scancode == 902);
    let has_10 = pool
        .voices
        .iter()
        .any(|v| v.scancode == 10 && v.is_active());

    assert!(has_901 && has_902, "New voices should be present");
    assert!(!has_10, "Oldest voice should have been stolen twice ago");
}

#[test]
fn test_process_sample_produces_output() {
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );

    let mut peak: f32 = 0.0;
    let mut any_nonzero = false;

    for _ in 0..SAMPLE_RATE as usize {
        let [l, r] = pool.process_sample(&model);
        let amp = l.abs().max(r.abs());
        peak = peak.max(amp);
        if amp > 1e-8 {
            any_nonzero = true;
        }
    }

    assert!(any_nonzero, "Pool should produce non-zero audio");
    assert!(
        peak > 0.01,
        "Peak amplitude should be audible (peak={peak})"
    );
}

#[test]
fn test_process_batch_matches_per_sample() {
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);

    // Render via process_sample. Reset the micro-randomization
    // counter before triggering so both voices start from the same
    // seed — without this, the v5.0.0 micro-randomization gives
    // each voice a unique noise seed + pitch/amplitude drift, and
    // the two paths would (correctly) produce different output.
    // The test's intent is to verify that the batch and per-sample
    // render paths agree FOR THE SAME VOICE STATE, not that two
    // separately-triggered voices are identical.
    //
    // v5.0.1: the counter is now per-instance (not global), so
    // resetting it here only affects THIS `model` instance —
    // other tests running in parallel with their own
    // MechanicalClick instances are unaffected. This eliminates
    // the flaky-test race that v5.0.0's global counter caused.
    model.reset_keystroke_counter_for_tests();
    let mut pool_a = VoicePool::new();
    pool_a.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );
    let mut samples_a: Vec<[f32; 2]> = Vec::with_capacity(256);
    for _ in 0..256 {
        samples_a.push(pool_a.process_sample(&model));
    }

    // Render via process (batch). Reset the counter again so this
    // trigger gets the SAME seed as pool_a's trigger above.
    model.reset_keystroke_counter_for_tests();
    let mut pool_b = VoicePool::new();
    pool_b.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );
    let mut samples_b = vec![[0.0f32; 2]; 256];
    pool_b.process(&model, &mut samples_b);

    // Compare
    for (i, (a, b)) in samples_a.iter().zip(samples_b.iter()).enumerate() {
        assert!(
            (a[0] - b[0]).abs() < 1e-10 && (a[1] - b[1]).abs() < 1e-10,
            "Mismatch at sample {i}: per_sample={a:?}, batch={b:?}"
        );
    }
}

#[test]
fn test_multiple_voices_mix_correctly() {
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    // Trigger two keys at opposite stereo positions
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: -1.0,
            velocity: None,
        },
    );
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 48,
            pressed: true,
            stereo_position: 1.0,
            velocity: None,
        },
    );

    let [l, r] = pool.process_sample(&model);

    // Both channels should have significant output (each voice dominates one side)
    assert!(
        l.abs() > 0.001,
        "Left channel should have output from the left-panned voice"
    );
    assert!(
        r.abs() > 0.001,
        "Right channel should have output from the right-panned voice"
    );
}

#[test]
fn test_reset_clears_all_voices() {
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    for i in 0..5u32 {
        pool.trigger(
            &model,
            &KeyTrigger {
                scancode: i,
                pressed: true,
                stereo_position: 0.0,
                velocity: None,
            },
        );
    }
    assert!(pool.active_count() > 0);

    pool.reset();
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn test_render_one_second_and_write_wav() {
    use std::io::Write;

    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    // Trigger 3 keys at different stereo positions for a spatial spread
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: -0.7,
            velocity: None,
        },
    );
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 48,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 2,
            pressed: true,
            stereo_position: 0.5,
            velocity: None,
        },
    );

    let num_samples = SAMPLE_RATE as usize;
    let mut pcm: Vec<f32> = Vec::with_capacity(num_samples * 2);
    let mut peak: f32 = 0.0;

    for _ in 0..num_samples {
        let [l, r] = pool.process_sample(&model);
        peak = peak.max(l.abs()).max(r.abs());
        pcm.push(l);
        pcm.push(r);
    }

    assert!(peak > 0.01, "Should produce audible output (peak={peak})");

    // Write raw f32 file
    let raw_path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/test_output.raw");
    {
        let _ = std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/target"));
        let mut f = std::fs::File::create(raw_path).expect("Failed to create raw file");
        for &s in &pcm {
            f.write_all(&s.to_le_bytes()).unwrap();
        }
    }
    assert!(std::path::Path::new(raw_path).exists());

    // Write WAV file (32-bit IEEE float, stereo)
    let wav_path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/test_output.wav");
    write_wav_f32(wav_path, &pcm, SAMPLE_RATE as u32);
    assert!(std::path::Path::new(wav_path).exists());

    // Verify WAV header sanity
    let metadata = std::fs::metadata(wav_path).unwrap();
    let expected_size = 44 + (num_samples * 2 * 4) as u64; // 44 byte header + data
    assert_eq!(metadata.len(), expected_size, "WAV file size mismatch");
}

#[test]
fn test_all_voices_naturally_decay() {
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    // Trigger all 16 voices
    for i in 0..MAX_POLYPHONY as u32 {
        pool.trigger(
            &model,
            &KeyTrigger {
                scancode: 100 + i,
                pressed: true,
                stereo_position: 0.0,
                velocity: None,
            },
        );
    }

    // Render until all voices have decayed
    let mut max_iter = 500_000usize;
    while pool.active_count() > 0 && max_iter > 0 {
        let _ = pool.process_sample(&model);
        max_iter -= 1;
    }

    assert_eq!(
        pool.active_count(),
        0,
        "All voices should eventually decay to zero"
    );
    assert!(max_iter > 0, "Voices should decay within a reasonable time");
}

#[test]
fn test_nan_output_replaced_with_silence() {
    // The v1.0.0 crash-proofing guard: if the DSP somehow produces
    // NaN or Infinity, process_sample must return 0.0 (silence)
    // instead of passing the non-finite value to cpal/PipeWire
    // (which would crash the audio server).
    //
    // We can't easily make MechanicalClick produce NaN directly
    // (the TPT SVF is stable), but we CAN test the guard by
    // constructing a custom AcousticModel that returns NaN.
    use zactrix_profiles::{AcousticModel, KeyProfile, KeyTrigger, SynthState};

    struct NanModel;
    impl AcousticModel for NanModel {
        fn get_profile(&self, _event: &KeyTrigger) -> KeyProfile {
            KeyProfile::default()
        }
        fn init_state(
            &self,
            _profile: &KeyProfile,
            state: &mut SynthState,
            _stereo_position: f32,
            _velocity: Option<f32>,
        ) {
            state.active = true;
        }
        fn render_sample(&self, _state: &mut SynthState) -> [f32; 2] {
            [f32::NAN, f32::INFINITY]
        }
    }

    let model = NanModel;
    let mut pool = VoicePool::new();

    // Trigger a voice so process_sample has something to render.
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 42,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );

    // The model returns NaN/Infinity, but process_sample MUST
    // return 0.0 for both channels (the guard catches it).
    let [l, r] = pool.process_sample(&model);
    assert!(
        l.is_finite(),
        "Left channel must be finite (got {l}), NaN guard failed"
    );
    assert!(
        r.is_finite(),
        "Right channel must be finite (got {r}), NaN guard failed"
    );
    assert_eq!(l, 0.0, "NaN input should produce 0.0 silence");
    assert_eq!(r, 0.0, "Infinity input should produce 0.0 silence");
}

#[test]
fn test_normal_output_not_affected_by_nan_guard() {
    // Verify the NaN guard doesn't affect normal (finite) output.
    let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
    let mut pool = VoicePool::new();

    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );

    let [l, r] = pool.process_sample(&model);
    assert!(l.is_finite(), "Normal output must be finite");
    assert!(r.is_finite(), "Normal output must be finite");
    // The click transient should produce non-zero output.
    assert!(
        l.abs() > 0.0 || r.abs() > 0.0,
        "Normal voice should produce audio"
    );
}
