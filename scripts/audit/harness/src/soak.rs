// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! PROOF 1 — the long-run soak (night-private-research-2xblade-3).
//!
//! One million keystrokes across all 24 presets, round-robin, at
//! 48 kHz, one fixed-seed pass rendered end to end. The run streams:
//! nothing accumulates except the counters the contract asks for.
//!
//! Schedule honesty (recorded in docs/ZACTRIX_LTS.md): the sandbox's
//! render cost (~2 tan() per active voice per sample) makes natural
//! pacing of one million keystrokes a multi-hour render, so this
//! instrument runs the pool at MAXIMUM PRESSURE — one keystroke every
//! SOAK_INTERVAL samples (no release events; with the pool full, the
//! oldest-first stealing IS the voice terminator). The compression
//! rides the keystroke schedule, not the DSP: every keystroke still
//! triggers, renders, and dies through the real path, and the
//! natural-lifetime dimension is covered by PROOF 5 (one hour of
//! audio with releases) and PROOF 4 (million-sample raw renders).
//!
//! Measured dimensions (the contract, verbatim):
//! (a) cumulative output drift — the FNV-1a hash of the final 1000
//!     samples; the wrapper script runs the whole mode TWICE and the
//!     two hashes must be byte-identical;
//! (b) memory growth — RSS at keystroke 1000 vs keystroke 1,000,000
//!     (target: < 1 MB total growth);
//! (c) CPU drift — wall-clock per 1000-keystroke block, first vs
//!     last (target: within ±5%);
//! (d) NaN/Inf on the raw path — the pool's NaN-to-0 net hides
//!     internal failures behind silence, so the soak counts the net's
//!     own signature on the output stream (an exact-zero sample pair
//!     right after a live pair) AND renders a raw-path spot check
//!     every SOAK_SPOT_EVERY keystrokes through a SEPARATE model
//!     instance (fresh counter — the main chain stays untouched and
//!     deterministic), counting NaN/Inf directly.

use crate::engine::VoicePool;
use crate::profiles::{AcousticModel, KeyTrigger, MechanicalClick, SynthState, registry_all};
use crate::proof::{fnv1a_f32, rss_kib};
use std::time::Instant;

const RATE: u32 = 48_000;
const KEYS: usize = 1_000_000;
/// Keystroke interval in samples: the max-pressure schedule (the pool
/// saturated, stealing continuous). 48 samples = 1000 keystrokes/s.
const SOAK_INTERVAL: usize = 48;
/// Raw-path spot check cadence: every 50,000th keystroke (20 checks).
const SOAK_SPOT_EVERY: usize = 50_000;
/// Per-keystroke spot-check window cap (samples).
const SPOT_CAP: usize = 1_000_000;

/// The net's signature + finiteness + peak, streamed per chunk.
struct StreamProbe {
    max_abs: f32,
    net_zero_steps: usize,
}

