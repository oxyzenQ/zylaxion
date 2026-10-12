// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// pool_tests.rs — the pool.rs test pins, relocated from the inline module (night-improve-18,
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

use crate::profiles::{KeyProfile, MechanicalClick};

#[test]
fn test_pool_instantiation() {
    let pool = VoicePool::new();
    assert_eq!(pool.polyphony(), MAX_POLYPHONY);
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn test_single_trigger_activates_one_voice() {
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
    let model = model_at_default_profile();
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
    let model = model_at_default_profile();
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
fn test_reset_clears_all_voices() {
    let model = model_at_default_profile();
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
fn test_all_voices_naturally_decay() {
    let model = model_at_default_profile();
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
