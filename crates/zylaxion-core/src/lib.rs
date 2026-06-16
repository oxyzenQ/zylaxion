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
//! The main loop uses `recv_timeout` so that audio rendering continues
//! even when no keys are pressed, preventing buffer underruns.

use std::fmt;
use std::time::Duration;

use crossbeam_channel::Receiver;
use zactrix_engine::VoicePool;
use zactrix_profiles::{AcousticModel, KeyEvent as ProfileKeyEvent, SAMPLE_RATE};
use zylaxion_input::KeyEvent as InputKeyEvent;
use zylaxion_output::AudioSink;

// ── Constants ───────────────────────────────────────────────────────────

/// Number of stereo frames rendered per loop iteration.
///
/// 64 frames at 44.1 kHz ≈ 1.45 ms of audio.  Small enough to keep
/// input-to-output latency well below human perception, large enough
/// that the per-iteration overhead is negligible.
const RENDER_CHUNK: usize = 64;

/// Timeout for `recv_timeout` when no input events are pending.
///
/// Should be shorter than the render-chunk duration so the loop can
/// always keep the ring buffer fed.  At 44.1 kHz the 64-frame chunk
/// lasts ~1.45 ms, so a 1 ms timeout is ideal.
const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(1);

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
        // Pre-allocate the render buffer once — no allocation in the loop.
        let mut batch = [[0.0f32; 2]; RENDER_CHUNK];

        // Report device info once — confirm low-latency tuning is active.
        let device_rate = self.sink.sample_rate();
        eprintln!(
            "[zylaxion-core] Low-latency mode active — device rate: {device_rate} Hz, \
             render chunk: {RENDER_CHUNK} frames (~{:.2} ms)",
            RENDER_CHUNK as f64 / SAMPLE_RATE as f64 * 1000.0,
        );
        eprintln!(
            "[zylaxion-core] Ring buffer: 4096 frames (~{:.1} ms), \
             cpal hw buffer: 64 frames (~{:.2} ms)",
            4096.0 / SAMPLE_RATE as f64 * 1000.0,
            64.0 / SAMPLE_RATE as f64 * 1000.0,
        );

        // ── Main loop ──────────────────────────────────────────────
        loop {
            // Drain all pending key events before rendering.  This
            // ensures that rapid key presses within one poll window
            // are all processed at the same sample position, avoiding
            // ordering artefacts.
            loop {
                match event_rx.recv_timeout(EVENT_POLL_TIMEOUT) {
                    Ok(event) => Self::handle_input_event(&mut self.pool, model, &event),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => break,
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        eprintln!("[zylaxion-core] input channel disconnected — shutting down");
                        return;
                    }
                }
            }

            // Render a chunk of audio from the voice pool.
            for frame in batch.iter_mut() {
                *frame = self.pool.process_sample(model);
            }

            // Push the rendered audio into the ring buffer that feeds
            // the cpal audio callback.  `write_batch` never blocks.
            self.sink.write_batch(&batch);
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
        let duration_ms = RENDER_CHUNK as f64 / SAMPLE_RATE as f64 * 1000.0;
        assert!(
            duration_ms > 0.5 && duration_ms < 10.0,
            "render chunk duration should be 0.5–10 ms, got {duration_ms:.2} ms"
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
