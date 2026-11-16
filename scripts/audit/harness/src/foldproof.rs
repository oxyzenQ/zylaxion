// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-frontier-Z3 C1: the saturation-boundary instrument — the
//! measurand the freeze bar names for the mode-state fold ("a
//! measurable improvement proven by the audit harness").
//!
//! The pool's saturation lane is where voice stealing used to live:
//! every slot sounding, one more keypress, and the oldest voice was
//! reinitialized — its tail cut mid-ring. The physical stake is the
//! energy that tail was still carrying at the cut instant; the
//! measurable artifact is how much of the mixed stream after the
//! boundary still carries it.
//!
//! This mode renders a fixed deterministic saturation sequence — 20
//! presses, 10 ms apart, of one preset through the REAL VoicePool —
//! and reports four rows:
//!
//! 1. the pre-boundary stream hash (presses 1..=MAX_POLYPHONY): the
//!    natural regime, byte-identical under any policy that only acts
//!    on saturation — the identity row between the committed runs;
//! 2. the natural onset band (the ordinary attack's median step);
//! 3. the boundary's carried energy: the OLDEST voice's own remaining
//!    output over the post-boundary window, rendered alone at unity
//!    gain (pre-limiter voice units — the stake the cut disposes of;
//!    policy-independent, identical in both runs);
//! 4. the post-boundary mix energy: the real pool's mixed output over
//!    the same window at the shipped gain (post-limiter listener
//!    units — THE policy-dependent row: the run whose policy cuts the
//!    tail loses the carried energy here; the run whose policy folds
//!    it retains it).
//!
//! The mode renders and measures; it never inspects which policy the
//! pool runs. The evidence is the delta between two committed runs
//! (the steal shape and the fold shape), interpreted in the ledger.

use crate::engine::VoicePool;
use crate::profiles::{KeyTrigger, MechanicalClick};

