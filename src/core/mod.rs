// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The Brain — orchestrates input → engine → output.
//!
//! [`Orchestrator`] connects the three layers of zylaxion:
//!
//! ```text
//!  input (The Ears)        core (The Brain)       engine (The Voice)      audio (The Mouth)
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

use std::sync::Arc;

mod pan;
use pan::scancode_to_pan;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::audio::AudioSink;
use crate::engine::VoicePool;

use crate::profiles::{AcousticModel, KeyTrigger};
use arc_swap::ArcSwap;
use crossbeam_channel::Receiver;

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

// ── OrchestratorError ───────────────────────────────────────────────────
//
// night-improve-21: the error contract lives in its own subsystem
// directory now (src/core/error/ — the no-mercy LOC split, a pure
// move); the root re-export keeps every consumer path stable.

mod error;
pub use error::OrchestratorError;

// ── Orchestrator ────────────────────────────────────────────────────────

/// A keyboard event delivered to the render loop (night-improve-23:
/// relocated here from the retired libinput module — the shape is the
/// contract between the kernel-side tap and the engine, and the engine
/// owns that contract now).
///
/// The `scancode` is the evdev key code as defined in
/// `<linux/input-event-codes.h>` (e.g. `KEY_A = 30`).
///
/// # Privacy note (carried from the libinput era)
///
/// The raw hardware scancode is the physical key location, not an ASCII
/// character. For privacy reasons, **never log the scancode field** in
/// production code — a journal reader could reconstruct typed passwords
/// by mapping scancodes back through the QWERTY layout. The struct
/// exists to drive the DSP engine, not for human inspection.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KeyEvent {
    /// Linux evdev key code (e.g. 30 for `KEY_A`).
    pub scancode: u32,
    /// `true` when the key was pressed, `false` when released.
    pub pressed: bool,
    /// Timestamp in microseconds sourced from `CLOCK_MONOTONIC` (the
    /// tap stamps it with bpf_ktime_get_ns / 1000 — the same clock and
    /// unit the old libinput path used).
    pub timestamp: u64,
}

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
/// - `S`: The [`AudioSink`] implementation. In production this is always
///   `CpalSink`. In tests, a `MockSink` can be injected via
///   [`Orchestrator::new_with`] to verify the render path without opening
///   a real audio device (v10.2.0+ — dragonzen audit P8).
pub struct Orchestrator<S: AudioSink> {
    sink: S,
    pool: VoicePool,
}

impl Orchestrator<crate::audio::CpalSink> {
    /// Create the orchestrator, initialising the audio output and voice pool
    /// with the default master volume (5.5× — laptop-speaker tuned).
    ///
    /// # Errors
    ///
    /// Returns [`OrchestratorError::Audio`] if no audio device is available
    /// or the stream cannot be started.
    // night-improve-21: production constructs through
    // with_master_volume (the daemon always applies the config
    // volume); this default-volume constructor stays as the documented
    // no-config entry — the zylaxion_live example is its caller, so
    // the marker is an allow, not an expect.
    #[allow(dead_code)]
    pub fn new() -> Result<Self, OrchestratorError> {
        let sink = crate::audio::CpalSink::new().map_err(OrchestratorError::Audio)?;
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
        let sink = crate::audio::CpalSink::new().map_err(OrchestratorError::Audio)?;
        let pool = VoicePool::with_volume(master_volume);
        Ok(Self { sink, pool })
    }
}

impl<S: AudioSink> Orchestrator<S> {
    /// Create an orchestrator with a pre-built sink and default master
    /// volume (v10.2.0+ — dragonzen audit P8).
    ///
    /// This constructor is primarily for **testing** — it allows injecting
    /// a `MockSink` that records samples without opening a real audio
    /// device. Production code should use [`new`](Self::new) or
    /// [`with_master_volume`](Self::with_master_volume) instead.
    // night-improve-21: exercised by the core test tree (MockSink
    // injection) only; the unified binary's non-test compilation never
    // calls it, hence the allow.
    #[allow(dead_code)]
    pub fn new_with(sink: S) -> Self {
        Self {
            sink,
            pool: VoicePool::new(),
        }
    }

    /// Create an orchestrator with a pre-built sink and custom master
    /// volume (v10.2.0+ — dragonzen audit P8).
    ///
    /// Testing constructor — see [`new_with`](Self::new_with).
    // night-improve-21: exercised by the core test tree (volume
    // sweep via injected sinks) only; the unified binary's non-test
    // compilation never calls it, hence the allow.
    #[allow(dead_code)]
    pub fn new_with_volume(sink: S, master_volume: f32) -> Self {
        Self {
            sink,
            pool: VoicePool::with_volume(master_volume),
        }
    }

