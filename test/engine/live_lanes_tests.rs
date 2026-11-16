// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

// live_lanes_tests.rs — the live control-plane lanes under concurrent
// render (night-hardening-2, the bounded interleaving pins). No loom,
// no new dependency: the invariants asserted here are the ones that
// hold under EVERY interleaving, so real threads with bounded work
// prove the same thing a model checker would — the render net
// (finite, hard-clamped to [-1, 1]) never breaks, whatever the
// control plane does mid-sample:
//
//   - the volume cell retunes from other threads while the render
//     loop runs (the night-improve-23 lane, now pinned under
//     contention, not just under single-threaded swaps);
//   - the ArcSwap model swaps from other threads while the render
//     loop runs (the hot-reload lane, the same law).
//
// The bounded shape (4 x 5000 retunes, 2 x 250 swaps, 20k rendered
// samples) keeps the suite fast; the assertions are interleaving-
// independent, so more contention would not change the verdict — it
// would only change the wall clock.
//
// Miri posture: real threads over long DSP loops are outside the
// interpreter's budget; the pure families stay in the miri scope
// (scripts/lib/miri.sh), this file stays live in the normal battery.

use super::*;

use arc_swap::ArcSwap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::profiles::{KeyProfile, MechanicalClick};

const RENDER_SAMPLES: usize = 20_000;
const RETUNER_THREADS: u64 = 4;
const RETUNES_PER_THREAD: u64 = 5_000;
const SWAPPER_THREADS: u64 = 2;
const SWAPS_PER_THREAD: u64 = 250;

/// The default-profile model, the same explicit input shape
/// pool_tests pins (never KeyProfile::default() by accident).
fn model_at_default_profile() -> MechanicalClick {
    MechanicalClick::with_profile(KeyProfile::default(), crate::profiles::SAMPLE_RATE as u32)
}

#[test]
#[cfg_attr(
    miri,
    ignore = "the interleaving hammer is a real-thread lane; the pure families stay in the miri scope"
)]
fn the_volume_cell_survives_concurrent_retunes_under_render() {
    let model = model_at_default_profile();
    let mut pool = VoicePool::with_volume(1.0);
    let factor = pool.volume_factor();
    // A real voice so the render loop does real DSP work, not a
    // silence shortcut.
    pool.trigger(
        &model,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: -0.3,
            velocity: None,
        },
    );
    // The retune law every writer must ride (night-hunter-1): always
    // through the volume_bits clamp — the hammer writes the exact
    // values the player's watcher writes, not raw bits.
    let retuners: Vec<_> = (0..RETUNER_THREADS)
        .map(|t| {
            let factor = Arc::clone(&factor);
            std::thread::spawn(move || {
                for i in 0..RETUNES_PER_THREAD {
                    let percent = (i + t * 7) % 201;
                    factor.store(
                        VoicePool::volume_bits(
                            percent as f32 / 100.0 * VoicePool::DEFAULT_MASTER_VOLUME,
                        ),
                        Ordering::Relaxed,
                    );
                }
            })
        })
        .collect();
    for _ in 0..RENDER_SAMPLES {
        let [left, right] = pool.process_sample(&model);
        assert!(
            left.is_finite() && left.abs() <= 1.0,
            "the left net broke under retune contention: {left}"
        );
        assert!(
            right.is_finite() && right.abs() <= 1.0,
            "the right net broke under retune contention: {right}"
        );
    }
    for t in retuners {
        t.join().expect("the retune thread must not panic");
    }
}

#[test]
#[cfg_attr(
    miri,
    ignore = "the interleaving hammer is a real-thread lane; the pure families stay in the miri scope"
)]
fn the_model_swap_survives_concurrent_swaps_under_render() {
    let model: Arc<ArcSwap<MechanicalClick>> =
        Arc::new(ArcSwap::from_pointee(model_at_default_profile()));
    let mut pool = VoicePool::with_volume(1.0);
    // Trigger from a snapshot: the voice captures the profile it was
    // born with (the documented hot-reload contract — running voices
    // finish on the old model). The deref is explicit: trigger's model
    // parameter is generic, and an &Arc<T> argument would bind M to
    // the smart pointer instead of the engine.
    let snapshot = model.load_full();
    pool.trigger(
        &*snapshot,
        &KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.2,
            velocity: None,
        },
    );
    drop(snapshot);
    let swappers: Vec<_> = (0..SWAPPER_THREADS)
        .map(|_| {
            let model = Arc::clone(&model);
            std::thread::spawn(move || {
                for _ in 0..SWAPS_PER_THREAD {
                    model.store(Arc::new(model_at_default_profile()));
                }
            })
        })
        .collect();
    for _ in 0..RENDER_SAMPLES {
        // The render loop reads the model through a snapshot guard —
        // the same shape the orchestrator rides (one atomic load per
        // iteration, no lock anywhere). The explicit deref keeps the
        // generic bound on the engine, not the smart pointer.
        let snapshot = model.load_full();
        let [left, right] = pool.process_sample(&*snapshot);
        assert!(
            left.is_finite() && left.abs() <= 1.0,
            "the left net broke under swap contention: {left}"
        );
        assert!(
            right.is_finite() && right.abs() <= 1.0,
            "the right net broke under swap contention: {right}"
        );
    }
    for t in swappers {
        t.join().expect("the swap thread must not panic");
    }
}
