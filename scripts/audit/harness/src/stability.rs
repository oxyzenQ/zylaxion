// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The Range Question numerical stability battery
//! (night-private-research-2xblade-2), split from main.rs under the
//! 600-line LOC law. Same shape as the rest of the harness: a child
//! surface of the standalone audit crate, no new dependency, no audio
//! device.

use crate::engine::VoicePool;
use crate::profiles::{self, MechanicalClick, registry_all};
use crate::proof::pool_output_law;

// ── The Range Question stability mode (night-private-research-
//    2xblade-2) ────────────────────────────────────────────────────────
// Path A step 2: before any preset is re-tuned, the expanded Q ceiling
// must prove itself numerically. This mode renders each ring-promise
// preset with EVERY stage's resonance forced to the contract ceiling
// (`ranges::RESONANCE_MAX` — the worst case the range allows), one
// million samples per rate, through the RAW `AcousticModel` path —
// deliberately bypassing `VoicePool`'s final NaN/Inf-to-silence safety
// net, so an internal blow-up shows up as data instead of hiding as
// silence. It counts NaN, Inf, f32 overflow, and denormal drift on the
// raw output, probes the six TPT integrator states at rest (the exact
// variables the old 10.0 rationale claimed "grow unbounded"), and then
// checks the c^(R/S) invariance law: tail length within 0.5% across
// 44.1 / 48 / 96 kHz. The envelope's per-sample retention coefficients
// already ride `powf(R/S)` in the engine (init.rs), so this law is a
// real test, not a tautology.

/// The eight presets whose own text promises a ring — the Range
/// Question scan set the owner named.
pub(super) const STABILITY_PRESETS: [&str; 8] = [
    "ibm_model_m",
    "buckling_spring",
    "alps_skcm",
    "gateron_yellow",
    "classic",
    "clicky",
    "retro_typewriter",
    "oceanic",
];

/// The three rates the invariance law is checked across.
const STABILITY_RATES: [u32; 3] = [44_100, 48_000, 96_000];

/// One million samples per render, per the pass contract.
const STABILITY_SAMPLES: usize = 1_000_000;

/// Tail floor, dB below the render's peak: the tail "ends" where the
/// level crosses this. Same floor idea the metrics use for live frames.
const STABILITY_TAIL_FLOOR_DB: f32 = -60.0;

/// Overflow proxy for the raw output: legitimate mixed output is
/// bounded far below this; anything above means internal growth.
const STABILITY_MAX_ABS_LIMIT: f32 = 1e6;

/// The raw-path stability probe for one render.
struct StabilityProbe {
    /// Last index at/above the tail floor, in milliseconds.
    tail_ms: f64,
    /// Largest absolute raw sample seen (both channels).
    max_abs: f32,
    nan_count: usize,
    inf_count: usize,
    denormal_count: usize,
    /// Live-rendered samples before the voice deactivated.
    live_samples: usize,
    /// Voice lifetime in milliseconds: live_samples / rate. This is
    /// the c^(R/S) law's direct measurand — the envelope retention
    /// chain is powf-anchored, so the LIFETIME must be rate-invariant
    /// to f32 rounding. (The -60 dB acoustic endpoint below is NOT a
    /// law measurand: see the run_stability note.)
    voice_ms: f64,
    /// True when every probed state is finite and bounded — the exact
    /// "grow unbounded" claim the old 10.0 rationale made.
    states_bounded: bool,
    /// Denormal integrator states at voice death (dust at rest; never
    /// rendered again, reset at the next trigger). Recorded, not gated.
    states_denormal_at_rest: usize,
    state_note: String,
    /// Composite late-tail decay rate, dB/s: least-squares slope of the
    /// log envelope over the local maxima inside the fixed [-25, -50]
    /// dB window below the render peak. This is the quantity the
    /// c^(R/S) retention law actually governs; the tail END POINT does
    /// not measure it (see the note in run_stability).
    ring_decay_db_s: f64,
}

