// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The shutdown fade-out lane — the silence pad flushed into the ring
//! buffer before `CpalSink` drops, and the deadline that keeps it from
//! hanging the stop verb on a dead device.
//!
//! A pure move out of the orchestrator root (night-dragon-maintenance-6,
//! the no-mercy LOC cap): the method is the last thing `run` does on
//! both exit paths and owns the only two shutdown constants, so the
//! lane lives alone.

use std::time::Duration;

use super::Orchestrator;
use crate::audio::AudioSink;

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

/// Hard ceiling on the total wall time `fade_out_before_drop` may spend
/// waiting for the ring buffer to drain.
///
/// The healthy path needs one pad length (~23 ms at 44.1 kHz) plus
/// scheduler slack; the ceiling is ~10x that. The pathological path is
/// real: when the audio device disconnects, the cpal error callback
/// flips the paused flag and the audio callback stops popping the ring
/// buffer entirely — `producer_vacancy()` then reads 0 forever. Without
/// the ceiling, `zylaxion off` (and the channel-disconnected exit)
/// spins in the fade-out sleep loop for the rest of the process's life:
/// a hung shutdown is a worse outcome than a skipped silence pad on a
/// device that is not producing sound anyway.
const FADEOUT_MAX_WAIT: Duration = Duration::from_millis(300);

impl<S: AudioSink> Orchestrator<S> {
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
    /// Called from both exit paths of [`run`](super::Orchestrator::run):
    /// the `stop_flag` path (IPC `stop` command / SIGTERM) and the
    /// input-channel `Disconnected` path (input source thread died).
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
    ///
    /// # The deadline (night-dragon-maintenance-6)
    ///
    /// The retry loop is bounded by [`FADEOUT_MAX_WAIT`]. On a
    /// disconnected device the cpal error callback pauses output and
    /// the audio callback stops popping the ring buffer — vacancy
    /// reads 0 forever, and an unbounded retry here would hang the
    /// stop path (`zylaxion off`, SIGTERM, bridge death) for the rest
    /// of the process's life. Past the deadline the fade-out gives up
    /// and the drop proceeds: the missed pad can only produce a pop
    /// artifact on a device that no longer plays anything, while the
    /// hang denied the user their stop verb.
    pub(super) fn fade_out_before_drop(&mut self) {
        let silence = [0.0f32; 2];
        let deadline = std::time::Instant::now() + FADEOUT_MAX_WAIT;
        let mut written = 0usize;
        while written < FADEOUT_SILENCE_FRAMES {
            if std::time::Instant::now() >= deadline {
                log::warn!(
                    "fade-out deadline reached after {} of {} silence frames — \
                     the audio callback is not draining (paused or disconnected \
                     device); dropping the stream without the full pad",
                    written,
                    FADEOUT_SILENCE_FRAMES,
                );
                break;
            }
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
        // duration is the pad's own duration plus a small margin for
        // scheduler jitter — proportional to what was actually written,
        // so a wedged stream that got zero frames in waits zero. Without
        // this, dropping the sink immediately after writing the silence
        // can leave it half-consumed in the ring buffer, and PipeWire's
        // drain behavior on stream teardown is implementation-defined.
        if written > 0 {
            let drain_ms =
                (written as f64 / self.sink.sample_rate() as f64 * 1000.0).ceil() as u64 + 5; // 5 ms jitter margin
            std::thread::sleep(Duration::from_millis(drain_ms));
        }
    }
}
