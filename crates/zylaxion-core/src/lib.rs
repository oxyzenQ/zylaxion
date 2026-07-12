// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use crossbeam_channel::Receiver;
use zactrix_engine::VoicePool;
use zactrix_profiles::{AcousticModel, KeyEvent as ProfileKeyEvent};
use zylaxion_input::KeyEvent as InputKeyEvent;
use zylaxion_output::AudioSink;

// ── Constants ───────────────────────────────────────────────────────────

/// Maximum number of stereo frames rendered per loop iteration.
///
/// 64 frames at 44.1 kHz ≈ 1.45 ms.  Small enough that simultaneous
/// key presses are processed within one render cycle, preventing
/// the second key from waiting behind a large chunk.
const MAX_RENDER_CHUNK: usize = 64;

/// Number of silence frames pre-filled into the ring buffer before
/// starting the audio stream.  Just enough to cover one ALSA period
/// (~46 ms) so the very first callback doesn't underrun.  Kept small
/// because pre-fill adds perceived latency — the audio callback
/// outputs silence naturally when the ring buffer is empty.
const PREFILL_SILENCE_FRAMES: usize = 2048;

/// Number of silence frames pushed into the ring buffer immediately
/// before the orchestrator exits and drops `CpalSink`.
///
/// Without this tail, dropping `CpalSink` mid-playback can leave a
/// non-zero DC offset in PipeWire's internal buffer. When the next
/// audio client (e.g. a music player) starts writing to the same
/// device, PipeWire reuses the same buffer and the sudden jump from
/// the residual offset to the new client's signal is heard as a
/// "pop" or an audible volume jump. Flushing ~23 ms of pure silence
/// (1024 frames at 44.1 kHz) gives the cpal callback time to drain
/// any remaining non-zero samples and settle the device back to a
/// zero baseline before the stream is torn down.
const FADEOUT_SILENCE_FRAMES: usize = 1024;

/// If the ring buffer has fewer vacant frames than this threshold,
/// the buffer is considered "mostly full" and the producer yields.
/// 128 frames ≈ 2.9 ms — enough for the ALSA callback to drain a
/// few periods before we re-check.
const SLEEP_THRESHOLD: usize = 128;