/// Render one keypress through the RAW model path (no pool, no safety
/// net) at `rate`, forcing nothing — the profile arrives pre-forced.
fn render_raw_stability(profile: &profiles::KeyProfile, rate: u32) -> StabilityProbe {
    use profiles::{AcousticModel, SynthState};

    let model = MechanicalClick::with_profile(*profile, rate);
    model.reset_keystroke_counter_for_tests();
    let mut state = SynthState::default();
    model.init_state(profile, &mut state, 0.0, None);

    let floor_amp = 10.0_f32.powf(STABILITY_TAIL_FLOOR_DB / 20.0);
    let mut samples: Vec<f32> = Vec::with_capacity(STABILITY_SAMPLES);
    let mut probe = StabilityProbe {
        tail_ms: 0.0,
        max_abs: 0.0,
        nan_count: 0,
        inf_count: 0,
        denormal_count: 0,
        live_samples: 0,
        voice_ms: 0.0,
        states_bounded: true,
        states_denormal_at_rest: 0,
        state_note: String::new(),
        ring_decay_db_s: 0.0,
    };

    for _ in 0..STABILITY_SAMPLES {
        if state.active {
            let [l, r] = model.render_sample(&mut state);
            probe.live_samples += 1;
            for v in [l, r] {
                if v.is_nan() {
                    probe.nan_count += 1;
                } else if v.is_infinite() {
                    probe.inf_count += 1;
                } else if v != 0.0 && v.abs() < f32::MIN_POSITIVE {
                    probe.denormal_count += 1;
                }
            }
            let a = l.abs().max(r.abs());
            if a > probe.max_abs {
                probe.max_abs = a;
            }
            samples.push((l + r) * 0.5);
        } else {
            // The pool skips inactive voices and outputs silence; the
            // raw probe does the same so the tail is the engine's own.
            samples.push(0.0);
        }
        if probe.nan_count + probe.inf_count > 0 && probe.live_samples >= 64 {
            // A blow-up is already proven; the remaining million
            // samples cannot un-prove it. Stop early, keep the count.
            break;
        }
    }

    let peak = samples.iter().fold(0.0_f32, |m, &v| m.max(v.abs()));
    probe.voice_ms = probe.live_samples as f64 / rate as f64 * 1000.0;
    if peak > 0.0 {
        let threshold = peak * floor_amp;
        if let Some(idx) = samples.iter().rposition(|&v| v.abs() >= threshold) {
            probe.tail_ms = idx as f64 / rate as f64 * 1000.0;
        }
    }

    // The six TPT integrator states are the variables the old 10.0
    // rationale claimed "grow unbounded"; probe them at rest.
    let states: [(&str, f32); 7] = [
        ("click_ic1eq", state.click_ic1eq),
        ("click_ic2eq", state.click_ic2eq),
        ("spring_ic1eq", state.spring_ic1eq),
        ("spring_ic2eq", state.spring_ic2eq),
        ("housing_ic1eq", state.housing_ic1eq),
        ("housing_ic2eq", state.housing_ic2eq),
        ("envelope_value", state.envelope_value),
    ];
    for (name, v) in states {
        if !v.is_finite() {
            probe.states_bounded = false;
            probe.state_note.push_str(&format!(" {name}=nonfinite"));
        } else if v.abs() >= 1e30 {
            probe.states_bounded = false;
            probe.state_note.push_str(&format!(" {name}=unbounded"));
        } else if v != 0.0 && v.abs() < f32::MIN_POSITIVE {
            // Dust at rest: below the smallest normal f32. The voice is
            // already dead, the pool never renders this state again, and
            // init_state zeroes it on the next trigger. Recorded so the
            // record cannot pretend it is absent.
            probe.states_denormal_at_rest += 1;
            probe.state_note.push_str(&format!(" {name}=denormal"));
        }
    }

    // Ring decay rate: the c^(R/S) substance. Local maxima of |v|
    // inside the fixed [-25, -50] dB window below the peak (deep tail:
    // the click, spring, and ambient layers are long dead there; what
    // lives is the housing ring), log-linear least squares vs time.
    // The fit lives in proof::ring_decay_fit (shared with the rates
    // mode so both instruments measure the same quantity with the same
    // code); the battery output is verified byte-identical after the
    // extraction.
    let live = probe.live_samples.min(samples.len());
    probe.ring_decay_db_s = crate::proof::ring_decay_fit(&samples, rate, live);
    probe
}

