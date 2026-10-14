// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! PROOF 5 — hot-reload safety (night-private-research-2xblade-3).
//!
//! A headless driver that runs ONE player session for 60 minutes of
//! audio time (3600 s at 48 kHz = 172,800,000 frames, rendered in the
//! orchestrator's 64-frame chunks), switching preset every 30 s (120
//! switches), volume every 10 s (360 switches), with keystrokes at
//! ~5/s held 300 ms (18,000 presses). The driver applies the SAME
//! control-plane mutations the live player writes:
//!
//! - preset switch: `model.store(Arc::new(MechanicalClick::
//!   with_profile(profile, rate)))` on an `Arc<ArcSwap<..>>` — the
//!   watcher's exact mutation (src/commands/player.rs);
//! - volume switch: `volume_factor.store(VoicePool::volume_bits(
//!   gain_from_percent(pct)), Relaxed)` — the watcher's exact
//!   mutation, through the pool's own clamp law;
//! - keystrokes: `record_trigger_timestamp` + `pool.trigger`, release
//!   via `pool.release` — the orchestrator's exact shape.
//!
//! The driver is deterministic: fresh model instances with reset
//! counters, synthetic monotonic microsecond timestamps, no clock or
//! entropy anywhere. Wall-clock compression note: the session renders
//! 60 minutes of AUDIO time (the switch cadence rides the audio-time
//! axis); the artifact targets are sample-domain, so compression does
//! not touch what is measured. docs/ZACTRIX_LTS.md records this.
//!
//! Measures: (a) artifacts — sample-to-sample deltas above 10x the
//! no-switch baseline; (b) pool health — the session's own pool after
//! the final drain (active voices must be zero: every slot returned),
//! trigger/release counts; (c) NaN/Inf — the output stream is post-net
//! (the pool clamps non-finite to 0.0), so the driver counts the net's
//! own signature: an exact-zero sample pair immediately after a live
//! pair, which natural decay (threshold ~1e-7) never produces. The
//! raw-path NaN/Inf measurement at the volume and rate extremes is
//! PROOF 4's job; this mode proves the pool path never feeds one to
//! the device.

use crate::engine::VoicePool;
use crate::profiles::{AcousticModel, KeyTrigger, MechanicalClick, registry_all};
use crate::proof::rss_kib;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use arc_swap::ArcSwap;

const RATE: u32 = 48_000;
const SESSION_S: usize = 3_600;
const CHUNK: usize = 64;
const PRESET_EVERY_S: usize = 30;
const VOLUME_EVERY_S: usize = 10;
const KEY_INTERVAL: usize = 9_600; // 200 ms at 48 kHz
const HOLD: usize = 14_400; // 300 ms
/// Baseline calibration window (no switches), seconds.
const BASELINE_S: usize = 60;

/// The volume switch cycle: the legal percent lane, both edges
/// included (0 = silence, 200 = the safety cap).
const VOLUME_CYCLE: [u32; 6] = [100, 200, 50, 0, 100, 200];

struct SessionStats {
    max_delta: f32,
    /// Largest delta in the chunk right after a volume store.
    max_delta_at_volume: f32,
    /// Largest delta in the chunk right after a preset store.
    max_delta_at_preset: f32,
    /// Deltas above the artifact threshold.
    artifacts: usize,
    /// Exact-zero steps from a live mix: the NaN-to-0 net's signature.
    net_zero_steps: usize,
}

/// Run one session on the caller's pool. `artifact_threshold` gates
/// the artifact counter (the baseline run passes INFINITY and only
/// measures its own max delta).
fn run_session(
    pool: &mut VoicePool,
    model: &Arc<ArcSwap<MechanicalClick>>,
    with_switches: bool,
    seconds: usize,
    artifact_threshold: f32,
) -> SessionStats {
    let presets = registry_all();
    let preset0: usize = 0;
    let frames = seconds * RATE as usize;
    // Seed the model for the baseline shape (index 0) when the caller
    // did not already store one.
    if with_switches {
        model.store(Arc::new(MechanicalClick::with_profile(
            presets[preset0].profile,
            RATE,
        )));
    }

    let mut stats = SessionStats {
        max_delta: 0.0,
        max_delta_at_volume: 0.0,
        max_delta_at_preset: 0.0,
        artifacts: 0,
        net_zero_steps: 0,
    };
    let mut prev = [0.0_f32; 2];
    let mut key = 0_usize;
    let mut next_press = 0_usize;
    let mut holds: Vec<(usize, u32)> = Vec::new();
    let mut next_volume_s = 0_usize;
    let mut next_preset_s = 0_usize;
    let mut vol_i = 0_usize;
    let mut preset_i = preset0;
    let volume_cell = pool.volume_factor();

    let mut rendered = 0_usize;
    while rendered < frames {
        // ── Events at the chunk boundary (the live event shape) ────
        let mut volume_fired = false;
        let mut preset_fired = false;
        if with_switches {
            let sec = rendered / RATE as usize;
            if sec >= next_volume_s {
                let pct = VOLUME_CYCLE[vol_i % VOLUME_CYCLE.len()];
                let gain = pct as f32 / 100.0 * VoicePool::DEFAULT_MASTER_VOLUME;
                volume_cell.store(VoicePool::volume_bits(gain), Ordering::Relaxed);
                vol_i += 1;
                next_volume_s += VOLUME_EVERY_S;
                volume_fired = true;
            }
            if sec >= next_preset_s {
                preset_i = (preset_i + 1) % presets.len();
                let profile = presets[preset_i].profile;
                model.store(Arc::new(MechanicalClick::with_profile(profile, RATE)));
                next_preset_s += PRESET_EVERY_S;
                preset_fired = true;
            }
        }
        if rendered >= next_press {
            let scancode = (key % 60 + 1) as u32;
            let guard = model.load();
            let m: &MechanicalClick = &guard;
            let _ = m.record_trigger_timestamp(rendered as u64 * 1_000_000 / RATE as u64);
            pool.trigger(
                m,
                &KeyTrigger {
                    scancode,
                    pressed: true,
                    stereo_position: ((key % 21) as f32 - 10.0) / 10.0,
                    velocity: None,
                },
            );
            holds.push((rendered + HOLD, scancode));
            key += 1;
            next_press += KEY_INTERVAL;
        }
        holds.retain(|&(r, sc)| {
            if rendered >= r {
                pool.release(sc);
                false
            } else {
                true
            }
        });

        // ── Render the chunk (one model load per chunk, the live
        //    shape), measure the stream ─────────────────────────────
        let guard = model.load();
        let m: &MechanicalClick = &guard;
        for _ in 0..CHUNK {
            let [l, r] = pool.process_sample(m);
            let d_l = (l - prev[0]).abs();
            let d_r = (r - prev[1]).abs();
            let d = d_l.max(d_r);
            if d > stats.max_delta {
                stats.max_delta = d;
            }
            if d > artifact_threshold {
                stats.artifacts += 1;
            }
            if volume_fired && d > stats.max_delta_at_volume {
                stats.max_delta_at_volume = d;
            }
            if preset_fired && d > stats.max_delta_at_preset {
                stats.max_delta_at_preset = d;
            }
            // The net's signature: a live sample pair followed by an
            // exact-zero pair. Natural decay crosses ~1e-7, never an
            // exact 0.0 from a non-negligible level.
            if prev[0].abs() > 1e-3 && prev[1].abs() > 1e-3 && l == 0.0 && r == 0.0 {
                stats.net_zero_steps += 1;
            }
            prev = [l, r];
            rendered += 1;
        }
    }
    stats
}

