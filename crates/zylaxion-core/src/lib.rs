// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Brain — orchestrates input → engine → output.
//!
//! [`Orchestrator`] connects the three layers of Zylaxion:
//!
//! ```text
//!  zylaxion-input          zylaxion-core           zactrix-engine          zylaxion-output
//!  (The Ears)              (The Brain)            (Zactrix Engine)        (The Mouth)
//!  ───────────             ───────────             ────────────────        ──────────────
//!  KeyEvent ──channel──►  recv_timeout()
//!                                │
//!                          trigger / release
//!                                │
//!                                ▼
//!                          VoicePool::process()
//!                                │
//!                          [[f32; 2]] batch
//!                                │
//!                                ▼
//!                          AudioSink::write_batch()
//!                                                          ringbuf ──►  cpal callback
//! ```
//!
//! The main loop uses a **continuous feed** model: the producer thread
//! greedily fills the ring buffer and only sleeps when it is full,
//! ensuring the cpal audio callback never starves.

use std::fmt;
use std::time::Duration;

use crossbeam_channel::Receiver;
use zactrix_engine::VoicePool;
use zactrix_profiles::{AcousticModel, KeyEvent as ProfileKeyEvent, SAMPLE_RATE};
use zylaxion_input::KeyEvent as InputKeyEvent;
use zylaxion_output::AudioSink;

// ── Constants ───────────────────────────────────────────────────────────

/// Maximum number of stereo frames rendered per loop iteration.
///
/// 256 frames at 44.1 kHz ≈ 5.8 ms.  Matches the cpal hardware buffer
/// size so each iteration produces exactly what the audio callback will
/// request next, keeping the ring buffer topped up without waste.
const MAX_RENDER_CHUNK: usize = 256;

/// Number of silence frames pre-filled into the ring buffer before
/// starting the audio stream.  4096 frames (~92 ms) survives the worst
/// Linux non-RT scheduler jitter (30–50 ms) with comfortable margin,
/// guaranteeing the audio callback never sees an empty buffer at startup.
const PREFILL_SILENCE_FRAMES: usize = 4096;

/// If the ring buffer has fewer vacant frames than this threshold,
/// the buffer is considered "mostly full" and the producer sleeps
/// briefly to avoid burning CPU.  512 frames ≈ 11.6 ms — roughly
/// double the period of a typical ALSA fragment, so the callback
/// will drain some frames well before we wake up.
const SLEEP_THRESHOLD: usize = 512;

/// Sleep duration when the ring buffer is mostly full, to avoid
/// spinning the CPU at 100 % while the audio callback drains frames.
const FULL_BUFFER_SLEEP: Duration = Duration::from_millis(1);

// ── Pan mapping ─────────────────────────────────────────────────────────

/// Map a Linux evdev scancode to a stereo pan position.
///
/// Standard QWERTY keyboards have roughly 15 columns.  Scancodes are
/// not contiguous across rows, so we use a simple heuristic: remap the
/// scancode into a 0–1 range based on common key columns, then shift
/// to [-1, 1].
///
/// | Region (scancodes)          | Position |
/// |----------------------------|----------|
/// | 1–14 (Escape, F1–F10…)     | far left → center-left |
/// | 16–27 (QWERTY row left)     | left → center |
/// | 30–41, 43–53 (main rows)   | full spread |
/// | 57–69 (right cluster)      | center-right → right |
///
/// The mapping is deliberately simple — it just needs to produce
/// audible stereo separation, not a physically accurate model.
fn scancode_to_pan(scancode: u32) -> f32 {
    // Approximate column index: most alphanumeric keys are in the
    // range 2–69.  Normalise to 0..1 then shift to -1..1.
    let column = match scancode {
        // Row 1: Escape + F-keys (far left to center-left)
        1 => 0.0,
        // Row 1–2: Escape + F-keys + number row (scancodes 2–14)
        2..=14 => (scancode - 2) as f32 / 12.0,
        // Row 3: QWERTY row
        16..=27 => (scancode - 16) as f32 / 11.0,
        // Row 4: ASDF row
        30..=41 => (scancode - 30) as f32 / 11.0,
        // Row 5: ZXCV row
        42..=53 => (scancode - 42) as f32 / 11.0,
        // Row 6: bottom row (Shift=42.., Ctrl=29/97, Alt=56/100, Space=57)
        57 => 0.5, // Space — center
        // Right-side modifier / arrow / nav cluster
        86..=100 => 0.7,
        // Everything else: center
        _ => 0.5,
    };

    // Map [0, 1] → [-1, 1] with slight bias toward center for a
    // more natural stereo image.
    (column - 0.5) * 2.0
}

