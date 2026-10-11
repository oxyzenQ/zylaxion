// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// tpt_tests.rs — the tpt.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

#[test]
fn test_svf_stability_sine_sweep() {
    let mut svf = TptSvf::new(crate::SAMPLE_RATE);
    let mut max_val: f32 = 0.0;

    // Sweep frequency from 100 Hz to 10 kHz over 1 second.
    // A naive biquad would blow up here; TPT should remain stable.
    for i in 0..44_100usize {
        let t = i as f32 / 44_100.0;
        let freq = 100.0 * (100.0_f32).powf(t);
        svf.set_params(freq, 2.0, crate::SAMPLE_RATE);

        let input = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
        let (lp, hp, bp) = svf.process(input);

        max_val = max_val.max(lp.abs()).max(hp.abs()).max(bp.abs());
    }

    assert!(
        max_val.is_finite() && max_val < 100.0,
        "SVF output blew up: max={max_val}"
    );
}

#[test]
fn test_svf_impulse_response_decays() {
    let mut svf = TptSvf::with_params(1000.0, 5.0, crate::SAMPLE_RATE);
    let mut sum: f32 = 0.0;

    for i in 0..44_100usize {
        let input = if i == 0 { 1.0 } else { 0.0 };
        let (lp, _, bp) = svf.process(input);
        sum += lp.abs() + bp.abs();
    }

    // A resonant filter driven by an impulse must produce output.
    assert!(sum > 0.0, "Filter produced no output from impulse");
    assert!(sum.is_finite(), "Filter output is not finite");
}

#[test]
fn test_svf_rapid_param_changes() {
    let mut svf = TptSvf::new(crate::SAMPLE_RATE);

    // Alternate between two extreme frequencies every 100 samples.
    for i in 0..44_100usize {
        let freq = if (i / 100) % 2 == 0 { 200.0 } else { 8000.0 };
        svf.set_params(freq, 0.5, crate::SAMPLE_RATE);
        let input = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44_100.0).sin();
        let _ = svf.process(input);
    }

    assert!(
        svf.ic1eq.is_finite() && svf.ic2eq.is_finite(),
        "Filter state diverged under rapid parameter changes"
    );
}

#[test]
fn test_svf_high_q_does_not_blow_up() {
    let mut svf = TptSvf::with_params(500.0, 50.0, crate::SAMPLE_RATE);

    for _ in 0..44_100 {
        let (lp, hp, bp) = svf.process(0.5);
        assert!(lp.is_finite() && hp.is_finite() && bp.is_finite());
    }
}