/// Timeout for `recv_timeout` when waiting for key events.
/// Wakes immediately on event arrival; 1 ms fallback keeps the
/// render loop responsive when voices are decaying.
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
    /// Create the orchestrator, initialising the audio output and voice pool
    /// with the default master volume (5.5× — laptop-speaker tuned).
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

    /// Create the orchestrator with a custom master volume (v10.2.0+ —
    /// dragonzen audit P1).
    ///
    /// `master_volume` is a linear gain multiplier. See
    /// [`VoicePool::with_volume`] for recommended values per output
    /// device type.
    ///
    /// # Errors
    ///
    /// Returns [`OrchestratorError::Audio`] if no audio device is available
    /// or the stream cannot be started.
    pub fn with_master_volume(master_volume: f32) -> Result<Self, OrchestratorError> {
        let sink = zylaxion_output::CpalSink::new().map_err(OrchestratorError::Audio)?;
        let pool = VoicePool::with_volume(master_volume);
        Ok(Self { sink, pool })
    }

    /// Update the master volume at runtime (v10.2.0+ — dragonzen audit
    /// P1). Useful for hot-reload when the user edits `[master]` in
    /// `config.toml`. The value is clamped to a sane range — see
    /// [`VoicePool::set_master_volume`].
    #[inline]
    pub fn set_master_volume(&mut self, volume: f32) {
        self.pool.set_master_volume(volume);
    }

    /// Return the current master volume (linear gain multiplier).
    #[inline]
    pub fn master_volume(&self) -> f32 {
        self.pool.master_volume()
    }

    /// Return the actual sample rate of the audio device (Hz).
    ///
    /// Used by callers to construct an `AcousticModel` with the correct
    /// sample rate for DSP coefficient calculations.
    #[inline]
    pub fn sample_rate(&self) -> u32 {
        self.sink.sample_rate()
    }

    /// Run the main input → render → output loop.
    ///
    /// Blocks the calling thread until one of three conditions:
    ///
    /// 1. `event_rx` is disconnected (input source shut down).
    /// 2. `stop_flag` is set to `true` (e.g. IPC "stop" command or SIGTERM).
    ///
    /// When the loop exits, `CpalSink` is dropped, releasing the audio
    /// device and un-stalling the PipeWire graph.
    ///
    /// # Hot-reload support
    ///
    /// The `model` parameter is an `Arc<ArcSwap<M>>` — a lock-free
    /// atomic pointer to the acoustic model. The IPC "reload" command
    /// can swap the pointer at any time (from another thread) by
    /// calling `model.store(Arc::new(new_model))`. The render loop
    /// picks up the new model on its next iteration without blocking.
    ///
    /// - **Active voices** (currently decaying) keep using the profile
    ///   they captured at trigger time — they finish naturally.
    /// - **New keypresses** pick up the new model immediately.
    ///
    /// This satisfies the strict rule: no blocking locks inside the
    /// cpal audio callback. `ArcSwap::load()` is a single atomic load,
    /// no Mutex involved.
    ///
    /// # Arguments
    ///
    /// * `model` — Hot-swappable acoustic model behind an `ArcSwap`.
    /// * `event_rx` — Channel receiver yielding [`InputKeyEvent`]s from
    ///   the input layer.
    /// * `stop_flag` — Shared flag checked each loop iteration; when
    ///   `true`, the loop breaks and the audio device is released.
    pub fn run<M: AcousticModel>(
        &mut self,
        model: &Arc<ArcSwap<M>>,
        event_rx: &Receiver<InputKeyEvent>,
        stop_flag: Arc<AtomicBool>,
    ) {
        // ── Pre-fill ring buffer with minimal silence ─────────────
        // Only enough to cover one ALSA period so the very first
        // callback doesn't underrun.  The cpal callback outputs
        // silence naturally when the ring buffer is empty, so we
        // keep this small to avoid adding perceived latency.
        let silence = [0.0f32; 2];
        for _ in 0..PREFILL_SILENCE_FRAMES {
            self.sink.write_sample(silence);
        }

        // Report device info once — confirm interrupt-driven tuning.
        let device_rate = self.sink.sample_rate();
        let sr = device_rate as f64;
        eprintln!(
            "[zylaxion-core] Interrupt-driven mode — device rate: {device_rate} Hz, \
             render chunk: {MAX_RENDER_CHUNK} frames (~{:.2} ms)",
            MAX_RENDER_CHUNK as f64 / sr * 1000.0,
        );
        eprintln!(
            "[zylaxion-core] Ring buffer: 16384 frames (~{:.1} ms), \
             cpal hw buffer: default (PipeWire/ALSA negotiated), \
             pre-fill: {PREFILL_SILENCE_FRAMES} frames (~{:.1} ms)",
            16384.0 / sr * 1000.0,
            PREFILL_SILENCE_FRAMES as f64 / sr * 1000.0,
        );

        // Pre-allocate the largest possible render buffer once —
        // no allocation in the hot loop.  We slice it down to the
        // actual chunk size each iteration.
        let mut batch = [[0.0f32; 2]; MAX_RENDER_CHUNK];

        // ── Interrupt-driven loop ────────────────────────────────
        loop {
            // 0. Check stop flag — IPC "stop" command or SIGTERM
            //    handler sets this to true.  Break immediately so
            //    CpalSink is dropped and PipeWire graph is released.
            if stop_flag.load(Ordering::Relaxed) {
                eprintln!("[zylaxion-core] stop flag set — shutting down");
                self.fade_out_before_drop();
                return;
            }

            // 1. Block until a key event arrives (wakes immediately)
            //    or the 1 ms timeout expires.  This is the primary
            //    idle mechanism — no CPU spin when no keys are pressed.
            //    Crucially, recv_timeout wakes the thread the instant
            //    the first key of a simultaneous pair arrives, so the
            //    second key (arriving µs later) is processed in the
            //    drain loop below with zero extra latency.
            match event_rx.recv_timeout(EVENT_POLL_TIMEOUT) {
                Ok(event) => {
                    // Snapshot the current model ONCE per event batch.
                    // ArcSwap::load() is a single atomic load — no
                    // blocking, no allocation. The Guard keeps the
                    // snapshot alive for the duration of this batch
                    // even if another thread swaps in a new model
                    // mid-batch.
                    let model_guard = model.load();
                    let model_ref: &M = &model_guard;

                    Self::handle_input_event(&mut self.pool, model_ref, &event);
                    // Drain any co-arriving events (e.g. 'k' and 'a'
                    // pressed within the same µs window).
                    while let Ok(event) = event_rx.try_recv() {
                        Self::handle_input_event(&mut self.pool, model_ref, &event);
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    eprintln!("[zylaxion-core] input channel disconnected — shutting down");
                    self.fade_out_before_drop();
                    return;
                } // Timeout is handled by falling through — the loop
                  // re-checks the stop flag at the top of the next iteration.
            }

            // 2. Only render and push when voices are active.
            //    When idle, the ring buffer stays empty so the cpal
            //    callback outputs silence naturally (zero-latency).
            if !self.pool.is_active() {
                continue;
            }

            // 3. Determine how many frames the ring buffer can accept.
            let vacancy = self.sink.producer_vacancy();

            if vacancy < SLEEP_THRESHOLD {
                // Buffer is mostly full — the audio callback hasn't
                // drained enough yet.  Loop back to recv_timeout
                // which blocks efficiently instead of spinning.
                continue;
            }

            // 4. Render up to `vacancy` frames (capped at MAX_RENDER_CHUNK).
            let chunk_len = vacancy.min(MAX_RENDER_CHUNK);
            let chunk = &mut batch[..chunk_len];

            // Render silence into the chunk first, then let process()
            // accumulate on top — this zeroes any residual from a
            // previous iteration that used a shorter slice.
            for frame in chunk.iter_mut() {
                *frame = [0.0f32; 2];
            }

            // Snapshot the model for this render batch. Same lock-free
            // load as above. Active voices use whatever profile they
            // captured at trigger time, so a mid-batch swap is safe.
            let model_guard = model.load();
            let model_ref: &M = &model_guard;

            // VoicePool::process accumulates into the buffer (does
            // NOT clear it), matching its documented contract.
            self.pool.process(model_ref, chunk);

            // 5. Push the rendered audio into the ring buffer.
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
            // v10.2.0 (dragonzen audit N5): record the keypress timestamp
            // BEFORE init_state so the model can compute the inter-
            // keystroke interval and attenuate fast repeats by up to -5%.
            // The default trait impl is a no-op; only models that care
            // about timing (e.g. MechanicalClick) override it.
            let _ = model.record_trigger_timestamp(event.timestamp);
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

    /// Flush a tail of pure silence into the ring buffer before the
    /// orchestrator returns and `CpalSink` is dropped.
    ///
    /// This prevents the "pop" / volume-jump artefact that otherwise occurs
    /// when PipeWire reuses its internal buffer for the next audio client
    /// while it still holds a non-zero DC offset from Zylaxion's last
    /// rendered frame. The 1024-frame silence pad gives the cpal callback
    /// enough time (~23 ms at 44.1 kHz) to drain any remaining non-zero
    /// samples and settle the device back to a zero baseline before the
    /// stream is torn down.
    ///
    /// Called from both exit paths of [`run`](Self::run): the `stop_flag`
    /// path (IPC `stop` command / SIGTERM) and the input-channel
    /// `Disconnected` path (input source thread died).
    ///
    /// # Pacing (v10.2.0+ — dragonzen audit B5)
    ///
    /// Previously this method called `write_sample` in a tight loop
    /// `FADEOUT_SILENCE_FRAMES` times. If the ring buffer was mostly
    /// full when stop was requested (e.g. the user mashed a key during
    /// shutdown), the silence writes were silently dropped by
    /// `producer.try_push` — the cpal callback continued playing the
    /// queued decay tail, then the sink was dropped mid-tail, and the
    /// next audio client heard the residual DC offset. The fade-out
    /// failed silently.
    ///
    /// The fix paces the silence writes against `producer_vacancy()`:
    /// when the buffer is mostly full, we sleep briefly to let the
    /// cpal callback drain some frames, then retry. We also sleep
    /// for the duration of the silence pad at the end so the cpal
    /// callback has time to actually drain it before `CpalSink::drop`
    /// tears down the stream.
    fn fade_out_before_drop(&mut self) {
        let silence = [0.0f32; 2];
        let mut written = 0usize;
        while written < FADEOUT_SILENCE_FRAMES {
            let vacancy = self.sink.producer_vacancy();
            if vacancy == 0 {
                // Buffer is full — wait for the cpal callback to drain
                // some frames. 2 ms is ~88 frames at 44.1 kHz, enough
                // to make progress without busy-spinning.
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            let n = vacancy.min(FADEOUT_SILENCE_FRAMES - written);
            for _ in 0..n {
                self.sink.write_sample(silence);
            }
            written += n;
        }

        // Give the cpal callback time to actually drain the silence pad
        // before `CpalSink::drop` tears down the stream. The sleep
        // duration is the silence-pad duration plus a small margin for
        // scheduler jitter. Without this, dropping the sink immediately
        // after writing the silence can leave it half-consumed in the
        // ring buffer, and PipeWire's drain behavior on stream teardown
        // is implementation-defined.
        let drain_ms = (FADEOUT_SILENCE_FRAMES as f64 / self.sink.sample_rate() as f64 * 1000.0)
            .ceil() as u64
            + 5; // 5 ms jitter margin
        std::thread::sleep(Duration::from_millis(drain_ms));
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
        let duration_ms = MAX_RENDER_CHUNK as f64 / 44100.0 * 1000.0;
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
