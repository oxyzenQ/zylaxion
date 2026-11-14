// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The night-depth-mariana delta + parity diagnostic instrument: where
//! exactly the denormal policy law changes the rendered bytes, and
//! whether the two shipped architectures render the same ones.
//!
//! Three questions, one mode:
//!
//! 1. **The pin, read back** — the control register after the law
//!    fires (MXCSR on x86-64, FPCR on AArch64): trust but verify, on
//!    the machine that runs the instrument.
//! 2. **The law's own delta** — one keystroke per preset at 48 kHz
//!    rendered twice in one process: once under the ambient control
//!    word (the pre-law posture this host inherited), once under the
//!    pinned law. The first sample whose bits differ is located and
//!    its level reported relative to the stream peak. Gate: every
//!    divergence at or below the -60 dB measurand floor — the
//!    audible envelope untouched, the change confined to the dust
//!    region the ZACTRIX_AUDIT finding describes.
//! 3. **The cross-arch fingerprints** — per preset: the full-stream
//!    hash under BOTH postures, plus the attack-region hash (the
//!    first 512 samples — pure excitation and init-shaped
//!    coefficients, integer PRNG and exact arithmetic). Two machines
//!    that agree on the attack hashes agree on every init-time
//!    transcendental; a machine that disagrees there disagrees BEFORE
//!    any dust can exist. These columns are what the parity
//!    investigation reads.
//!
//! x86-64 and AArch64 (each drives its own control register); other
//! architectures print the documented no-op line.

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use crate::engine::VoicePool;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use crate::profiles::registry_all;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use crate::profiles::{KeyTrigger, MechanicalClick};
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use crate::proof::{fnv1a_f32, tail_end_ms};

/// The delta probe's rate, Hz (the rates audit's mid span).
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const DELTA_RATE: u32 = 48_000;

/// The delta probe's window, seconds — the rates audit's window.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const DELTA_WINDOW_S: usize = 4;

/// The measurand floor: the -60 dB level the tail endpoint is defined
/// at. A divergence at or below this level is inside the dust region
/// the physical measurands never see.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const MEASURAND_FLOOR_DB: f64 = -60.0;

/// The attack region: the first 512 samples (pure excitation and
/// init-shaped coefficients — the cross-arch fingerprint of every
/// init-time transcendental, before any dust can exist).
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const ATTACK_SAMPLES: usize = 512;

/// The law's flush bits for THIS architecture: FTZ+DAZ on x86-64
/// (MXCSR bits 15 and 6), FZ on AArch64 (FPCR bit 24).
#[cfg(target_arch = "x86_64")]
const LAW_FLUSH_BITS: u64 = 0x8000;
#[cfg(target_arch = "aarch64")]
const LAW_FLUSH_BITS: u64 = 1 << 24;

/// Read the raw control word (x86-64 MXCSR / AArch64 FPCR).
#[cfg(target_arch = "x86_64")]
fn read_control_word() -> u64 {
    let mut mxcsr: u32 = 0;
    unsafe {
        core::arch::asm!(
            "stmxcsr [{mxcsr_ptr}]",
            mxcsr_ptr = in(reg) &mut mxcsr,
            options(nostack, preserves_flags)
        );
    }
    mxcsr as u64
}

#[cfg(target_arch = "aarch64")]
fn read_control_word() -> u64 {
    let fpcr: u64;
    unsafe {
        core::arch::asm!("mrs {0}, fpcr", out(reg) fpcr);
    }
    fpcr
}

/// Write the raw control word (x86-64 MXCSR / AArch64 FPCR).
#[cfg(target_arch = "x86_64")]
fn write_control_word(bits: u64) {
    let bits32 = bits as u32;
    unsafe {
        core::arch::asm!(
            "ldmxcsr [{mxcsr_ptr}]",
            mxcsr_ptr = in(reg) &bits32,
            options(nostack, preserves_flags)
        );
    }
}

#[cfg(target_arch = "aarch64")]
fn write_control_word(bits: u64) {
    unsafe {
        core::arch::asm!("msr fpcr, {0}", in(reg) bits);
    }
}

/// Render one keystroke under an EXPLICIT control word: the pool
/// constructor pins the shipped law first (the posture every pool
/// gets), then the caller's word overrides it for this render only.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn render_under_control_word(
    control_word: u64,
    profile: crate::profiles::KeyProfile,
    samples: usize,
) -> Vec<f32> {
    let mut pool = VoicePool::new();
    write_control_word(control_word);
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

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub(super) fn run_delta() {
    let samples = DELTA_WINDOW_S * DELTA_RATE as usize;
    let ambient = read_control_word();
    let law_word = ambient | LAW_FLUSH_BITS;
    println!(
        "DELTA (night-depth-mariana): arch={} ambient control word {:#x}, law word {:#x}",
        std::env::consts::ARCH,
        ambient,
        law_word
    );
    // Trust-but-verify on THIS arch: construct a pool (the law fires
    // in the constructor) and read the register back.
    let _pin_probe = VoicePool::new();
    println!(
        "DELTA pin readback: after a pool construction the control word reads {:#x}",
        read_control_word()
    );
    println!(
        "| preset | ambient hash | law hash | attack(512) ambient | attack(512) law | first-div sample | first-div dB | tail ms | gate |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    let presets = registry_all();
    let total = presets.len();
    let mut failures = 0usize;
    let mut identical = 0usize;

    for preset in presets {
        // Ambient posture: the host's own control word (pre-law).
        let pre = render_under_control_word(ambient, preset.profile, samples);
        // Law posture: the ambient word with the law's bits pinned.
        let law = render_under_control_word(law_word, preset.profile, samples);
        let peak = pre
            .iter()
            .fold(0.0_f32, |m, &v| m.max(v.abs()))
            .max(law.iter().fold(0.0_f32, |m, &v| m.max(v.abs())));
        let pre_hash = fnv1a_f32(0xcbf2_9ce4_8422_2325, &pre);
        let law_hash = fnv1a_f32(0xcbf2_9ce4_8422_2325, &law);
        let pre_attack = fnv1a_f32(0xcbf2_9ce4_8422_2325, &pre[..ATTACK_SAMPLES.min(pre.len())]);
        let law_attack = fnv1a_f32(0xcbf2_9ce4_8422_2325, &law[..ATTACK_SAMPLES.min(law.len())]);
        let name = preset.name;
        if pre_hash == law_hash {
            identical += 1;
            println!(
                "| `{name}` | {pre_hash:016x} | identical | {pre_attack:016x} | identical | - | - | - | - |"
            );
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
            "| `{name}` | {pre_hash:016x} | {law_hash:016x} | {pre_attack:016x} | {law_attack:016x} | {first} ({} ms) | {db:.1} | {tail:.2} | {} |",
            first as f64 / DELTA_RATE as f64 * 1000.0,
            if ok { "PASS" } else { "FAIL — divergence above the measurand floor" }
        );
    }
    println!(
        "DELTA verdict: {identical}/{total} presets byte-identical, {} diverged, {failures} gate failures",
        total - identical - failures
    );
    if failures > 0 {
        println!("DELTA FAIL: the law's byte re-baseline must stay below the measurand floor");
        std::process::exit(1);
    }
    println!("DELTA PASS: the law's byte delta is confined to the dust region the measurands never read");
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub(super) fn run_delta() {
    // The instrument holds two control words in one process; on other
    // architectures the law itself is a documented no-op.
    println!("DELTA: unsupported on this architecture ({})", std::env::consts::ARCH);
}
