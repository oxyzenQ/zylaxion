// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Shared proof helpers for the LTS certification harness
//! (night-private-research-2xblade-3). A child surface of the
//! standalone audit crate: no new dependency, no audio device.

use crate::engine::VoicePool;
use crate::profiles::{KeyTrigger, MechanicalClick};

/// FNV-1a 64-bit over the raw f32 bit patterns of a mono slice,
/// folded into a running hash. Deterministic across runs, platforms,
/// and std versions (no SipHash, no HashMap iteration order).
#[inline]
pub(super) fn fnv1a_f32(mut hash: u64, samples: &[f32]) -> u64 {
    for &s in samples {
        for byte in s.to_bits().to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    hash
}

/// Resident set size in KiB, read from /proc/self/status (Linux-only,
/// which is the project's whole world). Returns 0 if the file cannot
/// be read — the caller reports the number, never asserts on it.
pub(super) fn rss_kib() -> u64 {
    match std::fs::read_to_string("/proc/self/status") {
        Ok(status) => {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("VmRSS:") {
                    let kb: u64 = rest
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    return kb;
                }
            }
            0
        }
        Err(_) => 0,
    }
}

/// The CPU model line from /proc/cpuinfo, for the instrument record.
pub(super) fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|info| {
            info.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|m| m.trim().to_string())
        })
        .unwrap_or_else(|| "unknown cpu".to_string())
}

/// Render one keypress of one preset to mono through the real
/// VoicePool path at an arbitrary rate and window — the audit's
/// `render_mono`, generalized. Fresh model instance with the counter
/// reset and a fresh pool: the documented reproducibility hooks.
pub(super) fn render_mono_pool(
    profile: crate::profiles::KeyProfile,
    rate: u32,
    samples: usize,
) -> Vec<f32> {
    let model = MechanicalClick::with_profile(profile, rate);
    model.reset_keystroke_counter_for_tests();
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
    let mut mono = Vec::with_capacity(samples);
    for _ in 0..samples {
        let [left, right] = pool.process_sample(&model);
        mono.push((left + right) * 0.5);
    }
    mono
}

/// Ring decay rate, dB/s: least-squares slope of the log envelope over
/// the local maxima of |v| inside the fixed [-25, -50] dB window below
/// the slice peak, over the first `live` samples. The c^(R/S)
/// retention law's measurand. Same algorithm the stability battery
/// runs inline; extracted here so the rates mode measures the same
/// quantity with the same code (the battery's output is verified
/// byte-identical after the extraction).
pub(super) fn ring_decay_fit(samples: &[f32], rate: u32, live: usize) -> f64 {
    let peak = samples.iter().fold(0.0_f32, |m, &v| m.max(v.abs()));
    if peak <= 0.0 {
        return 0.0;
    }
    let win_hi = peak * 10.0_f32.powf(-25.0 / 20.0);
    let win_lo = peak * 10.0_f32.powf(-50.0 / 20.0);
    let live = live.min(samples.len());
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut i = 1;
    while i + 1 < live {
        let a = samples[i - 1].abs();
        let b = samples[i].abs();
        let c = samples[i + 1].abs();
        if b >= a && b >= c && b >= win_lo && b <= win_hi {
            pts.push((i as f64 / rate as f64, (b as f64).ln()));
            i += 2;
        } else {
            i += 1;
        }
    }
    if pts.len() < 32 {
        return 0.0;
    }
    let n = pts.len() as f64;
    let sx: f64 = pts.iter().map(|p| p.0).sum();
    let sy: f64 = pts.iter().map(|p| p.1).sum();
    let sxx: f64 = pts.iter().map(|p| p.0 * p.0).sum();
    let sxy: f64 = pts.iter().map(|p| p.0 * p.1).sum();
    let denom = n * sxx - sx * sx;
    if denom.abs() <= 1e-12 {
        return 0.0;
    }
    let slope = (n * sxy - sx * sy) / denom;
    -slope * 8.685_889_638
}

/// The -60 dB tail end point of a mono render, in milliseconds (the
/// last index at/above the floor below the render peak). NOT the
/// c^(R/S) law's measurand — see the stability battery's note — but
/// PROOF 2 measures it across rates and records the spread.
pub(super) fn tail_end_ms(samples: &[f32], rate: u32) -> f64 {
    let peak = samples.iter().fold(0.0_f32, |m, &v| m.max(v.abs()));
    if peak <= 0.0 {
        return 0.0;
    }
    let threshold = peak * 10.0_f32.powf(-60.0 / 20.0);
    match samples.iter().rposition(|&v| v.abs() >= threshold) {
        Some(idx) => idx as f64 / rate as f64 * 1000.0,
        None => 0.0,
    }
}

/// The pool's own post-voice law, applied sample-wise: multiply by the
/// linear gain, then the exact `process_sample` tail — is_finite check
/// first (NaN.clamp is NaN), then the hard clamp to [-1, 1], non-finite
/// to 0.0. One voice's raw render equals the pool's voice sum, so this
/// reproduces `process_sample`'s arithmetic bit-for-bit.
#[inline]
pub(super) fn pool_output_law(raw: f32, gain: f32) -> f32 {
    let v = raw * gain;
    if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// The c^(R/S) law's exact measurand: the RAW voice lifetime in
/// milliseconds — render one keypress through the raw model path (no
/// pool) and count samples until `state.active` clears (the envelope
/// chain crossing the voice-off threshold). The pool output stream is
/// deliberately NOT used for this number: its last nonzero sample
/// rides the ring's own denormal crossing, which conflates the filter
/// physics with the retention law. Cap bounds the render window.
pub(super) fn raw_voice_lifetime_ms(
    profile: crate::profiles::KeyProfile,
    rate: u32,
    cap_samples: usize,
) -> f64 {
    use crate::profiles::{AcousticModel, SynthState};
    let model = MechanicalClick::with_profile(profile, rate);
    model.reset_keystroke_counter_for_tests();
    let mut state = SynthState::default();
    model.init_state(&profile, &mut state, 0.0, None);
    let mut live = 0_usize;
    while state.active && live < cap_samples {
        let [l, r] = model.render_sample(&mut state);
        let _ = (l, r);
        live += 1;
    }
    live as f64 / rate as f64 * 1000.0
}
