// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The four physical movement metrics (night-private-research-2xblade,
//! Phase 0) and the conservative physical bands they are judged
//! against.
//!
//! The thesis under measurement: a real mechanical keystroke has an
//! EVOLVING spectrum (broadband click, mid-band spring wake, low
//! housing ring, HF-first tail death), while a static-spectrum engine
//! stretched by an envelope does not move. The metrics quantify
//! movement directly on the rendered mono signal:
//!
//! 1. `centroid_drift_hz`   — how far the spectral centroid travels
//!    between the attack window (0-5 ms) and the tail (50-90 ms).
//!    Real: hundreds of Hz (HF dies first). Static: near zero.
//! 2. `flatness_evolution`  — spectral flatness (Wiener entropy,
//!    0 = tone, 1 = noise) attack minus tail. Real: broadband attack,
//!    tonal tail. Static: near zero.
//! 3. `modal_spread_db_s`   — spread of per-mode decay rates across
//!    the strongest spectral modes. Real: modes die at different
//!    rates. One global envelope: uniform rates, near-zero spread.
//! 4. `attack_tail_db`      — energy ratio of the first 5 ms against
//!    the following 95 ms, in dB. Real impacts concentrate energy in
//!    the attack.
//!
//! MEASUREMENT HONESTY. Three guards keep the numbers physical:
//! - All spectral statistics run over the POSITIVE-frequency half of
//!   the FFT (bins 1..N/2). Over the full mirrored spectrum of a real
//!   signal the centroid collapses to Nyquist/2 for every frame —
//!   a mathematical artifact, not physics.
//! - A frame is LIVE only if its energy sits within -60 dB of the
//!   loudest analysis frame. Below that the samples are numerical
//!   dust, not sound; silent frames are excluded from the tail
//!   statistics and a mode that dies before the late frame is
//!   reported at the dead-rate cap. The -60 dB audibility floor is
//!   the audit's declared measurement floor.
//! - The analysis frames sit where the phenomenon lives. The modal
//!   tracking pairs 8/16 ms reference frames against 32/48 ms late
//!   frames (28 ms center separation): a 30-80 ms placement would
//!   measure the noise floor of an already-dead tail for the fast
//!   presets, an instrument pointed at a corpse. All placements are
//!   FIXED across presets — no per-preset adaptation, no tuning
//!   toward a verdict.
//!
//! PHYSICAL BANDS — honesty note. The preset authoring references
//! (ibm_model_m, topre, alps_skcm) are named in the docs as tuning
//! targets, but the tree ships NO reference recordings or measurement
//! tables for them (the zero-samples posture), so the bands below are
//! CONSERVATIVE estimates chosen from the published acoustics of
//! mechanical switch impact, not from an in-tree reference. They are
//! deliberately wide: a metric lands OUT of band only when the signal
//! clearly lacks the physical behavior it measures. Do not present
//! them as measured from a reference recording — they are not.

use crate::fft::{Fft, hann};

/// FFT size for every analysis frame (bin width 93.75 Hz at 96 kHz).
const FFT_N: usize = 1024;

/// Attack window: the first 5 ms after onset (480 samples at 96 kHz),
/// Hann-windowed and zero-padded to the FFT size.
const ATTACK_SAMPLES: usize = 480;

/// Tail frames (Hann, no padding) for the centroid/flatness tail
/// estimate: 10.67 ms windows from 12 ms to 80 ms, all fully inside
/// the 0-100 ms onset window. Only LIVE frames (see the -60 dB floor)
/// enter the tail statistics — for a fast preset that is the 12-25 ms
/// ring, for a long-ring preset the late windows dominate. The
/// selection is mechanical, not per-preset taste.
const TAIL_FRAME_STARTS_MS: [usize; 10] = [12, 16, 20, 25, 30, 40, 50, 60, 70, 80];

/// Reference frames for modal analysis (modes established, still
/// measurable): windows starting at 8 and 16 ms.
const REF_FRAME_STARTS_MS: [usize; 2] = [8, 16];

/// Late frames for modal analysis: 32 and 48 ms. The frame-center
/// separation against the reference pair is exactly 28 ms.
const LATE_FRAME_STARTS_MS: [usize; 2] = [32, 48];

/// Frame length for the modal-analysis windows (samples).
const MODAL_FRAME: usize = 1024;

/// Relative power floor applied before the logarithm in flatness
/// (-120 dB against each frame's own peak).
const POWER_FLOOR: f64 = 1e-12;

/// Audibility floor: a frame is live only if its energy sits within
/// -60 dB of the loudest analysis frame of the render.
const LIVE_FLOOR: f64 = 1e-6;

