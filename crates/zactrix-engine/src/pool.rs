// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Pre-allocated polyphonic voice pool with voice stealing.
//!
//! The [`VoicePool`] is the core orchestrator of the Zactrix engine. It manages
//! a fixed-capacity array of voices, handles note-on/note-off events, implements
//! oldest-first voice stealing, and mixes all active voices into a stereo output
//! buffer.
//!
//! # Memory Layout
//!
//! All voices are stored in a fixed-size array. No dynamic allocation occurs in
//! the render path. The pool is constructed once and reused for the entire
//! lifetime of the audio engine.
//!
//! # Voice Stealing Strategy
//!
//! When all voices are active and a new key is pressed, the voice with the
//! smallest `trigger_timestamp` (i.e., the oldest) is stolen and reinitialized
//! for the new key. This is a simple but effective strategy that prioritizes
//! the most recently pressed keys.

use zactrix_profiles::{AcousticModel, KeyEvent, MAX_POLYPHONY};

use crate::Voice;

/// Pre-allocated polyphonic voice pool.
///
/// # Example
///
/// ```rust,ignore
/// use zactrix_engine::VoicePool;
/// use zactrix_profiles::{MechanicalClick, KeyEvent};
///
/// let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
/// let mut pool = VoicePool::new();
///
/// pool.trigger(&model, &KeyEvent {
///     scancode: 30, pressed: true, stereo_position: -0.3,
/// });
///
/// // Render one second of audio
/// for _ in 0..44100 {
///     let [l, r] = pool.process_sample(&model);
///     // send l, r to audio output ...
/// }
/// ```
pub struct VoicePool {
    /// Fixed-capacity voice array. Pre-allocated, never reallocated.
    voices: [Voice; MAX_POLYPHONY],
    /// Monotonic counter incremented on each trigger for voice-stealing order.
    trigger_counter: u64,
    /// Master volume multiplier applied to the final stereo output.
    /// Combined with a hard clamp to prevent clipping when multiple
    /// voices sum above unity.
    master_volume: f32,
}

impl VoicePool {
    /// Default master volume — tuned for laptop / PC speakers whose
    /// higher impedance (vs. headphones) reproduces the synth at a
    /// lower per-watt SPL, allowing the physical key click to dominate
    /// at lower gains. 5.5× makes the synthesised "TEK" audibly
    /// overpower the mechanical click on typical laptop speakers. The
    /// final `.clamp(-1.0, 1.0)` in `process_sample` is the hard
    /// ceiling that turns any overflow into clean compression instead
    /// of digital clipping.
    ///
    /// For headphones (especially IEMs at 16–32 Ω), 5.5× with hard
    /// clamp produces severely compressed and loud audio. Headphone
    /// users should override this to ~1.5× via the `[master]` table in
    /// `config.toml` (v10.2.0+ — dragonzen audit P1).
    pub const DEFAULT_MASTER_VOLUME: f32 = 5.5;

    /// Create a new voice pool with all voices initialized to inactive
    /// and the default master volume (5.5× — laptop-speaker tuned).
    pub fn new() -> Self {
        Self::with_volume(Self::DEFAULT_MASTER_VOLUME)
    }

    /// Create a new voice pool with a custom master volume (v10.2.0+ —
    /// dragonzen audit P1).
    ///
    /// `master_volume` is a linear gain multiplier applied to the final
    /// stereo output. The hard clamp in `process_sample` prevents
    /// digital clipping, so values > 1.0 produce loudness compression
    /// rather than distortion — but extreme values (> 10.0) make the
    /// compression artifacts audible.
    ///
    /// # Recommended values
    ///
    /// - **5.5** (default): laptop / PC speakers. The physical key
    ///   click dominates at this gain.
    /// - **1.5**: headphones (especially IEMs at 16–32 Ω).
    /// - **3.0**: external monitor speakers or quiet laptop speakers.
    /// - **0.5**: subtle background effect.
    pub fn with_volume(master_volume: f32) -> Self {
        Self {
            voices: core::array::from_fn(|_| Voice::new()),
            trigger_counter: 0,
            // Clamp to a sane range to prevent NaN/Infinity from
            // reaching the render path. Negative gains are allowed
            // (phase inversion) but unusual.
            master_volume: if master_volume.is_finite() {
                master_volume.clamp(-100.0, 100.0)
            } else {
                Self::DEFAULT_MASTER_VOLUME
            },
        }
    }