// ── OrchestratorError ───────────────────────────────────────────────────

/// Errors from the orchestrator.
#[derive(Debug)]
pub enum OrchestratorError {
    /// Audio output device could not be opened.
    Audio(zylaxion_output::AudioError),
    /// Input source could not be started.
    Input(zylaxion_input::InputError),
}

impl fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audio(e) => write!(f, "audio: {e}"),
            Self::Input(e) => write!(f, "input: {e}"),
        }
    }
}

impl std::error::Error for OrchestratorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Audio(e) => Some(e),
            Self::Input(e) => Some(e),
        }
    }
}

// ── Orchestrator ────────────────────────────────────────────────────────

/// Wires keyboard input → DSP engine → audio output in a real-time loop.
///
/// # Lifecycle
///
/// 1. Construct with [`Orchestrator::new`] (creates `CpalSink` + `VoicePool`).
/// 2. Call [`run`] with a channel receiver and an acoustic model.
/// 3. The loop runs until the receiver is disconnected (Ctrl+C / SIGINT).
///
/// # Type Parameters
///
/// - `M`: The [`AcousticModel`] that defines how keys sound (e.g. [`MechanicalClick`](zactrix_profiles::MechanicalClick)).
pub struct Orchestrator {
    sink: zylaxion_output::CpalSink,
    pool: VoicePool,
}

impl Orchestrator {
    /// Create the orchestrator, initialising the audio output and voice pool.
    ///
    /// # Errors
    ///
    /// Returns [`OrchestratorError::Audio`] if no audio device is available
    /// or the stream cannot be started.
    pub fn new() -> Result<Self, OrchestratorError> {
        let sink = zylaxion_output::CpalSink::new().map_err(OrchestratorError::Audio)?;
        let pool = VoicePool::new();
        Ok(Self { sink, pool })
    }