    /// Update the master volume at runtime. The value is clamped to a
    /// sane range — see [`VoicePool::set_master_volume`].
    // night-improve-21: unwired in the unified tree (the hot-reload
    // lane applies params, not volume, today); expect() self-retires
    // when the volume hot-reload lands.
    #[expect(dead_code)]
    #[inline]
    pub fn set_master_volume(&mut self, volume: f32) {
        self.pool.set_master_volume(volume);
    }

    /// Return the current master volume (linear gain multiplier).
    // night-improve-21: unwired in the unified tree; expect() fires the
    // moment a reader appears.
    #[expect(dead_code)]
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

    /// The shared volume factor handle (night-improve-23): a clone of
    /// the voice pool's atomic volume cell, taken BEFORE `run` holds
    /// `&mut self` — the daemonless player's control-plane watcher
    /// stores retuned gains into it live while the render loop runs.
    #[inline]
    pub fn volume_factor(&self) -> Arc<std::sync::atomic::AtomicU32> {
        self.pool.volume_factor()
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
    /// * `event_rx` — Channel receiver yielding [`KeyEvent`]s from
    ///   the input layer.
    /// * `stop_flag` — Shared flag checked each loop iteration; when
    ///   `true`, the loop breaks and the audio device is released.
    pub fn run<M: AcousticModel>(
        &mut self,
        model: &Arc<ArcSwap<M>>,
        event_rx: &Receiver<KeyEvent>,
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

        // v10.2.0 (dragonzen audit S4): heartbeat tracking. Log a
        // debug message after 60 s of input inactivity, upgrade to
        // warn! after 5 min. Without this, if `libinput.dispatch()`
        // returns Ok(()) forever but never produces events (device fd
        // stuck, kernel bug), the daemon appears alive (PID file
        // present, IPC socket listening) but produces no audio. The
        // user has no feedback. The heartbeat makes the failure mode
        // visible in `journalctl --user -u zylaxion`.
        let mut last_event_monotonic = std::time::Instant::now();
        let heartbeat_debug = Duration::from_secs(60);
        let heartbeat_warn = Duration::from_secs(5 * 60);
        let mut heartbeat_debug_emitted = false;
        let mut heartbeat_warn_emitted = false;

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

            // v10.2.0 (S4): heartbeat check. Cheap — one Instant::
            // elapsed() per loop iteration (microseconds).
            let idle = last_event_monotonic.elapsed();
            if !heartbeat_debug_emitted && idle >= heartbeat_debug {
                log::debug!(
                    "no key events in {:.0}s — input thread may be stuck (this is normal during idle periods)",
                    idle.as_secs()
                );
                heartbeat_debug_emitted = true;
            }
            if !heartbeat_warn_emitted && idle >= heartbeat_warn {
                log::warn!(
                    "no key events in {:.0}s — input thread may be stuck. \
                     If the daemon appears alive but produces no audio, \
                     restart it (`systemctl --user restart zylaxion`).",
                    idle.as_secs()
                );
                heartbeat_warn_emitted = true;
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
                    // v10.2.0 (S4): reset the heartbeat on any event.
                    // The input thread is alive and producing events —
                    // the daemon is healthy.
                    last_event_monotonic = std::time::Instant::now();
                    heartbeat_debug_emitted = false;
                    heartbeat_warn_emitted = false;

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
                // drained enough yet. Sleep briefly to let the cpal
                // callback consume some frames instead of busy-looping
                // back to recv_timeout (v10.2.0 — dragonzen audit E3).
                //
                // The previous `continue` fell through to
                // recv_timeout(1ms) which is fine for keyboard-event
                // responsiveness, but during sustained audio (long
                // decay tail of a chord) it caused ~1000 iterations/
                // sec of atomic-load + comparison + branch — preventing
                // CPU deep C-states on battery-powered laptops.
                //
                // Sleep for ~500 µs (≈22 frames at 44.1 kHz) which is
                // short enough to keep the buffer fed without drops,
                // long enough to let the CPU enter C1 or deeper.
                std::thread::sleep(Duration::from_micros(500));
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
    fn handle_input_event<M: AcousticModel>(pool: &mut VoicePool, model: &M, event: &KeyEvent) {
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
                &KeyTrigger {
                    scancode: event.scancode,
                    pressed: true,
                    stereo_position: pan,
                    velocity: None, // standard libinput = digital switch
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
    /// while it still holds a non-zero DC offset from zylaxion's last
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
#[path = "../../test/core/core_tests.rs"]
mod tests;