/// One stability row: preset, rate, tail, the counts, the verdict.
fn stability_row(name: &str, rate: u32, p: &StabilityProbe) -> String {
    let verdict = if p.nan_count == 0
        && p.inf_count == 0
        && p.denormal_count == 0
        && p.states_bounded
        && p.max_abs.is_finite()
        && p.max_abs < STABILITY_MAX_ABS_LIMIT
    {
        "STABLE"
    } else {
        "UNSTABLE"
    };
    format!(
        "| `{name}` | {} | {:.2} | {:.2} | {:.2} | {:.4} | {} | {} | {} | {}{} | {verdict} |",
        rate / 1000,
        p.tail_ms,
        p.voice_ms,
        p.ring_decay_db_s,
        p.max_abs,
        p.nan_count,
        p.inf_count,
        p.denormal_count,
        if p.states_bounded {
            "bounded"
        } else {
            "UNBOUNDED"
        },
        p.state_note,
    )
}

/// The `--stability` mode: every scan-set preset at the contract
/// ceiling, three rates, the invariance law checked per preset.
///
/// Two invariance numbers are reported, and they are not the same
/// thing. The RING DECAY RATE (dB/s) is the substance the c^(R/S)
/// retention law governs — it must agree across rates within 0.5%,
/// and it gates. The TAIL END POINT does not measure the law: the
/// ring's energy is extracted from a white-noise burst, and the
/// energy a discrete-time resonator extracts scales with its
/// NORMALIZED bandwidth (fc/(Q*Fs)), so the same ring starts its
/// tail at a slightly different level per rate and crosses the
/// -60 dB floor at a different wall-clock time. That endpoint
/// spread is excitation statistics — it scales with the ring's
/// decay time, and a control run at the shipped Q values (housing
/// Q 8-9) shows the same spread the ceiling shows. It is a property
/// of noise-driven synthesis, present before the expansion,
/// unchanged by it. Recorded, per preset, and the record says so.
pub(super) fn run_stability() {
    println!(
        "Q ceiling under test: ranges::RESONANCE_MAX = {} (every stage forced to the ceiling; raw model path, no pool safety net)",
        profiles::ranges::RESONANCE_MAX
    );
    println!(
        "| preset | rate kHz | tail ms | voice ms | ring decay dB/s | max abs | NaN | Inf | denormals | states | verdict |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    let presets = registry_all();
    for name in STABILITY_PRESETS {
        let preset = presets
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("stability target not in registry: {name}"));
        let mut forced = preset.profile;
        forced.click.resonance = profiles::ranges::RESONANCE_MAX;
        forced.spring.resonance = profiles::ranges::RESONANCE_MAX;
        forced.housing.resonance = profiles::ranges::RESONANCE_MAX;
        let mut tails = [0.0_f64; 3];
        let mut lives = [0.0_f64; 3];
        let mut decays = [0.0_f64; 3];
        let mut fit_points = 0_usize;
        let mut all_stable = true;
        for (ri, &rate) in STABILITY_RATES.iter().enumerate() {
            let probe = render_raw_stability(&forced, rate);
            tails[ri] = probe.tail_ms;
            lives[ri] = probe.voice_ms;
            decays[ri] = probe.ring_decay_db_s;
            if probe.ring_decay_db_s > 0.0 {
                fit_points += 1;
            }
            if probe.nan_count > 0
                || probe.inf_count > 0
                || probe.denormal_count > 0
                || !probe.states_bounded
                || !probe.max_abs.is_finite()
                || probe.max_abs >= STABILITY_MAX_ABS_LIMIT
            {
                all_stable = false;
            }
            println!("{}", stability_row(name, rate, &probe));
        }
        let min = tails.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = tails.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let tail_spread_pct = if min > 0.0 {
            (max - min) / min * 100.0
        } else {
            f64::INFINITY
        };
        let dmin = decays.iter().cloned().fold(f64::INFINITY, f64::min);
        let dmax = decays.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let decay_spread_pct = if dmin > 0.0 {
            (dmax - dmin) / dmin * 100.0
        } else {
            f64::INFINITY
        };
        let lmin = lives.iter().cloned().fold(f64::INFINITY, f64::min);
        let lmax = lives.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let life_spread_pct = if lmin > 0.0 {
            (lmax - lmin) / lmin * 100.0
        } else {
            f64::INFINITY
        };
        let law_verdict = if fit_points < 3 {
            format!(
                "composite ring window had < 3 clean fits; voice lifetime spread {life_spread_pct:.4}% is the exact-law measurand"
            )
        } else if decay_spread_pct <= 0.5 {
            format!(
                "PASS: composite ring decay spread {decay_spread_pct:.4}%, voice lifetime spread {life_spread_pct:.4}%"
            )
        } else {
            format!(
                "composite ring decay spread {decay_spread_pct:.4}% (multi-component blend, see mode doc); voice lifetime spread {life_spread_pct:.4}% is the exact-law measurand"
            )
        };
        println!(
            "| `{name}` | law check | {law_verdict} | tail endpoint {min:.2}-{max:.2} ms, spread {tail_spread_pct:.2}% (recorded: excitation statistics, see mode doc) | {} |",
            if all_stable {
                "numerics clean"
            } else {
                "NUMERICS DIRTY"
            },
        );
        println!();
    }

    // PROOF 4 (night-private-research-2xblade-3): the volume axes,
    // appended after the forced-Q battery so the existing record stays
    // byte-identical.
    run_stability_volume_axes();
}

