// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// render_tests.rs — the voice-pool RENDER OUTPUT pins (night-improve-24,
// split from pool_tests.rs under the 600-line no-mercy cap: the pool
// lifecycle pins stay there, the render-output contract lives here -
// one theme per file, the zelynic Pattern C test tree, wired via
// #[path]). The module position is unchanged, so super::* still
// resolves (write_wav_f32 and the pool API ride it).

use super::*;

use crate::profiles::{KeyProfile, MechanicalClick, SAMPLE_RATE};

/// Same pin as pool_tests.rs: build the model against
/// KeyProfile::default() explicitly so the suite stays deterministic
/// no matter which preset the compiled DEFAULT_PRESET names.
fn model_at_default_profile() -> MechanicalClick {
    MechanicalClick::with_profile(KeyProfile::default(), crate::profiles::SAMPLE_RATE as u32)
}

#[test]
fn test_process_sample_produces_output() {
    let model = model_at_default_profile();
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
    let model = model_at_default_profile();

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

    // Compare. The tolerance is 1e-4 (-80 dBFS, far below the audible
    // floor at the levels this engine renders), not bitwise: the two
    // arms share process_sample by construction, but each arm triggers
    // its own voice, and the per-trigger filter coefficients ride
    // f32::tan — a transcendental the miri interpreter resolves
    // through host libm, which is NOT bit-deterministic across calls
    // (verified: tan on identical bits returned two different results
    // within one interpreted run). A few ULP of coefficient noise
    // therefore seeds the resonant TPT feedback loops differently, and
    // the divergence grows through the recursion (observed: ~4 ULP at
    // sample 0, 1.6e-6 by sample 19), while native codegen is
    // deterministic and passes at 1e-10 too. Any real algorithmic
    // divergence between the two paths — a shifted burst, a changed
    // envelope, a different voice count — is orders of magnitude
    // larger than 1e-4 and still fails this assert.
    for (i, (a, b)) in samples_a.iter().zip(samples_b.iter()).enumerate() {
        assert!(
            (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4,
            "Mismatch at sample {i}: per_sample={a:?}, batch={b:?}"
        );
    }
}

#[test]
fn test_multiple_voices_mix_correctly() {
    let model = model_at_default_profile();
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
fn test_render_one_second_and_write_wav() {
    let model = model_at_default_profile();
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

    // The artifact half is real file IO: the miri interpreter refuses
    // host filesystem access by design (isolation), so the write and
    // verify half is compiled out there — the interpreted lane still
    // owns the full one-second DSP render above, and the native lanes
    // keep the artifact contract.
    #[cfg(not(miri))]
    {
        use std::io::Write;

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
    use crate::profiles::{AcousticModel, KeyProfile, KeyTrigger, SynthState};

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
    let model = model_at_default_profile();
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
