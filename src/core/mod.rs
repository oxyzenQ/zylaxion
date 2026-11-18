// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The Brain — orchestrates input → engine → output.
//!
//! [`Orchestrator`] connects the three layers of zylaxion:
//!
//! ```text
//!  input (The Ears)        core (The Brain)       engine (The Voice)      audio (The Mouth)
//!  ───────────             ───────────             ────────────────        ──────────────
//!  KeyEvent ──channel──►  parked select (silent) /
//!                         recv_timeout(1ms) (active)
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

/// If the ring buffer has fewer vacant frames than this threshold,
/// the buffer is considered "mostly full" and the producer yields.
/// 128 frames ≈ 2.9 ms — enough for the ALSA callback to drain a
/// few periods before we re-check.
const SLEEP_THRESHOLD: usize = 128;

/// The render-wait timeout while voices are ACTIVE: the render loop
/// paces the ring buffer against the audio callback on a 1 ms
/// cadence. `recv_timeout` still returns the instant an event lands,
/// so this timeout never delays a keystroke — it bounds only how
/// often the loop re-checks the stop lane while a decay tail is
/// being rendered.
///
/// night-frontier-Z4: this is the ONLY timer the render loop owns,
/// and it arms only while sound is playing — render pacing is real
/// work, reported as such. While SILENT the loop carries no timer at
/// all (see [`Orchestrator::run`]).
const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(1);

/// The control stop command (night-frontier-Z4): the render loop's
/// evented stop lane. The control-plane watcher sends one on every
/// stop decision (the cleared enabled gate, the read-failure verdict,
/// the graceful signal, the process wind-down). The old shape polled
/// an `AtomicBool` each iteration of a timer-waking loop; the Z4 loop
/// has NO timer while idle — a parked thread is asleep in the kernel
/// until a channel op or an fd event lands — so a flag set from
/// another thread could sleep forever. A stop must WAKE the loop:
/// the command is a message, and the message is the wake.
pub struct ControlStop;

// ── Pan mapping ─────────────────────────────────────────────────────────

// ── OrchestratorError ───────────────────────────────────────────────────
//
// night-improve-21: the error contract lives in its own subsystem
// directory now (src/core/error/ — the no-mercy LOC split, a pure
// move); the root re-export keeps every consumer path stable.

mod error;
pub use error::OrchestratorError;

