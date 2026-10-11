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

use zactrix_profiles::{AcousticModel, KeyTrigger, MAX_POLYPHONY};

use crate::Voice;

/// Pre-allocated polyphonic voice pool.
///
/// # Example
///
/// ```rust,ignore
/// use zactrix_engine::VoicePool;
/// use zactrix_profiles::{MechanicalClick, KeyTrigger};
///
/// let model = MechanicalClick::new(zactrix_profiles::SAMPLE_RATE as u32);
/// let mut pool = VoicePool::new();
///
/// pool.trigger(&model, &KeyTrigger {
///     scancode: 30, pressed: true, stereo_position: -0.3, velocity: None,
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
    /// For headphones (especially IEMs at 16-32 ohm), 5.5× with hard
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
    /// - **1.5**: headphones (especially IEMs at 16-32 ohm).
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
    pub fn trigger<M: AcousticModel>(&mut self, model: &M, event: &KeyTrigger) {
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
        model.init_state(
            &voice.profile,
            &mut voice.state,
            event.stereo_position,
            event.velocity,
        );
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
                // v10.2.0 (dragonzen audit P4): zero the scancode now
                // so it doesn't persist in memory after the voice
                // deactivates. The scancode is a hardware evdev code
                // that can be mapped back to typed text via the user's
                // keyboard layout — leaving it in the slot until the
                // next trigger is a (small) privacy leak vector if
                // the daemon crashes and dumps core. Zeroing on
                // release ensures the slot holds no scancode between
                // keypress events.
                voice.scancode = 0;
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
#[path = "../../test/pool_tests.rs"]
mod tests;