/// A mode whose late-frame power falls below this relative level is
/// treated as fully dead and its decay rate is reported at the cap
/// below (-60 dB against the reference peak).
const MODE_DEAD_REL: f64 = 1e-6;

/// Reported decay rate for a fully dead mode, dB/s (capped: the mode
/// demonstrably decayed far faster than any surviving one).
const MODE_DEAD_RATE_CAP: f64 = 600.0;

// ── Conservative physical bands (see the honesty note above) ─────────

/// Centroid must travel at least this far between attack and tail
/// (the tail must be audibly darker than the attack).
pub const BAND_CENTROID_DRIFT_MIN_HZ: f64 = 150.0;

/// Flatness must drop at least this much from attack to tail
/// (broadband attack, tonal tail).
pub const BAND_FLATNESS_EVOLUTION_MIN: f64 = 0.01;

/// Modal decay rates must spread at least this far apart (modes die
/// at measurably different rates; one global envelope does not).
pub const BAND_MODAL_SPREAD_MIN_DB_S: f64 = 6.0;

/// Attack-tail energy ratio band, dB. Real impacts keep 20-94 percent
/// of the onset-window energy in the first 5 ms.
pub const BAND_ATTACK_TAIL_DB: (f64, f64) = (-6.0, 12.0);

/// The four measured metrics for one preset.
#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    /// Spectral centroid travel, attack vs tail, Hz.
    pub centroid_drift_hz: f64,
    /// Flatness attack minus flatness tail (positive = broadband to tonal).
    pub flatness_evolution: f64,
    /// Spread of per-mode decay rates, dB/s.
    pub modal_spread_db_s: f64,
    /// Energy of 0-5 ms against 5-100 ms, dB.
    pub attack_tail_db: f64,
}

/// Per-metric in-band verdicts and the A/B/C tier.
#[derive(Debug, Clone, Copy)]
pub struct Verdict {
    pub centroid_in_band: bool,
    pub flatness_in_band: bool,
    pub modal_in_band: bool,
    pub attack_tail_in_band: bool,
    /// A = 4 in band, B = 2-3, C = 0-1.
    pub tier: char,
}

impl Metrics {
    /// Judge the metrics against the conservative physical bands.
    pub fn verdict(&self) -> Verdict {
        let centroid_in_band = self.centroid_drift_hz >= BAND_CENTROID_DRIFT_MIN_HZ;
        let flatness_in_band = self.flatness_evolution >= BAND_FLATNESS_EVOLUTION_MIN;
        let modal_in_band = self.modal_spread_db_s >= BAND_MODAL_SPREAD_MIN_DB_S;
        let attack_tail_in_band = self.attack_tail_db >= BAND_ATTACK_TAIL_DB.0
            && self.attack_tail_db <= BAND_ATTACK_TAIL_DB.1;
        let count = usize::from(centroid_in_band)
            + usize::from(flatness_in_band)
            + usize::from(modal_in_band)
            + usize::from(attack_tail_in_band);
        let tier = match count {
            4 => 'A',
            2 | 3 => 'B',
            _ => 'C',
        };
        Verdict {
            centroid_in_band,
            flatness_in_band,
            modal_in_band,
            attack_tail_in_band,
            tier,
        }
    }
}

/// One analysis frame: its raw energy (windowed sample energy, the
/// liveness reference) and its normalized power spectrum.
struct Frame {
    energy: f64,
    power: Vec<f64>,
}

/// Analyze one frame: Hann window, forward FFT, magnitude squared,
/// normalized so the peak equals 1.0 (makes the log floor relative).
fn analyze_frame(fft: &Fft, window_len: usize, samples: &[f32]) -> Frame {
    let w = hann(window_len);
    let mut re = vec![0.0_f64; FFT_N];
    let mut im = vec![0.0_f64; FFT_N];
    let mut energy = 0.0_f64;
    for (i, s) in samples.iter().enumerate() {
        let x = *s as f64 * w[i];
        energy += x * x;
        re[i] = x;
    }
    fft.forward(&mut re, &mut im);
    let mut power: Vec<f64> = re.iter().zip(&im).map(|(r, i)| r * r + i * i).collect();
    let peak = power.iter().cloned().fold(0.0_f64, f64::max);
    let scale = if peak > 0.0 { 1.0 / peak } else { 0.0 };
    for p in &mut power {
        *p *= scale;
    }
    Frame { energy, power }
}

/// Spectral centroid of a power spectrum over the positive-frequency
/// half (bins 1..N/2), Hz. DC and the mirrored half are excluded.
fn centroid(power: &[f64], rate: f64) -> f64 {
    let mut num = 0.0;
    let mut den = 0.0;
    for (k, &p) in power.iter().enumerate().take(FFT_N / 2).skip(1) {
        let f = k as f64 * rate / FFT_N as f64;
        num += f * p;
        den += p;
    }
    if den > 0.0 { num / den } else { 0.0 }
}