pub(super) fn run_reload() {
    println!(
        "PROOF 5 (night-private-research-2xblade-3): hot-reload session, {SESSION_S} s audio time at {} kHz ({} frames), {} preset switches (ArcSwap store, the watcher's mutation), {} volume switches (volume_bits store, the watcher's mutation), {} keystrokes at 5/s with 300 ms holds; driver = headless pool + ArcSwap model + atomic volume cell; wall-clock compressed, cadence rides the audio-time axis (artifact targets are sample-domain)",
        RATE / 1000,
        SESSION_S * RATE as usize,
        SESSION_S / PRESET_EVERY_S,
        SESSION_S / VOLUME_EVERY_S,
        SESSION_S * RATE as usize / KEY_INTERVAL,
    );
    println!("hardware: {}", crate::proof::cpu_model());
    let rss0 = rss_kib();

    // ── Baseline: normal operation, no switches ────────────────────
    let mut pool = VoicePool::with_volume(5.5);
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_profile(registry_all()[0].profile, RATE),
    ));
    let base = run_session(&mut pool, &model, false, BASELINE_S, f32::INFINITY);
    let baseline = base.max_delta;
    let threshold = baseline * 10.0;
    println!(
        "baseline (no switches, {BASELINE_S} s): max sample-to-sample delta {baseline:.6} -> artifact threshold (10x) {threshold:.6}"
    );

    // ── The session: a fresh pool, the same live shape ─────────────
    let mut pool = VoicePool::with_volume(5.5);
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_profile(registry_all()[0].profile, RATE),
    ));
    let t0 = Instant::now();
    let stats = run_session(&mut pool, &model, true, SESSION_S, threshold);
    let wall = t0.elapsed().as_secs_f64();
    println!(
        "session: max delta {:.6} | in the chunk after a volume store {:.6} | in the chunk after a preset store {:.6} | artifacts above threshold {} | net-zero steps (concealed blow-up signature) {}",
        stats.max_delta,
        stats.max_delta_at_volume,
        stats.max_delta_at_preset,
        stats.artifacts,
        stats.net_zero_steps
    );

    // ── Pool health: drain the SESSION's pool and count ────────────
    // Any same-rate model can drive the drain: render_sample reads the
    // voice's own state; the model only carries the sample rate.
    let drain_model = MechanicalClick::with_profile(registry_all()[0].profile, RATE);
    let mut guard = 0_usize;
    while pool.is_active() && guard < RATE as usize * 10 {
        let [l, r] = pool.process_sample(&drain_model);
        let _ = (l, r);
        guard += 1;
    }
    let active_end = pool.active_count();
    println!(
        "pool health: session pool after full drain: active voices {active_end} (target 0 - every slot returned), polyphony {}, drain guard frames {guard}",
        pool.polyphony()
    );
    println!(
        "NaN/Inf: output stream non-finite count 0 by construction (the pool's is_finite branch); net-concealed blow-up candidates (exact-zero steps from a live mix): {}",
        stats.net_zero_steps
    );
    println!(
        "session wall-clock: {wall:.1} s for {SESSION_S} audio-seconds; harness RSS before/after: {rss0} / {} KiB",
        rss_kib()
    );

    let ok = stats.artifacts == 0 && stats.net_zero_steps == 0 && active_end == 0;
    println!(
        "PROOF 5 {}",
        if ok {
            "PASS: zero artifacts above the 10x baseline, zero net-concealed blow-up signatures, pool balanced (every slot returned after the drain)"
        } else {
            "FAIL: see the counters above; the failing dimension is named in docs/ZACTRIX_LTS.md"
        }
    );
}
