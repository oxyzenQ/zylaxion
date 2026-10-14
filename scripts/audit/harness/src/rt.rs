// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! PROOF 3 — RT-safety jitter measurement
//! (night-private-research-2xblade-3).
//!
//! The written audit (the no-alloc / no-lock / no-syscall reading of
//! the callback and the per-sample render path) lives in
//! docs/ZACTRIX_LTS.md; this mode is the measured half: render one
//! million samples through the real VoicePool in 64-frame chunks —
//! the exact chunk shape the orchestrator's producer loop runs
//! (MAX_RENDER_CHUNK) — and record the wall-clock time of every
//! chunk. Two passes:
//!
//! natural   — a 5 keystrokes/s stream with releases (real typing);
//! saturated — the pool held at MAX_POLYPHONY for the whole pass (a
//!             voice that dies is re-triggered at the next chunk
//!             boundary): the worst case the mixer can mix.
//!
//! Targets (verbatim from the pass contract): jitter ratio max/p50
//! < 10x; no block exceeds 5 ms wall-clock on the test hardware.

use crate::engine::VoicePool;
use crate::profiles::{AcousticModel, KeyTrigger, MAX_POLYPHONY, MechanicalClick, registry_all};
use crate::proof::cpu_model;
use std::time::Instant;

/// The orchestrator's producer chunk size (src/core/mod.rs) — the
/// shape the render loop actually runs.
const CHUNK: usize = 64;

/// Rate for the jitter pass: 48 kHz, the common device rate.
const RT_RATE: u32 = 48_000;

/// Total frames per pass, per the contract.
const RT_FRAMES: usize = 1_000_000;

/// Keystroke cadence for the natural pass: one press every 200 ms,
/// released 300 ms later (overlap with the next press, like typing).
const NATURAL_INTERVAL: usize = 9_600;
const HOLD_SAMPLES: usize = 14_400;

fn report(title: &str, mut times_us: Vec<f64>) -> (f64, f64, f64, f64) {
    times_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let blocks = times_us.len();
    let p50 = times_us[blocks / 2];
    let p99 = times_us[(blocks * 99 / 100).min(blocks - 1)];
    let max = times_us[blocks - 1];
    let ratio = if p50 > 0.0 { max / p50 } else { f64::INFINITY };
    let audio_ms = CHUNK as f64 / RT_RATE as f64 * 1000.0;
    println!(
        "{title}: {blocks} blocks of {CHUNK} frames ({audio_ms:.2} ms audio each) | p50 {p50:.1} us | p99 {p99:.1} us | max {max:.1} us | max/p50 {ratio:.2}x | max block wall-clock {:.4} ms",
        max / 1000.0
    );
    (p50, p99, max, ratio)
}

fn trigger(model: &MechanicalClick, pool: &mut VoicePool, cursor: usize, key: usize) {
    let _ = model.record_trigger_timestamp(cursor as u64 * 1_000_000 / RT_RATE as u64);
    pool.trigger(
        model,
        &KeyTrigger {
            scancode: (key % 60 + 1) as u32,
            pressed: true,
            stereo_position: ((key % 21) as f32 - 10.0) / 10.0,
            velocity: None,
        },
    );
}

/// Render `frames` frames in CHUNK-sized blocks, timing every block,
/// with `events` fired at each chunk boundary (the orchestrator's
/// event shape: input lands between chunks, never mid-chunk).
fn timed_pass(
    pool: &mut VoicePool,
    model: &MechanicalClick,
    mut events: impl FnMut(usize, &mut VoicePool),
) -> Vec<f64> {
    let mut times = Vec::with_capacity(RT_FRAMES / CHUNK + 1);
    let mut batch: [[f32; 2]; CHUNK] = [[0.0; 2]; CHUNK];
    let mut rendered = 0_usize;
    while rendered < RT_FRAMES {
        events(rendered, pool);
        let t0 = Instant::now();
        for frame in batch.iter_mut() {
            let [l, r] = pool.process_sample(model);
            *frame = [l, r];
        }
        times.push(t0.elapsed().as_secs_f64() * 1e6);
        rendered += CHUNK;
    }
    times
}

pub(super) fn run_rt() {
    println!(
        "PROOF 3 (night-private-research-2xblade-3): render-path jitter, hardware: {}, chunk = the orchestrator's MAX_RENDER_CHUNK ({CHUNK} frames), rate {RT_RATE}",
        cpu_model()
    );
    let presets = registry_all();
    let profile = presets
        .iter()
        .find(|p| p.name == "ibm_model_m")
        .unwrap_or_else(|| &presets[0])
        .profile;

    // ── Pass A: natural typing stream ──────────────────────────────
    let model = MechanicalClick::with_profile(profile, RT_RATE);
    model.reset_keystroke_counter_for_tests();
    let mut pool = VoicePool::with_volume(5.5);
    let mut next_press = 0_usize;
    let mut key = 0_usize;
    let mut holds: Vec<(usize, u32)> = Vec::new(); // (release cursor, scancode)
    let natural = timed_pass(&mut pool, &model, |cursor, pool| {
        if cursor >= next_press {
            let scancode = (key % 60 + 1) as u32;
            trigger(&model, pool, cursor, key);
            holds.push((cursor + HOLD_SAMPLES, scancode));
            key += 1;
            next_press += NATURAL_INTERVAL;
        }
        holds.retain(|&(r, sc)| {
            if cursor >= r {
                pool.release(sc);
                false
            } else {
                true
            }
        });
    });
    let a = report("pass A (natural, 5 keys/s with 300 ms holds)", natural);

    // ── Pass B: saturated pool, worst case ────────────────────────
    let model_b = MechanicalClick::with_profile(profile, RT_RATE);
    model_b.reset_keystroke_counter_for_tests();
    let mut pool_b = VoicePool::with_volume(5.5);
    let mut key_b = 0_usize;
    let saturated = timed_pass(&mut pool_b, &model_b, |cursor, pool| {
        // Hold the pool at MAX_POLYPHONY: refill every inactive slot
        // at the chunk boundary. A voice that dies mid-pass is
        // replaced before the next block renders — the worst case
        // stays the worst case for the whole pass.
        while pool.active_count() < MAX_POLYPHONY {
            trigger(&model_b, pool, cursor, key_b);
            key_b += 1;
        }
    });
    let b = report("pass B (saturated, MAX_POLYPHONY held)", saturated);

    let worst_ratio = a.3.max(b.3);
    let worst_max_us = a.2.max(b.2);
    let ok = worst_ratio < 10.0 && worst_max_us < 5_000.0;
    println!(
        "PROOF 3 verdict: worst max/p50 = {worst_ratio:.2}x (target < 10x); worst block = {:.4} ms wall (target < 5 ms); audio budget per block = {:.2} ms",
        worst_max_us / 1000.0,
        CHUNK as f64 / RT_RATE as f64 * 1000.0
    );
    println!(
        "PROOF 3 {}",
        if ok {
            "PASS: the measured worst-case chunk render sits inside both the 5 ms wall target and the 10x jitter heuristic (the written no-alloc/no-lock/no-syscall audit lives in docs/ZACTRIX_LTS.md)"
        } else {
            "FAIL: a jitter target was exceeded - see the numbers above and the written audit"
        }
    );
}