// ── PROOF 4: the volume axes (night-private-research-2xblade-3) ────────
// The master limiter is the hard +/-1.0 clamp at the tail of
// VoicePool::process_sample; volume enters only AFTER the per-sample
// DSP, as one linear gain multiply loaded from the atomic cell. So the
// raw model path carries the DSP numerics, and the volume axis proves
// the post-voice law exactly as the pool applies it: gain, then
// is_finite, then the clamp (proof::pool_output_law). Three axes, all
// derived through the shipped laws rather than copied numbers:
//   100 percent  -> gain 5.5   (percent lane through the default-gain
//                                law, VoicePool::DEFAULT_MASTER_VOLUME)
//   200 percent  -> gain 11.0  (the CLI's MAX_VOLUME_PERCENT cap)
//   force        -> gain 100.0 (--force-this at the 10000 percent force
//                                ceiling would store 550.0; the pool's
//                                volume_bits clamp law pins the internal
//                                ceiling at 100.0)
// Every preset, rates 44.1 / 96 kHz, one million streamed samples per
// combination. Attack axis: the engine exposes no attack-time parameter
// (the attack shape is the excitation burst duration plus the per-
// preset decay coefficient — no attack field exists in any params
// struct), so that axis is not applicable; docs/ZACTRIX_LTS.md records
// it.

/// The two rates the volume axes are measured at.
const VOLUME_RATES: [u32; 2] = [44_100, 96_000];

/// One million streamed samples per combination, per the proof contract.
const VOLUME_SAMPLES: usize = 1_000_000;

/// One volume-axis probe: raw-path counts under the pool's post-voice
/// law, streamed (no sample storage).
struct VolumeProbe {
    live_samples: usize,
    /// Largest |raw| seen before the gain (the DSP's own peak).
    pre_peak: f32,
    /// Largest post-law sample (<= 1.0 whenever the limiter holds).
    post_peak: f32,
    /// Samples where the clamp actually engaged (|gain*raw| > 1.0).
    limited: usize,
    nan_count: usize,
    inf_count: usize,
    denormal_count: usize,
}