/// Spectral flatness (Wiener entropy) of a power spectrum over the
/// positive-frequency half, with the relative floor before the log.
fn flatness(power: &[f64]) -> f64 {
    let bins = FFT_N / 2 - 1;
    let mut log_sum = 0.0;
    let mut lin_sum = 0.0;
    for &p in &power[1..FFT_N / 2] {
        let p = p.max(POWER_FLOOR);
        log_sum += p.ln();
        lin_sum += p;
    }
    let geo = (log_sum / bins as f64).exp();
    let ari = lin_sum / bins as f64;
    if ari > 0.0 { geo / ari } else { 0.0 }
}

/// Measure the four metrics on a mono render of one keypress.
///
/// `mono` is at least 100 ms of post-onset audio at `rate`; the audit
/// window is the first 100 ms (the prompt's measurement window), the
/// render itself is 500 ms.
pub fn measure(mono: &[f32], rate: u32) -> Metrics {
    let rate_f = rate as f64;
    let fft = Fft::new(FFT_N);
    let ms = |v: usize| v * rate as usize / 1000;

    // ── Metric 4: attack-tail energy ratio (time domain) ─────────────
    let onset_end = ms(100);
    let attack_energy: f64 = mono[..ATTACK_SAMPLES]
        .iter()
        .map(|x| {
            let x = *x as f64;
            x * x
        })
        .sum();
    let tail_energy: f64 = mono[ATTACK_SAMPLES..onset_end]
        .iter()
        .map(|x| {
            let x = *x as f64;
            x * x
        })
        .sum();
    let attack_tail_db = if tail_energy > 0.0 && attack_energy > 0.0 {
        10.0 * (attack_energy / tail_energy).log10()
    } else if attack_energy == 0.0 {
        // Degenerate: no attack energy at all.
        BAND_ATTACK_TAIL_DB.0
    } else {
        // The tail is numerically dead within the window.
        BAND_ATTACK_TAIL_DB.1
    };

    // ── Analysis frames with the live gate ───────────────────────────
    let attack = analyze_frame(&fft, ATTACK_SAMPLES, &mono[..ATTACK_SAMPLES]);
    let tail: Vec<(usize, Frame)> = TAIL_FRAME_STARTS_MS
        .iter()
        .map(|&start_ms| {
            let s = ms(start_ms);
            (
                start_ms,
                analyze_frame(&fft, MODAL_FRAME, &mono[s..s + MODAL_FRAME]),
            )
        })
        .collect();
    let refs: Vec<Frame> = REF_FRAME_STARTS_MS
        .iter()
        .map(|&start_ms| {
            let s = ms(start_ms);
            analyze_frame(&fft, MODAL_FRAME, &mono[s..s + MODAL_FRAME])
        })
        .collect();
    let lates: Vec<Frame> = LATE_FRAME_STARTS_MS
        .iter()
        .map(|&start_ms| {
            let s = ms(start_ms);
            analyze_frame(&fft, MODAL_FRAME, &mono[s..s + MODAL_FRAME])
        })
        .collect();

    // Liveness reference: the loudest analysis frame of this render.
    let loudest = attack
        .energy
        .max(tail.iter().map(|(_, f)| f.energy).fold(0.0, f64::max))
        .max(refs.iter().map(|f| f.energy).fold(0.0, f64::max))
        .max(lates.iter().map(|f| f.energy).fold(0.0, f64::max));
    let live_threshold = loudest * LIVE_FLOOR;

    // ── Metrics 1 + 2: attack vs live-tail centroid and flatness ─────
    let centroid_attack = centroid(&attack.power, rate_f);
    let flatness_attack = flatness(&attack.power);

    let mut live_tails: Vec<&Frame> = tail
        .iter()
        .map(|(_, f)| f)
        .filter(|f| f.energy >= live_threshold)
        .collect();
    if live_tails.is_empty() {
        // The voice dies before 12 ms: its tail is its last audible
        // frame inside the window — the 8/16 ms reference frames.
        live_tails = refs.iter().filter(|f| f.energy >= live_threshold).collect();
    }

    let (centroid_drift_hz, flatness_evolution) = if live_tails.is_empty() {
        // Nothing audible beyond the attack anywhere in the window:
        // there is no tail to move to. Report zero movement and let
        // the bands judge it.
        (0.0, 0.0)
    } else {
        let n = live_tails.len() as f64;
        let centroid_tail = live_tails
            .iter()
            .map(|f| centroid(&f.power, rate_f))
            .sum::<f64>()
            / n;
        let flatness_tail = live_tails.iter().map(|f| flatness(&f.power)).sum::<f64>() / n;
        (
            (centroid_attack - centroid_tail).abs(),
            flatness_attack - flatness_tail,
        )
    };

    // ── Metric 3: modal decay spread ─────────────────────────────────
    let modal_spread_db_s = modal_spread(&refs, &lates, live_threshold);

    Metrics {
        centroid_drift_hz,
        flatness_evolution,
        modal_spread_db_s,
        attack_tail_db,
    }
}