mod fade;

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
/// 2. Call [`Orchestrator::run`] with a channel receiver and an acoustic model.
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
    // with_master_volume; this default-volume constructor stays as the
    // documented no-config entry. night-dragon-lts: the old comment
    // named the zylaxion_live example as its caller — no such example
    // exists in the tree; the constructor is caller-free today, kept
    // as the documented entry point and honestly marked.
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

    // night-hardening-2: the orchestrator-level volume setter pair
    // (set_master_volume / master_volume) is retired. Both sat under
    // #[expect(dead_code)] on the premise that they were waiting for
    // the volume hot-reload lane to land — but that lane shipped
    // through a different door: the player's watcher stores the pool's
    // atomic volume cell directly, via the volume_factor handle and
    // the volume_bits clamp law. An unwired second retune path was
    // maintenance weight pretending to be flexibility; the pool's own
    // cell remains the single volume law.

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
    /// 2. A [`ControlStop`] arrives on `stop_rx` (the control-plane
    ///    watcher's stop decision — the cleared enabled gate, the
    ///    read-failure verdict, a graceful signal, the process
    ///    wind-down).
    /// 3. The render or audio machinery fails.
    ///
    /// When the loop exits, `CpalSink` is dropped, releasing the audio
    /// device and un-stalling the PipeWire graph.
    ///
    /// # The wait law (night-frontier-Z4: the last timer dies)
    ///
    /// The loop has TWO wait regimes, scoped by the pool's own state:
    ///
    /// - **Silent** (no active voices): a blocking select over the
    ///   event channel and the stop lane with NO timeout. The thread
    ///   parks in the kernel until the tap rings or a stop lands —
    ///   zero timer wakeups while silent. The thread wakes the
    ///   instant an event arrives, so keystroke latency is untouched;
    ///   the first press re-arms the 1 ms pacing by the next
    ///   iteration (the pool is active then).
    /// - **Active** (voices decaying): `recv_timeout(1 ms)` — the
    ///   render pacing, real work feeding the Mouth's ring between
    ///   key events. Unchanged from the pre-Z4 loop; the stop lane is
    ///   checked with a cheap `try_recv` each pacing iteration.
    ///
    /// The deep history this law completes: the first loop waited 1 ms
    /// around the clock (1000 futex wakeups per second, about 1.9
    /// percent CPU for a supposedly idle process — the idle-CPU
    /// audit's numbers); night-improve-27's adaptive wait demoted the
    /// silent burn to 100 ms (ten wakeups per second, 0.035 percent);
    /// Z4 demotes it to NOTHING — the park replaces the timer, and
    /// the idle cost is zero events, not a small percent.
    ///
    /// # Hot-reload support
    ///
    /// The `model` parameter is an `Arc<ArcSwap<M>>` — a lock-free
    /// atomic pointer to the acoustic model. The control plane can
    /// swap the pointer at any time (from another thread) by calling
    /// `model.store(Arc::new(new_model))`. The render loop picks up
    /// the new model on its next iteration without blocking.
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
    /// * `stop_rx` — The control stop lane ([`ControlStop`]); a
    ///   message here wakes the parked loop and the audio device is
    ///   released.
    pub fn run<M: AcousticModel>(
        &mut self,
        model: &Arc<ArcSwap<M>>,
        event_rx: &Receiver<KeyEvent>,
        stop_rx: &Receiver<ControlStop>,
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
        // The log lane, not the terminal: the detached player owns no
        // terminal bytes (night-hunter-6) — these surface through
        // RUST_LOG=info in a foreground `zylaxion _play` run.
        let device_rate = self.sink.sample_rate();
        let sr = device_rate as f64;
        log::info!(
            "Interrupt-driven mode — device rate: {device_rate} Hz, \
             render chunk: {MAX_RENDER_CHUNK} frames (~{:.2} ms)",
            MAX_RENDER_CHUNK as f64 / sr * 1000.0,
        );
        log::info!(
            "Ring buffer: 16384 frames (~{:.1} ms), \
             cpal hw buffer: default (PipeWire/ALSA negotiated), \
             pre-fill: {PREFILL_SILENCE_FRAMES} frames (~{:.1} ms)",
            16384.0 / sr * 1000.0,
            PREFILL_SILENCE_FRAMES as f64 / sr * 1000.0,
        );

        // Pre-allocate the largest possible render buffer once —
        // no allocation in the hot loop.  We slice it down to the
        // actual chunk size each iteration.
        let mut batch = [[0.0f32; 2]; MAX_RENDER_CHUNK];

        // night-frontier-Z4: the idle heartbeat retires with the timer
        // it rode. Its 60 s debug / 5 min warning lines could only
        // fire on a timeout wake, and its own comment already carried
        // the honest confession: this seat cannot prove tap liveness
        // (the ring-buffer fd outlives an unpin — the inode lesson the
        // orphan sweep learned). The Z4 loop does not wake while
        // idle, so idle logs cannot exist; the diagnostics that could
        // actually tell "ordinary idle" from "the tap is not
        // delivering" stay where they can prove something — `zylaxion
        // doctor`, `zylaxion status`, and the E2E battery's witnesses.

        // ── The render loop: the parked wait (night-frontier-Z4) ──
        // (The interrupt-driven half of the pipeline is the audio
        // callback itself — hardware clocked. While SILENT this loop
        // parks with no timer at all; while ACTIVE it paces the ring
        // on the 1 ms render cadence.)
        loop {
            // The wait, by regime (night-frontier-Z4). SILENT: a
            // blocking select over [events, stop] with NO timeout —
            // the park IS the idle state, zero timer wakeups; the
            // thread sleeps in the kernel until the first op lands.
            // ACTIVE: the 1 ms render pacing (real work — feeding the
            // Mouth's ring between key events) plus a cheap stop-lane
            // check. Both arms wake the instant their op lands; a
            // simultaneous event+stop pair resolves at the loop top
            // on the next iteration either way.
            if self.pool.is_active() {
                // The ACTIVE regime: `recv_timeout` wakes the instant
                // a key lands, so the pacing never delays a keystroke
                // — the same guarantee the pre-Z4 loop documented.
                match event_rx.recv_timeout(EVENT_POLL_TIMEOUT) {
                    Ok(event) => {
                        // Snapshot the current model ONCE per event
                        // batch. ArcSwap::load() is a single atomic
                        // load — no blocking, no allocation. The Guard
                        // keeps the snapshot alive for the duration of
                        // this batch even if another thread swaps in a
                        // new model mid-batch.
                        let model_guard = model.load();
                        let model_ref: &M = &model_guard;
                        Self::handle_event_batch(&mut self.pool, model_ref, event_rx, event);
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        log::warn!("input channel disconnected — shutting down");
                        self.fade_out_before_drop();
                        return;
                    }
                }
                // The stop lane rides the pacing cadence: one
                // try_recv per 1 ms iteration while sound plays (the
                // flag era's stop check, evented — the watcher's
                // ControlStop message lands within one pacing step).
                if stop_rx.try_recv().is_ok() {
                    log::info!("control stop — shutting down");
                    self.fade_out_before_drop();
                    return;
                }
            } else {
                // The SILENT regime: the park. A dropped event sender
                // (the bridge died) is a ready op and wakes the park;
                // a ControlStop wakes it the same way. This arm is
                // where the old loop's 100 ms idle timeout used to
                // wake the thread ten times a second for nothing.
                crossbeam_channel::select! {
                    recv(event_rx) -> res => match res {
                        Ok(event) => {
                            let model_guard = model.load();
                            let model_ref: &M = &model_guard;
                            Self::handle_event_batch(&mut self.pool, model_ref, event_rx, event);
                        }
                        Err(_) => {
                            log::warn!("input channel disconnected — shutting down");
                            self.fade_out_before_drop();
                            return;
                        }
                    },
                    recv(stop_rx) -> _ => {
                        log::info!("control stop — shutting down");
                        self.fade_out_before_drop();
                        return;
                    }
                }
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

    /// Process one delivered event and drain the co-arriving ones
    /// (e.g. 'k' and 'a' pressed within the same µs window) under one
    /// model snapshot — the batch shape the pre-Z4 loop inlined in
    /// both wait arms, extracted when the wait became two regimes.
    #[inline]
    fn handle_event_batch<M: AcousticModel>(
        pool: &mut VoicePool,
        model: &M,
        event_rx: &Receiver<KeyEvent>,
        first: KeyEvent,
    ) {
        Self::handle_input_event(pool, model, &first);
        while let Ok(event) = event_rx.try_recv() {
            Self::handle_input_event(pool, model, &event);
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
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "../../test/core/core_tests.rs"]
mod tests;