/// FNV-1a 64-bit over the raw f32 bit patterns of the mono stream —
/// the same digest discipline the golden corpus and the rates lane
/// ride (deterministic across runs, no HashMap ordering).
fn fnv1a_f32(hash: u64, samples: &[f32]) -> u64 {
    let mut h = hash;
    for &s in samples {
        for byte in s.to_bits().to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h
}

/// The measured sequence's shape: MAX_POLYPHONY natural presses fill
/// the pool, four more cross the saturation boundary, every press
/// sits one spacing window from its neighbours.
const PRESSES: usize = 20;
/// The pool's capacity — the natural regime is exactly this many
/// presses; the first press beyond it is the boundary.
const NATURAL: usize = 16;
/// Press spacing in milliseconds: the machine-gun burst regime the
/// artifact lives in (fast enough that every voice still rings when
/// the pool fills).
const SPACING_MS: f64 = 10.0;
/// The post-boundary observation window, in milliseconds: long
/// enough to carry the tail's residual energy, short enough to stay
/// inside the sequence's own render.
const WINDOW_MS: f64 = 200.0;

fn run_preset(name: &str, profile: crate::profiles::KeyProfile, rate: u32, spacing_ms: f64) {
    let spacing = (rate as f64 * spacing_ms / 1000.0).round() as usize;
    let window = (rate as f64 * WINDOW_MS / 1000.0).round() as usize;
    let total = (PRESSES - 1) * spacing + window;

    // ── The saturation render (the policy under test) ──────────────
    let model = MechanicalClick::with_profile(profile, rate);
    model.reset_keystroke_counter_for_tests();
    let mut pool = VoicePool::new();

    let event = KeyTrigger {
        scancode: 30,
        pressed: true,
        stereo_position: 0.0,
        velocity: None,
    };

    let mut mono: Vec<f32> = Vec::with_capacity(total);
    let mut idxs: Vec<usize> = Vec::new();
    for press in 0..PRESSES {
        pool.trigger(&model, &event);
        // The trigger instant's first audible sample is the next one
        // rendered; the boundary sits at exactly that edge.
        idxs.push(mono.len());
        // Render up to the next press (the last press renders the
        // observation window).
        let to = if press + 1 == PRESSES {
            total
        } else {
            idxs[press] + spacing
        };
        while mono.len() < to {
            let [l, r] = pool.process_sample(&model);
            mono.push((l + r) * 0.5);
        }
    }

    // The pre-boundary stream: everything before the FIRST saturation
    // trigger (press NATURAL+1's index). The natural regime.
    let boundary = idxs[NATURAL];
    let pre_hash = fnv1a_f32(0xcbf2_9ce4_8422_2325, &mono[..boundary]);

    // The natural onset band: |y[i] - y[i-1]| at every non-boundary
    // trigger instant (presses 2..=NATURAL — every onset whose
    // previous sample already carries sound).
    let mut natural: Vec<f64> = idxs[1..NATURAL]
        .iter()
        .map(|&i| (mono[i] - mono[i - 1]).abs() as f64)
        .collect();
    natural.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let natural_median = if natural.is_empty() {
        0.0
    } else {
        natural[natural.len() / 2]
    };

    // ── The carried stake (the boundary voice's own tail) ──────────
    // The oldest voice at the boundary is press 1, alone: one press,
    // unity gain (voice units — a single voice never reaches the
    // limiter's ceiling), the same clock. Its remaining energy over
    // the post-boundary window is exactly what a mid-ring cut
    // destroys.
    let mut lone = VoicePool::with_volume(1.0);
    let lone_model = MechanicalClick::with_profile(profile, rate);
    lone_model.reset_keystroke_counter_for_tests();
    let mut lone_mono: Vec<f32> = Vec::with_capacity(total);
    lone.trigger(&lone_model, &event);
    while lone_mono.len() < total {
        let [l, r] = lone.process_sample(&lone_model);
        lone_mono.push((l + r) * 0.5);
    }
    let carried: f64 = lone_mono[boundary..]
        .iter()
        .map(|&s| (s as f64) * (s as f64))
        .sum();
    // The same voice's ONSET energy (its first spacing window — the
    // attack every listener hears): the dB relation carried/onset is
    // the audibility verdict the honest risk asked for. A cut whose
    // carried energy sits at or below the -60 dB measurand floor
    // destroys nothing a listener could have heard.
    let onset: f64 = lone_mono[0..spacing]
        .iter()
        .map(|&s| (s as f64) * (s as f64))
        .sum();
    let carried_db = if onset > 0.0 {
        10.0 * (carried / onset).log10()
    } else {
        0.0
    };

    // ── The policy row: the mix's post-boundary energy ─────────────
    let mixed: f64 = mono[boundary..]
        .iter()
        .map(|&s| (s as f64) * (s as f64))
        .sum();

    println!(
        "FOLD-PROOF preset={name} rate={rate} presses={PRESSES} spacing_ms={spacing_ms} pool={NATURAL}"
    );
    println!(
        "FOLD-PROOF pre-boundary hash={pre_hash:016x} samples={boundary} (presses 1..={NATURAL}, the natural regime — identical under any policy that only acts on saturation)"
    );
    println!(
        "FOLD-PROOF natural median step={natural_median:.6} (presses 2..={NATURAL}, the ordinary onset band)"
    );
    println!(
        "FOLD-PROOF carried energy={carried:.6} rel_onset={carried_db:+.1} dB (voice units, unity gain — the boundary voice's own remaining tail; the dB relation names the audibility of cutting it)"
    );
    println!(
        "FOLD-PROOF post-boundary mix energy={mixed:.6} samples={} (listener units, shipped gain — the policy-dependent row: the cut loses the carried energy here, the fold retains it)",
        mono.len() - boundary
    );
}

/// The mode body: the default preset (the shipped voice every doc
/// quotes), the long-decay witness, and the FORCED-CEILING shape the
/// stability battery rides (every stage's resonance at the legal
/// maximum — the slowest ring the product's own parameter space can
/// produce, the worst case the cut could ever meet). All at the
/// canonical 44100 Hz.
pub fn run_fold_proof() {
    let registry = crate::profiles::registry_all();
    for name in [crate::profiles::DEFAULT_PRESET, "glass_tactile"] {
        let Some(preset) = registry.iter().find(|p| p.name == name) else {
            println!("FOLD-PROOF: FAIL — the registry carries no '{name}' preset");
            std::process::exit(1);
        };
        run_preset(preset.name, preset.profile, crate::profiles::SAMPLE_RATE as u32, SPACING_MS);
    }
    // The forced-ceiling shape: the stability battery's own posture,
    // copied law-for-law (every stage's resonance at the legal
    // ceiling) — the slowest ring the parameter space allows.
    let mut forced = registry
        .iter()
        .find(|p| p.name == crate::profiles::DEFAULT_PRESET)
        .expect("the default preset exists")
        .profile;
    forced.click.resonance = crate::profiles::ranges::RESONANCE_MAX;
    forced.spring.resonance = crate::profiles::ranges::RESONANCE_MAX;
    forced.housing.resonance = crate::profiles::ranges::RESONANCE_MAX;
    run_preset("forced-ceiling", forced, crate::profiles::SAMPLE_RATE as u32, SPACING_MS);
    // The extreme-regime leg: the ceiling shape at 2 ms spacing (a
    // 500 Hz repeat no keyboard, autorepeat or macro produces) — the
    // OLDEST voice the boundary could ever meet, the fold's whole
    // potential market. The rows name what is even there.
    run_preset("forced-ceiling", forced, crate::profiles::SAMPLE_RATE as u32, 2.0);
}