/// Track the strongest modes from the reference frames to the late
/// frames and report the spread of their decay rates in dB/s.
///
/// LEVEL HONESTY (measurement correction, applied before any Phase 1
/// conclusion was drawn): the first instrument draft compared the two
/// frames' peak-normalized spectra directly — a shape comparison, blind
/// to absolute level. A late frame sitting 74 dB below the attack
/// normalized its own dust to full scale and its bins read as
/// "surviving modes". Both frames are now scaled by their absolute
/// windowed energies before the rate is taken, so a rate is a DECAY
/// rate; the dead-mode cap carries a mode that reaches the -60 dB
/// measurement floor. Both matrices in docs/ZACTRIX_AUDIT.md were
/// (re-)measured with this corrected instrument.
fn modal_spread(refs: &[Frame], lates: &[Frame], live_threshold: f64) -> f64 {
    // Reference spectrum: mean of the LIVE reference frames. If no
    // reference frame is live, nothing rings measurably at 8-26 ms
    // and no spread is measurable.
    let live_refs: Vec<&Frame> = refs.iter().filter(|f| f.energy >= live_threshold).collect();
    if live_refs.is_empty() {
        return 0.0;
    }
    let mut ref_power = vec![0.0_f64; FFT_N];
    let mut ref_energy = 0.0_f64;
    for f in &live_refs {
        for (r, &v) in ref_power.iter_mut().zip(&f.power) {
            *r += v;
        }
        ref_energy += f.energy;
    }
    let n_live = live_refs.len() as f64;
    for p in &mut ref_power {
        *p /= n_live;
    }
    ref_energy /= n_live;

    // Late spectrum: mean over ALL late frames (dead ones contribute
    // zero — a mode vanishing into the floor is real decay, and the
    // dead-mode cap below carries it honestly).
    let mut late_power = vec![0.0_f64; FFT_N];
    let mut late_energy = 0.0_f64;
    for f in lates {
        for (r, &v) in late_power.iter_mut().zip(&f.power) {
            *r += v;
        }
        late_energy += f.energy;
    }
    let n_late = lates.len() as f64;
    for p in &mut late_power {
        *p /= n_late;
    }
    late_energy /= n_late;

    // Peak picking on the reference spectrum: strict local maxima
    // above -60 dB of the frame peak, merged within +/- 2 bins, top 3.
    let mut peaks: Vec<usize> = Vec::new();
    for k in 2..FFT_N / 2 - 2 {
        if ref_power[k] > ref_power[k - 1]
            && ref_power[k] > ref_power[k + 1]
            && ref_power[k] > MODE_DEAD_REL
        {
            if let Some(last) = peaks.last()
                && k - last <= 2
            {
                if ref_power[k] > ref_power[*last] {
                    *peaks.last_mut().unwrap() = k;
                }
                continue;
            }
            peaks.push(k);
        }
    }
    peaks.sort_by(|a, b| ref_power[*b].total_cmp(&ref_power[*a]));
    peaks.truncate(3);
    if peaks.len() < 2 {
        // Fewer than two surviving modes: the signal is single-modal,
        // so no spread is measurable and the metric reports zero
        // (out of band — a real keystroke rings several modes).
        return 0.0;
    }

    // Per-mode ABSOLUTE decay rate across the 28 ms center separation:
    // shape x frame energy on both sides, so the rate is a level-honest
    // decay rate, not a shape-drift rate.
    let dt_s = 0.028;
    let mut rates: Vec<f64> = Vec::with_capacity(peaks.len());
    for &k in &peaks {
        let p_ref: f64 = ref_power[k - 2..=k + 2].iter().sum::<f64>() * ref_energy;
        let p_late: f64 = late_power[k - 2..=k + 2].iter().sum::<f64>() * late_energy;
        let rate = if p_ref <= 0.0 {
            0.0
        } else if p_late <= MODE_DEAD_REL * p_ref {
            MODE_DEAD_RATE_CAP
        } else {
            10.0 * (p_ref / p_late).log10() / dt_s
        };
        rates.push(rate);
    }
    let max = rates.iter().cloned().fold(f64::MIN, f64::max);
    let min = rates.iter().cloned().fold(f64::MAX, f64::min);
    max - min
}
