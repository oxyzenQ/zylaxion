// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Minimal radix-2 FFT for the audit harness — stdlib only, f64.
//!
//! The harness is an offline measurement tool, so clarity beats speed:
//! a plain iterative Cooley-Tukey butterfly pass with precomputed
//! twiddle factors. Size must be a power of two; the callers zero-pad
//! shorter windows (the 5 ms attack window) up to the FFT size.

use std::f64::consts::PI;

/// Precomputed twiddle factors for one FFT size.
pub struct Fft {
    n: usize,
    cos: Vec<f64>,
    sin: Vec<f64>,
}

impl Fft {
    /// Build the twiddle tables for `n` (must be a power of two).
    pub fn new(n: usize) -> Self {
        assert!(n.is_power_of_two(), "FFT size must be a power of two");
        let half = n / 2;
        let mut cos = Vec::with_capacity(half);
        let mut sin = Vec::with_capacity(half);
        for k in 0..half {
            let ang = -2.0 * PI * k as f64 / n as f64;
            cos.push(ang.cos());
            sin.push(ang.sin());
        }
        Self { n, cos, sin }
    }

    /// In-place forward FFT. Inputs are the real and imaginary parts;
    /// a real signal is passed with `im` all zeros.
    pub fn forward(&self, re: &mut [f64], im: &mut [f64]) {
        let n = self.n;
        debug_assert_eq!(re.len(), n);
        debug_assert_eq!(im.len(), n);

        // Bit-reversal permutation.
        let mut j = 0_usize;
        for i in 0..n {
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
            let mut m = n >> 1;
            while m >= 1 && j & m != 0 {
                j ^= m;
                m >>= 1;
            }
            j |= m;
        }

        // Butterfly passes.
        let mut len = 2_usize;
        while len <= n {
            let half = len >> 1;
            let step = n / len;
            for start in (0..n).step_by(len) {
                for k in 0..half {
                    let tw = k * step;
                    let wr = self.cos[tw];
                    let wi = self.sin[tw];
                    let i = start + k;
                    let jj = i + half;
                    let tr = re[jj] * wr - im[jj] * wi;
                    let ti = re[jj] * wi + im[jj] * wr;
                    re[jj] = re[i] - tr;
                    im[jj] = im[i] - ti;
                    re[i] += tr;
                    im[i] += ti;
                }
            }
            len <<= 1;
        }
    }
}

/// Hann window of length `n` (periodic convention).
pub fn hann(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / n as f64).cos()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_recovers_sine_frequency_and_amplitude() {
        let n = 1024;
        let fft = Fft::new(n);
        let freq_bins = 40.0_f64; // bin 40
        let amp = 0.5_f64;
        let mut re: Vec<f64> = (0..n)
            .map(|i| amp * (2.0 * PI * freq_bins * i as f64 / n as f64).sin())
            .collect();
        let mut im = vec![0.0_f64; n];
        fft.forward(&mut re, &mut im);
        // Peak must land on bin 40 with magnitude amp*n/2.
        let mut best = (0_usize, 0.0_f64);
        for k in 0..n {
            let mag = re[k] * re[k] + im[k] * im[k];
            if mag > best.1 {
                best = (k, mag);
            }
        }
        assert_eq!(best.0, 40);
        let mag = best.1.sqrt();
        let expect = amp * n as f64 / 2.0;
        assert!(
            (mag - expect).abs() < 1e-6 * expect,
            "magnitude {mag} vs expected {expect}"
        );
    }

    #[test]
    fn hann_ends_near_zero() {
        let w = hann(64);
        assert!(w[0].abs() < 1e-9);
        assert!(w[63].abs() < 1e-2);
    }
}
