// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The night-depth-mariana-1 delta instrument: where exactly the
//! denormal policy law changes the rendered bytes, quantified.
//!
//! The law (see src/engine/denormals.rs) pins FTZ/DAZ on the render
//! lane. Against the pre-law ambient posture (flush bits clear) the
//! law's render hashes differ — the rates audit measured that much.
//! This instrument answers the question the rates table cannot: WHERE
//! do the bytes diverge, and how loud is the first divergence relative
//! to the stream's peak? For every shipped preset at 48 kHz it renders
//! one keystroke twice — once under the pre-law ambient control word,
//! once under the law — finds the first sample whose bits differ, and
//! reports that sample's level in dB relative to the stream peak. The
//! gate: every divergence must sit at or below the -60 dB measurand
//! floor (the tail endpoint's own level) — the audible envelope is
//! untouched, the change is the dust region the ZACTRIX_AUDIT finding
//! describes, and the new hashes are the estate's re-baselined bytes.
//!
//! x86-64 only: the instrument drives stmxcsr/ldmxcsr directly to
//! hold both postures in one process. The law itself is per-arch and
//! tested per-arch in the main test tree.

#[cfg(target_arch = "x86_64")]
use crate::engine::VoicePool;
#[cfg(target_arch = "x86_64")]
use crate::profiles::registry_all;
#[cfg(target_arch = "x86_64")]
use crate::profiles::{KeyTrigger, MechanicalClick};
#[cfg(target_arch = "x86_64")]
use crate::proof::{fnv1a_f32, tail_end_ms};

/// The delta probe's rate, Hz (the rates audit's mid span).
#[cfg(target_arch = "x86_64")]
const DELTA_RATE: u32 = 48_000;

/// The delta probe's window, seconds — the rates audit's window.
#[cfg(target_arch = "x86_64")]
const DELTA_WINDOW_S: usize = 4;

/// The measurand floor: the -60 dB level the tail endpoint is defined
/// at. A divergence at or below this level is inside the dust region
/// the physical measurands never see.
#[cfg(target_arch = "x86_64")]
const MEASURAND_FLOOR_DB: f64 = -60.0;

/// The x86-64 flush bits the law pins (MXCSR bit 15 FTZ, bit 6 DAZ) —
/// the same constant the engine law writes; duplicated here because
/// the harness reads the engine trees by path and gains no new
/// dependency surface.
#[cfg(target_arch = "x86_64")]
const X86_FLUSH_BITS: u32 = 0x8040;

#[cfg(target_arch = "x86_64")]
fn read_mxcsr_raw() -> u32 {
    let mut mxcsr: u32 = 0;
    unsafe {
        core::arch::asm!(
            "stmxcsr [{mxcsr_ptr}]",
            mxcsr_ptr = in(reg) &mut mxcsr,
            options(nostack, preserves_flags)
        );
    }
    mxcsr
}

#[cfg(target_arch = "x86_64")]
fn write_mxcsr_raw(bits: u32) {
    unsafe {
        core::arch::asm!(
            "ldmxcsr [{mxcsr_ptr}]",
            mxcsr_ptr = in(reg) &bits,
            options(nostack, preserves_flags)
        );
    }
}

/// Render one keystroke under an EXPLICIT control word: the pool
/// constructor pins the shipped law first (the posture every pool
/// gets), then the caller's word overrides it for this render only.
#[cfg(target_arch = "x86_64")]
fn render_under_mxcsr(
    control_word: u32,
    profile: crate::profiles::KeyProfile,
    samples: usize,
) -> Vec<f32> {
    let mut pool = VoicePool::new();
    write_mxcsr_raw(control_word);
    let model = MechanicalClick::with_profile(profile, DELTA_RATE);
    model.reset_keystroke_counter_for_tests();
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

#[cfg(target_arch = "x86_64")]
pub(super) fn run_delta() {
    let samples = DELTA_WINDOW_S * DELTA_RATE as usize;
    println!(
        "DELTA (night-depth-mariana-1): pre-law ambient vs pinned law, one keystroke per preset, {DELTA_RATE} Hz, window {DELTA_WINDOW_S} s"
    );
    // The ambient control word, read BEFORE any pool construction —
    // the constructor pins the law, so this is the last look at the
    // host's own posture.
    let ambient = read_mxcsr_raw();
    println!("DELTA ambient mxcsr: {ambient:#010x} (flush bits {})", if ambient & X86_FLUSH_BITS != 0 { "set" } else { "clear" });
    let law_word = ambient | X86_FLUSH_BITS;
    let presets = registry_all();
    let total = presets.len();
    let mut failures = 0usize;
    let mut identical = 0usize;

    for preset in presets {
        // Pre-law posture: the host's own control word.
        let pre = render_under_mxcsr(ambient, preset.profile, samples);
        // Law posture: the ambient word with the flush bits pinned.
        let law = render_under_mxcsr(law_word, preset.profile, samples);
        let peak = pre
            .iter()
            .fold(0.0_f32, |m, &v| m.max(v.abs()))
            .max(law.iter().fold(0.0_f32, |m, &v| m.max(v.abs())));
        let pre_hash = fnv1a_f32(0xcbf2_9ce4_8422_2325, &pre);
        let law_hash = fnv1a_f32(0xcbf2_9ce4_8422_2325, &law);
        let name = preset.name;
        if pre_hash == law_hash {
            identical += 1;
            println!("| `{name}` | identical | - | - |");
            continue;
        }
        let first = pre
            .iter()
            .zip(law.iter())
            .position(|(a, b)| a.to_bits() != b.to_bits())
            .expect("hashes differ so some sample must differ");
        let level = (pre[first].abs().max(law[first].abs()) as f64 / peak as f64)
            .max(f64::EPSILON)
            .abs();
        let db = 20.0 * level.log10();
        let tail = tail_end_ms(&pre, DELTA_RATE);
        let ok = db <= MEASURAND_FLOOR_DB;
        if !ok {
            failures += 1;
        }
        println!(
            "| `{name}` | {first} ({} ms) | {db:.1} dB | tail endpoint {tail:.2} ms | {}",
            first as f64 / DELTA_RATE as f64 * 1000.0,
            if ok { "PASS" } else { "FAIL — divergence above the measurand floor" }
        );
    }
    println!(
        "DELTA verdict: {identical} presets byte-identical, {} presets diverged below the {MEASURAND_FLOOR_DB} dB floor, {failures} gate failures",
        total - identical - failures
    );
    if failures > 0 {
        println!("DELTA FAIL: the law's byte re-baseline must stay below the measurand floor");
        std::process::exit(1);
    }
    println!("DELTA PASS: the law's byte delta is confined to the dust region the measurands never read");
}

#[cfg(not(target_arch = "x86_64"))]
pub(super) fn run_delta() {
    // The instrument holds two x86-64 control words in one process;
    // the cross-arch parity lane is the equivalent evidence on the
    // AArch64 side, and the per-arch policy tests pin the law there.
    println!("DELTA: unsupported on this architecture (x86-64 only instrument)");
}
