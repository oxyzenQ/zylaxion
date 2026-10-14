// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The tuning lab (night-private-research-2xblade-2). A child module
//! of `metrics` so it can reach the analysis primitives directly; the
//! metric definitions themselves are untouched.

use super::*;

/// The tuning lab (night-private-research-2xblade-2): print what the
/// modal tracker and the tail statistics actually see for one render.
/// Reports the SAME placements, the SAME live gate, and the SAME
/// peak-picking the metrics use — no measured number changes; this
/// only exposes the intermediates so a re-tune can be aimed instead
/// of guessed. fmt of the numbers mirrors the matrix rows.
pub fn debug_modal(mono: &[f32], rate: u32, click_fc: f32, spring_fc: f32) -> String {
    let rate_f = rate as f64;
    let fft = Fft::new(FFT_N);
    let ms = |v: usize| v * rate as usize / 1000;

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
    let late_frames: Vec<Frame> = LATE_FRAME_STARTS_MS
        .iter()
        .map(|&start_ms| {
            let s = ms(start_ms);
            analyze_frame(&fft, MODAL_FRAME, &mono[s..s + MODAL_FRAME])
        })
        .collect();

    let loudest = attack
        .energy
        .max(tail.iter().map(|(_, f)| f.energy).fold(0.0, f64::max))
        .max(refs.iter().map(|f| f.energy).fold(0.0, f64::max))
        .max(late_frames.iter().map(|f| f.energy).fold(0.0, f64::max));
    let live_threshold = loudest * LIVE_FLOOR;

    let mut out = String::new();
    out.push_str(&format!(
        "attack: flatness {:.4} centroid {:.0} Hz | loudest frame energy {:.3e}\n",
        flatness(&attack.power),
        centroid(&attack.power, rate_f),
        loudest
    ));
    out.push_str("tail frames (start ms | dB vs loudest | live | flatness | centroid Hz):\n");
    for (start_ms, f) in &tail {
        let db = 10.0 * (f.energy / loudest).log10();
        out.push_str(&format!(
            "  {:>3} | {:+7.1} | {:>3} | {:.4} | {:.0}\n",
            start_ms,
            db,
            if f.energy >= live_threshold {
                "yes"
            } else {
                "no"
            },
            flatness(&f.power),
            centroid(&f.power, rate_f)
        ));
    }

    // The modal tracker's own view: live refs, mean spectrum, the
    // picked peaks, and each mode's rate (same math as modal_spread).
    let live_refs: Vec<&Frame> = refs.iter().filter(|f| f.energy >= live_threshold).collect();
    if live_refs.is_empty() {
        out.push_str("modal: no live reference frame\n");
        return out;
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
    let mut late_power = vec![0.0_f64; FFT_N];
    let mut late_energy = 0.0_f64;
    for f in &late_frames {
        for (r, &v) in late_power.iter_mut().zip(&f.power) {
            *r += v;
        }
        late_energy += f.energy;
    }
    let n_late = late_frames.len() as f64;
    for p in &mut late_power {
        *p /= n_late;
    }
    late_energy /= n_late;

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
    let bin_hz = rate_f / FFT_N as f64;
    out.push_str(&format!(
        "modal: {} live ref frames, {} local maxima; top candidates (bin | Hz | dB vs frame peak):\n",
        live_refs.len(),
        peaks.len()
    ));
    for (i, &k) in peaks.iter().take(6).enumerate() {
        let db = 10.0 * ref_power[k].log10();
        let p_ref: f64 = ref_power[k - 2..=k + 2].iter().sum::<f64>() * ref_energy;
        let p_late: f64 = late_power[k - 2..=k + 2].iter().sum::<f64>() * late_energy;
        let rate_line = if p_ref <= 0.0 {
            "0.0".to_string()
        } else if p_late <= MODE_DEAD_REL * p_ref {
            format!("{MODE_DEAD_RATE_CAP:.0} (dead cap)")
        } else {
            format!("{:.1}", 10.0 * (p_ref / p_late).log10() / 0.028)
        };
        out.push_str(&format!(
            "  #{i} bin {k} | {:.0} Hz | {db:+.1} dB | rate {rate_line} dB/s{}\n",
            k as f64 * bin_hz,
            if i < 3 { "  [tracked]" } else { "" }
        ));
    }
    let spread = modal_spread(&refs, &late_frames, live_threshold);
    out.push_str(&format!("modal spread: {spread:.1} dB/s (band >= 6.0)\n"));
    // Raw reference-spectrum shape, bins 2..24 (power dB vs frame peak):
    // where the peaks, the skirts, and the pickup floor actually sit.
    out.push_str("ref spectrum bins 2-24 (bin:dB):");
    for (k, &power) in ref_power.iter().enumerate().take(24).skip(2) {
        out.push_str(&format!(" {}:{:+.1}", k, 10.0 * power.log10()));
    }
    out.push('\n');
    // Spectral floor at the preset's own click/spring bins, relative to
    // the reference frame peak: how far each stage's ring sits from the
    // -60 dB pickup floor. This is the number a level re-tune moves.
    for (label, fc) in [("click", click_fc), ("spring", spring_fc)] {
        let k = (fc as f64 / bin_hz).round() as usize;
        if (2..FFT_N / 2 - 2).contains(&k) {
            let db = 10.0 * ref_power[k].log10();
            out.push_str(&format!(
                "ref floor at {label} bin {k} ({:.0} Hz): {db:+.1} dB vs frame peak (pickup floor -60.0)\n",
                k as f64 * bin_hz
            ));
        }
    }
    out
}