    /// Returns the current master volume (linear gain multiplier).
    #[inline]
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Update the master volume at runtime (v10.2.0+ — dragonzen audit
    /// P1). Useful for hot-reload when the user edits `[master]` in
    /// `config.toml`. The value is clamped to a sane range — see
    /// [`Self::with_volume`].
    #[inline]
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = if volume.is_finite() {
            volume.clamp(-100.0, 100.0)
        } else {
            Self::DEFAULT_MASTER_VOLUME
        };
    }

    /// Trigger a new voice for a key press event.
    ///
    /// If an inactive voice slot is available, it is used. Otherwise, the
    /// oldest active voice (smallest `trigger_timestamp`) is stolen.
    pub fn trigger<M: AcousticModel>(&mut self, model: &M, event: &KeyEvent) {
        let profile = model.get_profile(event);

        // Find the index of the slot to use: prefer inactive, else steal oldest.
        let idx = self
            .voices
            .iter()
            .position(|v| !v.is_active())
            .unwrap_or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.trigger_timestamp)
                    .map(|(i, _)| i)
                    .expect("MAX_POLYPHONY > 0")
            });

        let ts = self.trigger_counter;
        self.trigger_counter = self.trigger_counter.wrapping_add(1);

        let voice = &mut self.voices[idx];
        voice.profile = profile;
        voice.scancode = event.scancode;
        voice.trigger_timestamp = ts;
        model.init_state(&voice.profile, &mut voice.state, event.stereo_position);
    }

    /// Release all active voices matching the given scancode.
    ///
    /// In normal use, exactly one voice matches. If multiple voices somehow
    /// share a scancode (should not happen), all are released.
    ///
    /// # Soft release ramp (v10.2.0+ — dragonzen audit N2)
    ///
    /// Previously this method hard-cut `state.active = false`, producing a
    /// 1-sample discontinuity at the audio output — audible as a tiny
    /// "click off" tell, especially on short taps. Now we set
    /// `state.releasing = true` instead, and `render_sample` ramps the
    /// envelope down to zero over ~2 ms via `state.release_coeff` before
    /// deactivating the voice naturally through the normal
    /// `voice_off_threshold` check.
    pub fn release(&mut self, scancode: u32) {
        for voice in &mut self.voices {
            if voice.is_active() && voice.scancode == scancode {
                voice.state.releasing = true;
            }
        }
    }

    /// Process a single sample for all active voices and return the mixed stereo output.
    ///
    /// This is the primary zero-allocation render path for real-time audio callbacks.
    /// Each call advances all active voices by exactly one sample.
    ///
    /// # Crash-proofing (v1.0.0)
    ///
    /// After applying master volume, the output is checked for `NaN` and
    /// `Infinity`. If either channel is non-finite (which would crash
    /// PipeWire or produce ear-splitting clicks), it is replaced with
    /// `0.0` (silence). This is the **final safety net** — if the DSP
    /// somehow blows up despite the TPT SVF's stability guarantees and
    /// the `validate_and_clamp()` parameter guardrails, the worst the
    /// user hears is a brief moment of silence, not a crash.
    #[inline]
    pub fn process_sample<M: AcousticModel>(&mut self, model: &M) -> [f32; 2] {
        let mut out = [0.0f32; 2];
        for voice in &mut self.voices {
            if voice.is_active() {
                let [left, right] = model.render_sample(&mut voice.state);
                out[0] += left;
                out[1] += right;
            }
        }
        // Apply master volume.
        let l = out[0] * self.master_volume;
        let r = out[1] * self.master_volume;

        // Hard-clamp to [-1.0, 1.0], replacing NaN/Infinity with 0.0.
        // NaN.clamp() returns NaN (NaN comparisons are always false), so
        // we must check is_finite() FIRST.
        [
            if l.is_finite() {
                l.clamp(-1.0, 1.0)
            } else {
                0.0
            },
            if r.is_finite() {
                r.clamp(-1.0, 1.0)
            } else {
                0.0
            },
        ]
    }

    /// Process a batch of samples, accumulating into the output buffer.
    ///
    /// The output buffer is a slice of interleaved stereo frames `[[f32; 2]]`.
    /// The buffer is **not** cleared before mixing — the caller is responsible
    /// for clearing it if needed.
    pub fn process<M: AcousticModel>(&mut self, model: &M, output: &mut [[f32; 2]]) {
        for frame in output.iter_mut() {
            let [l, r] = self.process_sample(model);
            frame[0] += l;
            frame[1] += r;
        }
    }

    /// Returns `true` if at least one voice is currently active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.voices.iter().any(|v| v.is_active())
    }

    /// Return the number of currently active voices.
    #[inline]
    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
    }

    /// Return the total polyphony capacity.
    #[inline]
    pub const fn polyphony(&self) -> usize {
        MAX_POLYPHONY
    }

    /// Reset all voices to inactive state and zero the trigger counter.
    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.reset();
        }
        self.trigger_counter = 0;
    }
}