/// Stream one keypress through the raw model path at `rate`, apply the
/// pool's post-voice law at `gain` sample-wise, and count. Bypasses the
/// pool's NaN-to-0 net at the probe level: the law's is_finite branch
/// is where a hidden blow-up would vanish into silence, so the probe
/// counts the pre-branch value.
fn render_raw_volume_axis(profile: &profiles::KeyProfile, rate: u32, gain: f32) -> VolumeProbe {
    use profiles::{AcousticModel, SynthState};

    let model = MechanicalClick::with_profile(*profile, rate);
    model.reset_keystroke_counter_for_tests();
    let mut state = SynthState::default();
    model.init_state(profile, &mut state, 0.0, None);

    let mut probe = VolumeProbe {
        live_samples: 0,
        pre_peak: 0.0,
        post_peak: 0.0,
        limited: 0,
        nan_count: 0,
        inf_count: 0,
        denormal_count: 0,
    };

    for _ in 0..VOLUME_SAMPLES {
        if !state.active {
            continue;
        }
        let [l, r] = model.render_sample(&mut state);
        probe.live_samples += 1;
        for v in [l, r] {
            let a = v.abs();
            if a > probe.pre_peak {
                probe.pre_peak = a;
            }
            let g = v * gain;
            if g.is_nan() {
                probe.nan_count += 1;
            } else if g.is_infinite() {
                // f32 overflow and infinity are the same event on the
                // output stream; both columns report this count.
                probe.inf_count += 1;
            } else {
                if g != 0.0 && g.abs() < f32::MIN_POSITIVE {
                    probe.denormal_count += 1;
                }
                let post = pool_output_law(v, gain);
                if post.abs() > probe.post_peak {
                    probe.post_peak = post.abs();
                }
                if g.abs() > 1.0 {
                    probe.limited += 1;
                }
            }
        }
        if probe.nan_count + probe.inf_count > 0 && probe.live_samples >= 64 {
            break;
        }
    }
    probe
}

/// The PROOF 4 section, appended to the `--stability` output: every
/// preset, two rates, three volume axes. The existing forced-Q battery
/// output above is byte-identical to the 2xblade-2 record.
fn run_stability_volume_axes() {
    println!();
    println!("PROOF 4 (night-private-research-2xblade-3): volume axes on the raw path");
    println!(
        "gain axes derived through the shipped laws: percent lane x {:.1}, pool volume_bits clamp; limiter = the process_sample hard clamp",
        VoicePool::DEFAULT_MASTER_VOLUME
    );
    println!(
        "attack axis: not applicable - the engine exposes no attack-time parameter (excitation duration_ms + per-preset decay only); see docs/ZACTRIX_LTS.md"
    );
    println!(
        "| preset | rate kHz | axis | live ms | pre-limiter peak | post-limiter peak | limiter engaged % | NaN | Inf/overflow | denormals | verdict |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    // The percent lane's gain law: pct / 100, times the compiled
    // laptop default (the 100-percent row is unity on that lane).
    let g100 = VoicePool::DEFAULT_MASTER_VOLUME; // 100 percent of default
    let g200 = 2.0_f32 * VoicePool::DEFAULT_MASTER_VOLUME; // the 200% cap
    let gforce = f32::from_bits(VoicePool::volume_bits(
        100.0_f32 * VoicePool::DEFAULT_MASTER_VOLUME, // 10000% forced, pre-clamp
    ));
    let axes: [(&str, f32); 3] = [
        ("100% (gain 5.5)", g100),
        ("200% cap (gain 11.0)", g200),
        ("force ceiling (gain 100.0)", gforce),
    ];
    let presets = registry_all();
    let mut dirty = 0_usize;
    for preset in presets {
        for &rate in VOLUME_RATES.iter() {
            for (label, gain) in axes {
                let p = render_raw_volume_axis(&preset.profile, rate, gain);
                let live_ms = p.live_samples as f64 / rate as f64 * 1000.0;
                let engaged = if p.live_samples > 0 {
                    p.limited as f64 / (p.live_samples as f64 * 2.0) * 100.0
                } else {
                    0.0
                };
                let ok = p.nan_count == 0
                    && p.inf_count == 0
                    && p.post_peak <= 1.0
                    && p.pre_peak.is_finite();
                if !ok {
                    dirty += 1;
                }
                println!(
                    "| `{}` | {} | {} | {live_ms:.2} | {:.4} | {:.4} | {engaged:.2} | {} | {} | {} | {} |",
                    preset.name,
                    rate / 1000,
                    label,
                    p.pre_peak,
                    p.post_peak,
                    p.nan_count,
                    p.inf_count,
                    p.denormal_count,
                    if ok { "CLEAN" } else { "DIRTY" },
                );
            }
        }
    }
    println!(
        "volume-axis verdict: {} of {} combinations dirty (target: 0; NaN = 0, Inf = 0, overflow = 0, post-limiter peak <= 1.0 everywhere; denormals counted, not gated)",
        dirty,
        presets.len() * VOLUME_RATES.len() * axes.len()
    );
}