pub(super) fn run_soak() {
    let presets = registry_all();
    println!(
        "PROOF 1 (night-private-research-2xblade-3): long-run soak, {KEYS} keystrokes, round-robin over {} presets, {} kHz, interval {SOAK_INTERVAL} samples ({} keystrokes/s, pool saturated - the documented max-pressure schedule)",
        presets.len(),
        RATE / 1000,
        RATE as usize / SOAK_INTERVAL
    );
    println!("hardware: {}", crate::proof::cpu_model());

    // One model per preset, fresh counters: the deterministic chain.
    let models: Vec<MechanicalClick> = presets
        .iter()
        .map(|p| {
            let m = MechanicalClick::with_profile(p.profile, RATE);
            m.reset_keystroke_counter_for_tests();
            m
        })
        .collect();

    let mut pool = VoicePool::with_volume(5.5);
    let mut stream = StreamProbe {
        max_abs: 0.0,
        net_zero_steps: 0,
    };
    let mut prev = [0.0_f32; 2];
    // Rolling tail: the last 1000 mono samples, streamed.
    let mut tail: Vec<f32> = Vec::with_capacity(1000);
    // Per-1000-keystroke wall-clock blocks.
    let mut block_first: f64 = 0.0;
    let mut block_last: f64 = 0.0;
    let mut block_max: f64 = 0.0;
    let mut block_min: f64 = f64::INFINITY;
    let mut block_times: Vec<f64> = Vec::with_capacity(KEYS / 1000);
    let mut block_t0 = Instant::now();
    // RSS checkpoints.
    let mut rss_at_1000: u64 = 0;
    let mut peak_active: usize = 0;
    let mut active_sum: u64 = 0;
    let mut active_n: u64 = 0;
    // Raw spot-check totals.
    let mut spot_checks = 0_usize;
    let mut spot_nan = 0_usize;
    let mut spot_inf = 0_usize;

    let t_start = Instant::now();
    let mut rendered_total = 0_usize;

    for key in 0..KEYS {
        // Render up to the keystroke's sample position.
        let target = key * SOAK_INTERVAL;
        while rendered_total < target {
            let [l, r] = pool.process_sample(&models[key % models.len()]);
            feed(
                &mut stream,
                &mut prev,
                &mut tail,
                rendered_total,
                KEYS,
                l,
                r,
            );
            rendered_total += 1;
        }
        // Trigger the keystroke (round-robin preset, cycling scancode).
        let model = &models[key % models.len()];
        let _ = model.record_trigger_timestamp(
            key as u64 * (SOAK_INTERVAL as u64) * 1_000_000 / RATE as u64,
        );
        pool.trigger(
            model,
            &KeyTrigger {
                scancode: (key % 60 + 1) as u32,
                pressed: true,
                stereo_position: ((key % 21) as f32 - 10.0) / 10.0,
                velocity: None,
            },
        );
        let ac = pool.active_count();
        if ac > peak_active {
            peak_active = ac;
        }
        active_sum += ac as u64;
        active_n += 1;

        // RSS at the contract's checkpoint.
        if key == 1000 {
            rss_at_1000 = rss_kib();
        }

        // Raw-path spot check on a SEPARATE model chain (deterministic,
        // independent of the main chain).
        if key > 0 && key % SOAK_SPOT_EVERY == 0 {
            let spot_model =
                MechanicalClick::with_profile(presets[key % presets.len()].profile, RATE);
            spot_model.reset_keystroke_counter_for_tests();
            let mut st = SynthState::default();
            spot_model.init_state(&presets[key % presets.len()].profile, &mut st, 0.0, None);
            for _ in 0..SPOT_CAP {
                if !st.active {
                    break;
                }
                let [l, r] = spot_model.render_sample(&mut st);
                for v in [l, r] {
                    if v.is_nan() {
                        spot_nan += 1;
                    } else if v.is_infinite() {
                        spot_inf += 1;
                    }
                }
            }
            spot_checks += 1;
        }

        // Wall-clock per 1000-keystroke block: the elapsed time since
        // the previous block boundary (render + trigger + checks).
        if (key + 1) % 1000 == 0 {
            let dt = block_t0.elapsed().as_secs_f64() * 1000.0;
            block_t0 = Instant::now();
            block_times.push(dt);
            if block_first == 0.0 {
                block_first = dt;
            }
            block_last = dt;
            if dt > block_max {
                block_max = dt;
            }
            if dt < block_min {
                block_min = dt;
            }
        }
    }

    // Drain: every voice must reach its natural end (no releases in
    // this schedule; the last 16 voices ring out).
    while pool.is_active() {
        let [l, r] = pool.process_sample(&models[KEYS % models.len()]);
        feed(
            &mut stream,
            &mut prev,
            &mut tail,
            rendered_total,
            KEYS,
            l,
            r,
        );
        rendered_total += 1;
    }
    let active_end = pool.active_count();
    let wall = t_start.elapsed().as_secs_f64();

    let hash = fnv1a_f32(0xcbf2_9ce4_8422_2325, &tail);
    let rss_end = rss_kib();
    let rss_growth_kib = rss_end.saturating_sub(rss_at_1000);
    let drift_pct = if block_first > 0.0 {
        (block_last - block_first) / block_first * 100.0
    } else {
        f64::INFINITY
    };

    println!(
        "rendered {rendered_total} frames ({:.1} audio-seconds) in {wall:.1} s wall; peak active voices {peak_active}, mean active {:.2}",
        rendered_total as f64 / RATE as f64,
        active_sum as f64 / active_n.max(1) as f64
    );
    println!(
        "(b) memory: RSS at keystroke 1000 = {rss_at_1000} KiB, at keystroke {KEYS} = {rss_end} KiB, growth {rss_growth_kib} KiB (target < 1024 KiB)"
    );
    println!(
        "(c) CPU drift: per-1000-keystroke block first {block_first:.1} ms, last {block_last:.1} ms, drift {drift_pct:+.2}% (target within +/-5%); block min {block_min:.1} / max {block_max:.1} ms over {} blocks",
        block_times.len()
    );
    println!(
        "(d) NaN/Inf: raw spot checks {spot_checks} x <= {SPOT_CAP}-sample full lifetimes -> NaN {spot_nan}, Inf {spot_inf} (target 0); pool output stream: net-zero steps {} , max |sample| {:.4} (post-clamp <= 1.0)",
        stream.net_zero_steps, stream.max_abs
    );
    println!(
        "pool end state: active voices {active_end} after drain (target 0 - every slot returned)"
    );
    println!(
        "(a) final-1000-sample hash (fnv1a64): {hash:016x}  <- the wrapper runs this mode twice; the hashes must be byte-identical"
    );
    let ok = spot_nan == 0
        && spot_inf == 0
        && stream.net_zero_steps == 0
        && active_end == 0
        && rss_growth_kib < 1024
        && drift_pct.abs() <= 5.0
        && block_times.len() == KEYS / 1000;
    println!(
        "PROOF 1 {}",
        if ok {
            "PASS: numerics clean end to end, pool returned, RSS flat, CPU steady (hash equality is the wrapper's second run)"
        } else {
            "FAIL: the failing dimension is named above and in docs/ZACTRIX_LTS.md"
        }
    );
}

/// Stream one output frame into the probe: peak, net signature, tail.
#[inline]
fn feed(
    stream: &mut StreamProbe,
    prev: &mut [f32; 2],
    tail: &mut Vec<f32>,
    rendered: usize,
    keys: usize,
    l: f32,
    r: f32,
) {
    let a = l.abs().max(r.abs());
    if a > stream.max_abs {
        stream.max_abs = a;
    }
    if prev[0].abs() > 1e-3 && prev[1].abs() > 1e-3 && l == 0.0 && r == 0.0 {
        stream.net_zero_steps += 1;
    }
    *prev = [l, r];
    // Keep only the final 1000 mono samples: once inside the last
    // keystroke's window, push; before that, keep the buffer empty.
    let tail_start = keys * SOAK_INTERVAL;
    if rendered + 1 > tail_start.saturating_sub(1000) {
        if tail.len() == 1000 {
            tail.remove(0);
        }
        tail.push((l + r) * 0.5);
    }
}
