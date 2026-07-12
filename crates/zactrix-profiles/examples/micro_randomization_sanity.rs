// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Sanity test for the v5.0.0 micro-randomization: render two voices
//! with the SAME profile and verify their waveforms differ. Before
//! v5.0.0 the waveforms were bit-identical (deterministic synthesis
//! falling into the "uncanny valley"). After v5.0.0 they should
//! differ in:
//!   - noise seed (different xorshift32 starting state)
//!   - pitch drift (±1.5% on click/spring/housing frequencies)
//!   - amplitude drift (±5% on excitation envelope)
//!
//! Run with:
//!   cargo run --example micro_randomization_sanity -p zactrix-profiles

use zactrix_profiles::{AcousticModel, KeyProfile, KeyTrigger, MechanicalClick, SynthState};

fn render_waveform(profile: KeyProfile, n: usize) -> Vec<[f32; 2]> {
    let model = MechanicalClick::with_profile(profile, 44_100);
    let event = KeyTrigger {
        scancode: 30,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };
    let p = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&p, &mut state, event.stereo_position, None);

    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(model.render_sample(&mut state));
        if !state.active {
            break;
        }
    }
    out
}

fn main() {
    let profile = KeyProfile::default();

    let wave_a = render_waveform(profile, 2000);
    let wave_b = render_waveform(profile, 2000);
    let wave_c = render_waveform(profile, 2000);

    // Verify all three are different. Compare sample-by-sample; if any
    // sample differs, the waveforms differ. We expect the very first
    // sample to differ (different noise seed → different first xorshift
    // output → different excitation value).
    let min_len = wave_a.len().min(wave_b.len()).min(wave_c.len());
    if min_len == 0 {
        println!("FAIL: voices produced no output");
        std::process::exit(1);
    }

    let mut a_vs_b_diff = 0;
    let mut a_vs_c_diff = 0;
    let mut b_vs_c_diff = 0;
    let mut max_a_vs_b_diff: f32 = 0.0;
    for i in 0..min_len {
        let [la, ra] = wave_a[i];
        let [lb, rb] = wave_b[i];
        let [lc, rc] = wave_c[i];
        let d_ab = (la - lb).abs() + (ra - rb).abs();
        let d_ac = (la - lc).abs() + (ra - rc).abs();
        let d_bc = (lb - lc).abs() + (rb - rc).abs();
        if d_ab > 1e-6 {
            a_vs_b_diff += 1;
            max_a_vs_b_diff = max_a_vs_b_diff.max(d_ab);
        }
        if d_ac > 1e-6 {
            a_vs_c_diff += 1;
        }
        if d_bc > 1e-6 {
            b_vs_c_diff += 1;
        }
    }

    println!("Rendered 3 voices with the SAME profile (each {min_len} samples)");
    println!("  A vs B: {a_vs_b_diff} samples differ (max delta = {max_a_vs_b_diff:.6})");
    println!("  A vs C: {a_vs_c_diff} samples differ");
    println!("  B vs C: {b_vs_c_diff} samples differ");

    // Pass criteria: all three pairs should differ in at least 25% of
    // samples. The early excitation burst is where most variation lives
    // (different noise seed → different first xorshift output → different
    // filter excitation); the late decay tail converges to near-zero in
    // all voices, so requiring >50% would be artificially strict. The
    // max delta reported above is the real proof — a value > 0.1 means
    // the first excitation sample was drastically different between
    // voices (full-scale audio is ~1.0).
    let threshold = min_len / 4;
    if a_vs_b_diff >= threshold
        && a_vs_c_diff >= threshold
        && b_vs_c_diff >= threshold
        && max_a_vs_b_diff > 0.1
    {
        println!(
            "\nPASS: micro-randomization produces distinct waveforms (threshold = {threshold}, max delta = {max_a_vs_b_diff:.3})"
        );
    } else {
        println!(
            "\nFAIL: micro-randomization did not produce enough variation (threshold = {threshold}, max delta = {max_a_vs_b_diff:.3})"
        );
        std::process::exit(1);
    }
}
