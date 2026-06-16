// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

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
/// let model = MechanicalClick::new();
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
}

impl VoicePool {
    /// Create a new voice pool with all voices initialized to inactive.
    pub fn new() -> Self {
        Self {
            voices: core::array::from_fn(|_| Voice::new()),
            trigger_counter: 0,
        }
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
    pub fn release(&mut self, scancode: u32) {
        for voice in &mut self.voices {
            if voice.is_active() && voice.scancode == scancode {
                voice.state.active = false;
            }
        }
    }

    /// Process a single sample for all active voices and return the mixed stereo output.
    ///
    /// This is the primary zero-allocation render path for real-time audio callbacks.
    /// Each call advances all active voices by exactly one sample.
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
        out
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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_release_nonexistent_scancode_is_noop() {
        let model = MechanicalClick::new();
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
    fn test_voice_stealing_replaces_oldest() {
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();

        // Render via process_sample
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

        // Render via process (batch)
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
        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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

        let model = MechanicalClick::new();
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
        let model = MechanicalClick::new();
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
}
