// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The Zactrix Phase 0 acoustic audit harness
//! (night-private-research-2xblade).
//!
//! Renders every shipped preset through the REAL VoicePool render
//! path — the same `MechanicalClick` model, the same micro-
//! randomization plumbing, the same per-sample loop the live player
//! runs — at 96 kHz, mono, one keypress, 500 ms window, with the
//! micro-randomization seed fixed so every run is reproducible. It
//! then measures the four physical movement metrics over the 0-100 ms
//! onset window and prints the 24x4 matrix, the tier counts, and the
//! Phase 0 gate verdict.
//!
//! Two modes:
//! - default       — the full 24-preset matrix plus the gate verdict
//! - `--preset X`  — one preset, one row, per-metric in-band verdicts
//!   (the verifier surface Phase 1 tunes against)
//!
//! The harness includes the engine and profiles module trees in place
//! via #[path]; it adds no dependency to the main package and touches
//! no audio device.

// The included trees expose their full public surface; the harness
// consumes a subset (the render path only). The main crate's own
// consumers normally absorb the rest, so silence the dead-code
// warnings here rather than thinning the shared source.
#![allow(dead_code)]

#[path = "../../../../src/profiles/mod.rs"]
mod profiles;

#[path = "../../../../src/engine/mod.rs"]
mod engine;

mod fft;
mod metrics;

use engine::VoicePool;
use profiles::{KeyTrigger, MechanicalClick, registry_all};

use metrics::Metrics;

/// Audit render rate, Hz (the prompt locks 96 kHz).
const RENDER_RATE: u32 = 96_000;

/// Render window, samples: 500 ms at 96 kHz.
const RENDER_SAMPLES: usize = 48_000;

/// Render one keypress of one preset to mono through the real render
/// path, with the micro-randomization seed fixed (fresh model
/// instance, counter reset — the documented reproducibility hooks).
fn render_mono(profile: profiles::KeyProfile) -> Vec<f32> {
    let model = MechanicalClick::with_profile(profile, RENDER_RATE);
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
    let mut mono = Vec::with_capacity(RENDER_SAMPLES);
    for _ in 0..RENDER_SAMPLES {
        let [left, right] = pool.process_sample(&model);
        mono.push((left + right) * 0.5);
    }
    mono
}

/// One matrix row: name + the four metrics + the tier.
fn row(name: &str, m: Metrics) -> String {
    let v = m.verdict();
    format!(
        "| `{name}` | {:.0} | {:.3} | {:.0} | {:+.1} | {v:?} |",
        m.centroid_drift_hz,
        m.flatness_evolution,
        m.modal_spread_db_s,
        m.attack_tail_db,
        v = v.tier
    )
}

/// One matrix row in raw-metric form (no tier) for the single-preset
/// verifier mode, with the per-metric in-band verdicts spelled out.
fn verify_row(name: &str, m: Metrics) -> String {
    let v = m.verdict();
    format!(
        "| `{name}` | {:.0} ({}) | {:.3} ({}) | {:.0} ({}) | {:+.1} ({}) | {} |",
        m.centroid_drift_hz,
        mark(v.centroid_in_band),
        m.flatness_evolution,
        mark(v.flatness_in_band),
        m.modal_spread_db_s,
        mark(v.modal_in_band),
        m.attack_tail_db,
        mark(v.attack_tail_in_band),
        v.tier
    )
}

/// Pass/fail marker for the verifier row.
fn mark(in_band: bool) -> &'static str {
    if in_band { "in band" } else { "OUT" }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Single-preset verifier mode: same four metrics, per-metric
    // verdicts, no gate math. Unknown names exit 2.
    if let Some(pos) = args.iter().position(|a| a == "--preset") {
        let name = args
            .get(pos + 1)
            .unwrap_or_else(|| panic!("--preset needs a preset name"));
        let presets = registry_all();
        let preset = presets
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("unknown preset: {name}"));
        println!(
            "| preset | centroid drift Hz | flatness evolution | modal spread dB/s | attack/tail dB | tier |"
        );
        println!("|---|---|---|---|---|---|");
        let m = metrics::measure(&render_mono(preset.profile), RENDER_RATE);
        println!("{}", verify_row(preset.name, m));
        return;
    }

    // Full matrix mode.
    let presets = registry_all();
    println!(
        "| preset | centroid drift Hz | flatness evolution | modal spread dB/s | attack/tail dB | tier |"
    );
    println!("|---|---|---|---|---|---|");

    let mut a = 0_usize;
    let mut b = 0_usize;
    let mut c = 0_usize;
    for preset in presets {
        let m = metrics::measure(&render_mono(preset.profile), RENDER_RATE);
        match m.verdict().tier {
            'A' => a += 1,
            'B' => b += 1,
            _ => c += 1,
        }
        println!("{}", row(preset.name, m));
    }

    println!();
    println!("tiers: A = {a}, B = {b}, C = {c} (of {})", presets.len());

    // The Phase 0 gate: closed means the current engine already meets
    // the physical bands (zero C-tier presets and at least 20 A-tier
    // presets); the audit is then the whole deliverable. Open means
    // the data earned the time-variant upgrade.
    let gate_closed = c == 0 && a >= 20;
    println!("gate: {}", if gate_closed { "closed" } else { "open" });
}