impl Default for VoicePool {
    fn default() -> Self {
        Self::new()
    }
}

// ── Minimal WAV writer for tests (no external crate) ───────────────────

/// Write an interleaved stereo f32 buffer as a 32-bit float WAV file.
///
/// Only intended for test/output verification — not part of the public API.
#[cfg(test)]
fn write_wav_f32(path: &str, samples: &[f32], sample_rate: u32) {
    use std::io::Write;

    let num_channels: u16 = 2;
    let bits_per_sample: u16 = 32;
    let bytes_per_sample: u16 = bits_per_sample / 8;
    let data_size = (samples.len() * bytes_per_sample as usize) as u32;
    let file_size = 36 + data_size;

    let mut f = std::fs::File::create(path).expect("Failed to create WAV file");

    // RIFF header
    f.write_all(b"RIFF").unwrap();
    f.write_all(&file_size.to_le_bytes()).unwrap();
    f.write_all(b"WAVE").unwrap();

    // fmt chunk (16 bytes for PCM/IEEE float)
    f.write_all(b"fmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&3u16.to_le_bytes()).unwrap(); // IEEE float
    f.write_all(&num_channels.to_le_bytes()).unwrap();
    f.write_all(&sample_rate.to_le_bytes()).unwrap();
    let byte_rate = sample_rate * num_channels as u32 * bytes_per_sample as u32;
    f.write_all(&byte_rate.to_le_bytes()).unwrap();
    let block_align = num_channels * bytes_per_sample;
    f.write_all(&block_align.to_le_bytes()).unwrap();
    f.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    // data chunk
    f.write_all(b"data").unwrap();
    f.write_all(&data_size.to_le_bytes()).unwrap();
    for &sample in samples {
        f.write_all(&sample.to_le_bytes()).unwrap();
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use zactrix_profiles::{MechanicalClick, SAMPLE_RATE};

    #[test]
    fn test_pool_instantiation() {
        let pool = VoicePool::new();
        assert_eq!(pool.polyphony(), MAX_POLYPHONY);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_single_trigger_activates_one_voice() {
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: 0.0,
            },
        );
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn test_release_deactivates_voice() {
        // v10.2.0 (dragonzen audit N2): release no longer hard-cuts
        // active=false. Instead it sets `releasing=true` and the voice
        // ramps its envelope down to zero over ~2 ms via the
        // `release_coeff` coefficient before naturally deactivating
        // through the `voice_off_threshold` check. The test must now
        // render samples until the voice actually goes silent.
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: 0.0,
            },
        );
        assert_eq!(pool.active_count(), 1);

        pool.release(30);
        // Voice is still active immediately after release — it's now in
        // the fast release-ramp phase.
        assert_eq!(
            pool.active_count(),
            1,
            "voice should still be active during release ramp"
        );

        // Render until the release ramp completes. 10 ms of audio at
        // 44.1 kHz = 441 samples, which is ~5x the 2 ms release window —
        // generous headroom.
        let mut samples_rendered = 0;
        for _ in 0..500 {
            let _ = pool.process_sample(&model);
            samples_rendered += 1;
            if pool.active_count() == 0 {
                break;
            }
        }
        assert_eq!(
            pool.active_count(),
            0,
            "voice should deactivate after release ramp (rendered {samples_rendered} samples)"
        );
    }

    #[test]
    fn test_release_nonexistent_scancode_is_noop() {
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: 0.0,
            },
        );
        pool.release(999); // non-existent
        assert_eq!(
            pool.active_count(),
            1,
            "Releasing a non-existent scancode should be a no-op"
        );
    }

    #[test]
    fn test_release_ramp_is_smooth_no_click_artifact() {
        // v10.2.0 (dragonzen audit N2): the release ramp must NOT
        // produce a discontinuity at the moment of release. The
        // pre-release sample and the first post-release sample should
        // differ by a small amount (envelope multiplied by
        // release_coeff instead of decay_coeff), not by a large jump
        // to zero. We assert the max sample-to-sample delta around the
        // release boundary stays well below the "click" threshold.
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: 0.0,
            },
        );

        // Render enough samples for the click transient to peak and
        // start decaying (~5 ms = 220 samples at 44.1 kHz).
        for _ in 0..220 {
            let _ = pool.process_sample(&model);
        }

        // Capture the last pre-release sample.
        let pre = pool.process_sample(&model);
        let pre_amp = pre[0].abs().max(pre[1].abs());

        // Release and capture the first post-release sample.
        pool.release(30);
        let post = pool.process_sample(&model);
        let post_amp = post[0].abs().max(post[1].abs());

        // The post-release amplitude should be CLOSE to the pre-release
        // amplitude — the only difference is the decay coefficient
        // (slow) vs the release coefficient (fast). For typical
        // configs (decay=0.9994, release≈0.95 at 44.1 kHz), the ratio
        // post/pre is roughly 0.95/0.9994 ≈ 0.95 — a 5% drop, not a
        // hard cut to zero.
        assert!(
            post_amp > pre_amp * 0.5,
            "release ramp should be smooth: pre={pre_amp:.6}, post={post_amp:.6} (post should be > 50% of pre, not a hard cut to zero)"
        );
        assert!(
            post_amp < pre_amp * 1.05,
            "release should not INCREASE amplitude: pre={pre_amp:.6}, post={post_amp:.6}"
        );
    }

    #[test]
    fn test_voice_stealing_replaces_oldest() {
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        // Fill all 16 voice slots
        for i in 0..MAX_POLYPHONY as u32 {
            pool.trigger(
                &model,
                &KeyEvent {
                    scancode: 10 + i,
                    pressed: true,
                    stereo_position: 0.0,
                },
            );
        }
        assert_eq!(pool.active_count(), MAX_POLYPHONY);

        // Trigger one more — should steal the oldest (scancode 10, timestamp 0)
        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 999,
                pressed: true,
                stereo_position: 0.0,
            },
        );
        assert_eq!(pool.active_count(), MAX_POLYPHONY);

        let has_999 = pool
            .voices
            .iter()
            .any(|v| v.scancode == 999 && v.is_active());
        assert!(has_999, "Stolen slot should now hold scancode 999");
    }

    #[test]
    fn test_voice_stealing_priority() {
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        // Fill all voices
        for i in 0..MAX_POLYPHONY as u32 {
            pool.trigger(
                &model,
                &KeyEvent {
                    scancode: 10 + i,
                    pressed: true,
                    stereo_position: 0.0,
                },
            );
        }

        // Steal twice: first steals scancode 10, second steals scancode 11
        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 901,
                pressed: true,
                stereo_position: 0.0,
            },
        );
        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 902,
                pressed: true,
                stereo_position: 0.0,
            },
        );

        let has_901 = pool.voices.iter().any(|v| v.scancode == 901);
        let has_902 = pool.voices.iter().any(|v| v.scancode == 902);
        let has_10 = pool
            .voices
            .iter()
            .any(|v| v.scancode == 10 && v.is_active());

        assert!(has_901 && has_902, "New voices should be present");
        assert!(!has_10, "Oldest voice should have been stolen twice ago");
    }

    #[test]
    fn test_process_sample_produces_output() {
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: 0.0,
            },
        );

        let mut peak: f32 = 0.0;
        let mut any_nonzero = false;

        for _ in 0..SAMPLE_RATE as usize {
            let [l, r] = pool.process_sample(&model);
            let amp = l.abs().max(r.abs());
            peak = peak.max(amp);
            if amp > 1e-8 {
                any_nonzero = true;
            }
        }

        assert!(any_nonzero, "Pool should produce non-zero audio");
        assert!(
            peak > 0.01,
            "Peak amplitude should be audible (peak={peak})"
        );
    }

    #[test]
    fn test_process_batch_matches_per_sample() {
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);

        // Render via process_sample. Reset the micro-randomization
        // counter before triggering so both voices start from the same
        // seed — without this, the v5.0.0 micro-randomization gives
        // each voice a unique noise seed + pitch/amplitude drift, and
        // the two paths would (correctly) produce different output.
        // The test's intent is to verify that the batch and per-sample
        // render paths agree FOR THE SAME VOICE STATE, not that two
        // separately-triggered voices are identical.
        //
        // v5.0.1: the counter is now per-instance (not global), so
        // resetting it here only affects THIS `model` instance —
        // other tests running in parallel with their own
        // MechanicalClick instances are unaffected. This eliminates
        // the flaky-test race that v5.0.0's global counter caused.
        model.reset_keystroke_counter_for_tests();
        let mut pool_a = VoicePool::new();
        pool_a.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: 0.0,
            },
        );
        let mut samples_a: Vec<[f32; 2]> = Vec::with_capacity(256);
        for _ in 0..256 {
            samples_a.push(pool_a.process_sample(&model));
        }

        // Render via process (batch). Reset the counter again so this
        // trigger gets the SAME seed as pool_a's trigger above.
        model.reset_keystroke_counter_for_tests();
        let mut pool_b = VoicePool::new();
        pool_b.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: 0.0,
            },
        );
        let mut samples_b = vec![[0.0f32; 2]; 256];
        pool_b.process(&model, &mut samples_b);

        // Compare
        for (i, (a, b)) in samples_a.iter().zip(samples_b.iter()).enumerate() {
            assert!(
                (a[0] - b[0]).abs() < 1e-10 && (a[1] - b[1]).abs() < 1e-10,
                "Mismatch at sample {i}: per_sample={a:?}, batch={b:?}"
            );
        }
    }

    #[test]
    fn test_multiple_voices_mix_correctly() {
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        // Trigger two keys at opposite stereo positions
        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: -1.0,
            },
        );
        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 48,
                pressed: true,
                stereo_position: 1.0,
            },
        );

        let [l, r] = pool.process_sample(&model);

        // Both channels should have significant output (each voice dominates one side)
        assert!(
            l.abs() > 0.001,
            "Left channel should have output from the left-panned voice"
        );
        assert!(
            r.abs() > 0.001,
            "Right channel should have output from the right-panned voice"
        );
    }

    #[test]
    fn test_reset_clears_all_voices() {
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        for i in 0..5u32 {
            pool.trigger(
                &model,
                &KeyEvent {
                    scancode: i,
                    pressed: true,
                    stereo_position: 0.0,
                },
            );
        }
        assert!(pool.active_count() > 0);

        pool.reset();
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_render_one_second_and_write_wav() {
        use std::io::Write;

        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        // Trigger 3 keys at different stereo positions for a spatial spread
        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: -0.7,
            },
        );
        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 48,
                pressed: true,
                stereo_position: 0.0,
            },
        );
        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 2,
                pressed: true,
                stereo_position: 0.5,
            },
        );

        let num_samples = SAMPLE_RATE as usize;
        let mut pcm: Vec<f32> = Vec::with_capacity(num_samples * 2);
        let mut peak: f32 = 0.0;

        for _ in 0..num_samples {
            let [l, r] = pool.process_sample(&model);
            peak = peak.max(l.abs()).max(r.abs());
            pcm.push(l);
            pcm.push(r);
        }

        assert!(peak > 0.01, "Should produce audible output (peak={peak})");

        // Write raw f32 file
        let raw_path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/test_output.raw");
        {
            let _ = std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/target"));
            let mut f = std::fs::File::create(raw_path).expect("Failed to create raw file");
            for &s in &pcm {
                f.write_all(&s.to_le_bytes()).unwrap();
            }
        }
        assert!(std::path::Path::new(raw_path).exists());

        // Write WAV file (32-bit IEEE float, stereo)
        let wav_path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/test_output.wav");
        write_wav_f32(wav_path, &pcm, SAMPLE_RATE as u32);
        assert!(std::path::Path::new(wav_path).exists());

        // Verify WAV header sanity
        let metadata = std::fs::metadata(wav_path).unwrap();
        let expected_size = 44 + (num_samples * 2 * 4) as u64; // 44 byte header + data
        assert_eq!(metadata.len(), expected_size, "WAV file size mismatch");
    }

    #[test]
    fn test_all_voices_naturally_decay() {
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        // Trigger all 16 voices
        for i in 0..MAX_POLYPHONY as u32 {
            pool.trigger(
                &model,
                &KeyEvent {
                    scancode: 100 + i,
                    pressed: true,
                    stereo_position: 0.0,
                },
            );
        }

        // Render until all voices have decayed
        let mut max_iter = 500_000usize;
        while pool.active_count() > 0 && max_iter > 0 {
            let _ = pool.process_sample(&model);
            max_iter -= 1;
        }

        assert_eq!(
            pool.active_count(),
            0,
            "All voices should eventually decay to zero"
        );
        assert!(max_iter > 0, "Voices should decay within a reasonable time");
    }

    #[test]
    fn test_nan_output_replaced_with_silence() {
        // The v1.0.0 crash-proofing guard: if the DSP somehow produces
        // NaN or Infinity, process_sample must return 0.0 (silence)
        // instead of passing the non-finite value to cpal/PipeWire
        // (which would crash the audio server).
        //
        // We can't easily make MechanicalClick produce NaN directly
        // (the TPT SVF is stable), but we CAN test the guard by
        // constructing a custom AcousticModel that returns NaN.
        use zactrix_profiles::{AcousticModel, KeyEvent, KeyProfile, SynthState};

        struct NanModel;
        impl AcousticModel for NanModel {
            fn get_profile(&self, _event: &KeyEvent) -> KeyProfile {
                KeyProfile::default()
            }
            fn init_state(
                &self,
                _profile: &KeyProfile,
                state: &mut SynthState,
                _stereo_position: f32,
            ) {
                state.active = true;
            }
            fn render_sample(&self, _state: &mut SynthState) -> [f32; 2] {
                [f32::NAN, f32::INFINITY]
            }
        }

        let model = NanModel;
        let mut pool = VoicePool::new();

        // Trigger a voice so process_sample has something to render.
        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 42,
                pressed: true,
                stereo_position: 0.0,
            },
        );

        // The model returns NaN/Infinity, but process_sample MUST
        // return 0.0 for both channels (the guard catches it).
        let [l, r] = pool.process_sample(&model);
        assert!(
            l.is_finite(),
            "Left channel must be finite (got {l}), NaN guard failed"
        );
        assert!(
            r.is_finite(),
            "Right channel must be finite (got {r}), NaN guard failed"
        );
        assert_eq!(l, 0.0, "NaN input should produce 0.0 silence");
        assert_eq!(r, 0.0, "Infinity input should produce 0.0 silence");
    }

    #[test]
    fn test_normal_output_not_affected_by_nan_guard() {
        // Verify the NaN guard doesn't affect normal (finite) output.
        let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
        let mut pool = VoicePool::new();

        pool.trigger(
            &model,
            &KeyEvent {
                scancode: 30,
                pressed: true,
                stereo_position: 0.0,
            },
        );

        let [l, r] = pool.process_sample(&model);
        assert!(l.is_finite(), "Normal output must be finite");
        assert!(r.is_finite(), "Normal output must be finite");
        // The click transient should produce non-zero output.
        assert!(
            l.abs() > 0.0 || r.abs() > 0.0,
            "Normal voice should produce audio"
        );
    }
}