    /// Run the main input → render → output loop.
    ///
    /// Blocks the calling thread until the `event_rx` receiver is
    /// disconnected (e.g. the input background thread has shut down
    /// due to Ctrl+C).
    ///
    /// # Arguments
    ///
    /// * `model` — The acoustic model that synthesises key sounds.
    ///   Must implement [`AcousticModel`] and typically outlive the
    ///   orchestrator (e.g. a static reference).
    /// * `event_rx` — Channel receiver yielding [`InputKeyEvent`]s from
    ///   the input layer.
    pub fn run<M: AcousticModel>(&mut self, model: &M, event_rx: &Receiver<InputKeyEvent>) {
        // ── Pre-fill ring buffer with silence ──────────────────────
        // Prevents the very first cpal callback from hitting an empty
        // buffer (startup underrun).  The silence is harmless — it
        // plays for ~23 ms before the loop catches up.
        let silence = [0.0f32; 2];
        for _ in 0..PREFILL_SILENCE_FRAMES {
            self.sink.write_sample(silence);
        }

        // Report device info once — confirm continuous-feed tuning.
        let device_rate = self.sink.sample_rate();
        eprintln!(
            "[zylaxion-core] Continuous-feed mode — device rate: {device_rate} Hz, \
             max render chunk: {MAX_RENDER_CHUNK} frames (~{:.2} ms)",
            MAX_RENDER_CHUNK as f64 / SAMPLE_RATE as f64 * 1000.0,
        );
        eprintln!(
            "[zylaxion-core] Ring buffer: 16384 frames (~{:.1} ms), \
             cpal hw buffer: default (PipeWire/ALSA negotiated), \
             pre-fill: {PREFILL_SILENCE_FRAMES} frames (~{:.1} ms), \
             sleep threshold: {SLEEP_THRESHOLD} frames (~{:.1} ms)",
            16384.0 / SAMPLE_RATE as f64 * 1000.0,
            PREFILL_SILENCE_FRAMES as f64 / SAMPLE_RATE as f64 * 1000.0,
            SLEEP_THRESHOLD as f64 / SAMPLE_RATE as f64 * 1000.0,
        );

        // Pre-allocate the largest possible render buffer once —
        // no allocation in the hot loop.  We slice it down to the
        // actual chunk size each iteration.
        let mut batch = [[0.0f32; 2]; MAX_RENDER_CHUNK];

        // ── Continuous feed loop ───────────────────────────────────
        loop {
            // 1. Drain all pending KeyEvents (non-blocking).
            //    Also detects channel disconnect for clean shutdown:
            //    when the Sender is dropped, `try_recv` returns
            //    `Disconnected` once the queue is empty.
            loop {
                match event_rx.try_recv() {
                    Ok(event) => Self::handle_input_event(&mut self.pool, model, &event),
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        eprintln!("[zylaxion-core] input channel disconnected — shutting down");
                        return;
                    }
                }
            }

            // 2. Determine how many frames the ring buffer can accept.
            let vacancy = self.sink.producer_vacancy();

            if vacancy < SLEEP_THRESHOLD {
                // Buffer is mostly full — the audio callback hasn't
                // drained enough yet.  Sleep briefly to avoid 100 %
                // CPU, then re-check.  The SLEEP_THRESHOLD ensures we
                // always keep a comfortable headroom of audio queued.
                std::thread::sleep(FULL_BUFFER_SLEEP);
                continue;
            }

            // 3. Render up to `vacancy` frames (capped at MAX_RENDER_CHUNK).
            let chunk_len = vacancy.min(MAX_RENDER_CHUNK);
            let chunk = &mut batch[..chunk_len];

            // Render silence into the chunk first, then let process()
            // accumulate on top — this zeroes any residual from a
            // previous iteration that used a shorter slice.
            for frame in chunk.iter_mut() {
                *frame = [0.0f32; 2];
            }

            // VoicePool::process accumulates into the buffer (does
            // NOT clear it), matching its documented contract.
            self.pool.process(model, chunk);

            // 4. Push the rendered audio into the ring buffer.
            //    `write_batch` never blocks — it silently drops if
            //    the buffer filled between the vacancy check and now.
            self.sink.write_batch(chunk);
        }
    }

    /// Process a single input event: trigger on press, release on release.
    #[inline]
    fn handle_input_event<M: AcousticModel>(
        pool: &mut VoicePool,
        model: &M,
        event: &InputKeyEvent,
    ) {
        if event.pressed {
            let pan = scancode_to_pan(event.scancode);
            pool.trigger(
                model,
                &ProfileKeyEvent {
                    scancode: event.scancode,
                    pressed: true,
                    stereo_position: pan,
                },
            );
        } else {
            pool.release(event.scancode);
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scancode_pan_produces_reasonable_range() {
        // Scan a wide range of scancodes and verify the pan values
        // stay within [-1, 1].  Some edge-case scancodes may land at
        // the default 0.5 → 0.0 mapping, which is fine.
        for sc in 0..200u32 {
            let pan = scancode_to_pan(sc);
            assert!(
                (-1.0..=1.0).contains(&pan),
                "pan for scancode {sc} out of range: {pan}"
            );
        }
    }

    #[test]
    fn scancode_pan_left_keys_are_negative() {
        // Left-side keys (Q=16, A=30, Z=42) should pan left.
        assert!(scancode_to_pan(16) < 0.0, "Q should pan left");
        assert!(scancode_to_pan(30) < 0.0, "A should pan left");
    }

    #[test]
    fn scancode_pan_right_keys_are_positive() {
        // Right-side keys (P=25, L=38, /=53) should pan right.
        assert!(scancode_to_pan(25) > 0.0, "P should pan right");
        assert!(scancode_to_pan(38) > 0.0, "L should pan right");
    }

    #[test]
    fn scancode_pan_stereo_separation() {
        // Q (left) and P (right) should have different pan values.
        let left = scancode_to_pan(16); // Q
        let right = scancode_to_pan(25); // P
        assert_ne!(
            left, right,
            "Q and P should have different stereo positions"
        );
        assert!(left < right, "Q should be more left than P");
    }

    #[test]
    fn scancode_pan_space_is_center() {
        // Space bar (scancode 57) should map near center.
        let pan = scancode_to_pan(57);
        assert!(
            (pan - 0.0).abs() < 0.1,
            "Space should be near center, got {pan}"
        );
    }

    #[test]
    fn render_chunk_duration_is_sane() {
        let duration_ms = MAX_RENDER_CHUNK as f64 / SAMPLE_RATE as f64 * 1000.0;
        assert!(
            duration_ms > 0.5 && duration_ms < 10.0,
            "max render chunk duration should be 0.5–10 ms, got {duration_ms:.2} ms"
        );
    }

    #[test]
    fn orchestrator_error_display() {
        let e = OrchestratorError::Audio(zylaxion_output::AudioError::NoDeviceAvailable);
        let msg = e.to_string();
        assert!(msg.contains("audio"));
    }

    #[test]
    fn orchestrator_error_is_error() {
        let e = OrchestratorError::Input(zylaxion_input::InputError::LibinputError("test".into()));
        let _: &dyn std::error::Error = &e;
    }
}
