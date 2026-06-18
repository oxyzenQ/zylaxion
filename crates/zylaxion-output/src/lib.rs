// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real-time audio output via cpal with lock-free ring buffer bridging.
//!
//! This crate connects the [`zactrix_engine::VoicePool`] to the OS audio
//! server (PipeWire / PulseAudio / ALSA) through a zero-lock, zero-allocation
//! audio callback.
//!
//! # Architecture
//!
//! ```text
//!  Main/VoicePool thread          Audio callback thread (real-time)
//!  ───────────────────            ─────────────────────────────────
//!  VoicePool::process_sample()
//!         │
//!         ▼
//!  CpalSink::write_sample()
//!         │
//!         ▼
//!  Producer::try_push  ──►  Ring Buffer  ──►  Consumer::try_pop
//!                                                     │
//!                                                     ▼
//!                                              cpal output buffer
//! ```
//!
//! The [`ringbuf`] SPSC ring buffer is the only shared data structure
//! between the two threads — no Mutex, no blocking.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;

/// Concrete producer type for the heap ring buffer.
type HeapProd = <HeapRb<[f32; 2]> as Split>::Prod;

/// Ring buffer capacity in stereo frames (~370 ms at 44.1 kHz).
///
/// Sized large enough to absorb Linux non-RT scheduler jitter spikes
/// (30–50 ms) without the audio callback ever seeing an empty buffer.
/// The extra latency (~370 ms worst-case) is inaudible for a keyboard
/// sound effect that plays for hundreds of milliseconds anyway.
const RING_BUFFER_FRAMES: usize = 16384;

// ── Error type ─────────────────────────────────────────────────────────

/// Errors that can occur when setting up the audio output.
#[derive(Debug)]
pub enum AudioError {
    /// No audio output device was found on the system.
    NoDeviceAvailable,
    /// Querying the device's default stream configuration failed.
    DefaultStreamConfigError(String),
    /// Building the cpal output stream failed.
    BuildStreamError(String),
    /// Starting the cpal output stream failed.
    PlayStreamError(String),
    /// The device's sample format is not supported (only f32 and i16).
    UnsupportedSampleFormat(cpal::SampleFormat),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDeviceAvailable => write!(f, "no audio output device available"),
            Self::DefaultStreamConfigError(e) => {
                write!(f, "failed to query default stream config: {e}")
            }
            Self::BuildStreamError(e) => write!(f, "failed to build audio stream: {e}"),
            Self::PlayStreamError(e) => write!(f, "failed to start audio stream: {e}"),
            Self::UnsupportedSampleFormat(fmt) => {
                write!(f, "unsupported sample format: {fmt:?}")
            }
        }
    }
}

impl std::error::Error for AudioError {}

// ── AudioSink trait ────────────────────────────────────────────────────

/// Trait for pushing stereo samples into an audio output.
///
/// All methods take `&mut self` because the producer side of an SPSC ring
/// buffer is single-threaded by design.
pub trait AudioSink {
    /// Push a single interleaved stereo sample `[left, right]`.
    ///
    /// If the internal buffer is full, the sample is silently dropped.
    fn write_sample(&mut self, sample: [f32; 2]);

    /// Push a batch of stereo samples.
    fn write_batch(&mut self, samples: &[[f32; 2]]) {
        for &sample in samples {
            self.write_sample(sample);
        }
    }
}

// ── CpalSink ───────────────────────────────────────────────────────────

/// Lock-free audio output backed by cpal and a ring buffer.
///
/// # Lifecycle
///
/// 1. [`CpalSink::new`] creates the ring buffer, opens the default audio
///    device, builds the cpal stream (which captures the consumer), and
///    starts playback.
/// 2. The caller owns the `CpalSink` and its producer. Call
///    [`write_sample`](AudioSink::write_sample) or
///    [`write_batch`](AudioSink::write_batch) to feed audio.
/// 3. Dropping the `CpalSink` stops the cpal stream and silences output.
pub struct CpalSink {
    producer: HeapProd,
    /// The active cpal output stream. Kept alive to prevent the audio
    /// callback from being dropped (which would silence output).
    _stream: cpal::Stream,
    sample_rate: u32,
    /// Set to `true` by the cpal error callback when the audio device
    /// disconnects or the stream encounters an unrecoverable error.
    /// When `true`, the audio callback outputs silence instead of
    /// reading from the ring buffer — preventing a crash and keeping
    /// the daemon alive. The daemon remains running so the user can
    /// `zylaxion stop` gracefully or re-plug the device.
    _paused: Arc<AtomicBool>,
}

impl CpalSink {
    /// Create a new audio output, open the default device, and start
    /// the real-time audio callback.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError`] if no device is available, the config cannot
    /// be queried, or the stream cannot be built/started.
    pub fn new() -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioError::NoDeviceAvailable)?;

        let default_config = device
            .default_output_config()
            .map_err(|e| AudioError::DefaultStreamConfigError(e.to_string()))?;

        // ── Sample rate negotiation ────────────────────────────────
        // cpal's ALSA backend often reports 44100 as the "default"
        // even when PipeWire is configured system-wide for 48000.
        // This causes a resampling layer in PipeWire (visible as a
        // non-power-of-2 quantum in `pw-top`). To avoid that, we
        // iterate the device's supported configs and prefer 48000 if
        // available — this matches the most common PipeWire/PulseAudio
        // default and eliminates the resampler.
        let preferred_rates = [48000_u32, 96000, 192000];
        let default_rate = default_config.sample_rate().0;

        let chosen_rate = device
            .supported_output_configs()
            .ok()
            .and_then(|configs| {
                // Find the first preferred rate that the device supports.
                // supported_output_configs() returns an iterator of
                // SupportedStreamConfigRange, each covering a range of
                // sample rates. We check if our preferred rate falls
                // within any range.
                let collected: Vec<_> = configs.collect();
                preferred_rates
                    .iter()
                    .find(|&&preferred| {
                        collected.iter().any(|c| {
                            c.min_sample_rate().0 <= preferred && c.max_sample_rate().0 >= preferred
                        })
                    })
                    .copied()
            })
            .unwrap_or(default_rate)
            .max(44100); // Never go below 44100 — DSP math breaks.

        log::info!("negotiated sample rate: {} Hz", chosen_rate);

        // Extract channels and sample format BEFORE consuming
        // default_config via .into() below.
        let channels = default_config.channels() as usize;
        let sample_format = default_config.sample_format();

        // Build a StreamConfig with the chosen sample rate, preserving
        // the default channels and buffer size from the device.
        let mut stream_config: cpal::StreamConfig = default_config.into();
        stream_config.sample_rate = cpal::SampleRate(chosen_rate);

        let sample_rate = chosen_rate;

        let rb = HeapRb::<[f32; 2]>::new(RING_BUFFER_FRAMES);
        let (producer, consumer) = rb.split();

        // Shared "paused" flag — set to true by the error callback when
        // the audio device disconnects. The audio callback checks this
        // flag and outputs silence when true, preventing a crash.
        let paused = Arc::new(AtomicBool::new(false));
        let paused_for_err = Arc::clone(&paused);
        let err_fn = move |err: cpal::StreamError| {
            eprintln!("zylaxion: warning: Audio stream error: {err}");
            eprintln!("zylaxion: warning: Audio device disconnected. Pausing output.");
            paused_for_err.store(true, Ordering::Relaxed);
        };

        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let mut cons = consumer;
                let paused_cb = Arc::clone(&paused);
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                            // If the device disconnected, output silence
                            // and skip ring-buffer reads. This keeps the
                            // callback alive without crashing.
                            if paused_cb.load(Ordering::Relaxed) {
                                for s in data.iter_mut() {
                                    *s = 0.0;
                                }
                                return;
                            }
                            for frame in data.chunks_exact_mut(channels) {
                                match cons.try_pop() {
                                    Some([l, r]) => {
                                        // Defense-in-depth: replace
                                        // NaN/Infinity with 0.0 before
                                        // handing to ALSA. The
                                        // VoicePool already guards
                                        // against this, but a second
                                        // check here is cheap insurance.
                                        frame[0] = if l.is_finite() {
                                            l.clamp(-1.0, 1.0)
                                        } else {
                                            0.0
                                        };
                                        if channels > 1 {
                                            frame[1] = if r.is_finite() {
                                                r.clamp(-1.0, 1.0)
                                            } else {
                                                0.0
                                            };
                                        }
                                    }
                                    None => {
                                        for s in frame.iter_mut() {
                                            *s = 0.0;
                                        }
                                    }
                                }
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| AudioError::BuildStreamError(e.to_string()))?
            }
            cpal::SampleFormat::I16 => {
                let mut cons = consumer;
                let paused_cb = Arc::clone(&paused);
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                            if paused_cb.load(Ordering::Relaxed) {
                                for s in data.iter_mut() {
                                    *s = 0;
                                }
                                return;
                            }
                            for frame in data.chunks_exact_mut(channels) {
                                match cons.try_pop() {
                                    Some([l, r]) => {
                                        frame[0] = if l.is_finite() {
                                            (l.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
                                        } else {
                                            0
                                        };
                                        if channels > 1 {
                                            frame[1] = if r.is_finite() {
                                                (r.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
                                            } else {
                                                0
                                            };
                                        }
                                    }
                                    None => {
                                        for s in frame.iter_mut() {
                                            *s = 0;
                                        }
                                    }
                                }
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| AudioError::BuildStreamError(e.to_string()))?
            }
            other => return Err(AudioError::UnsupportedSampleFormat(other)),
        };

        stream
            .play()
            .map_err(|e| AudioError::PlayStreamError(e.to_string()))?;

        Ok(Self {
            producer,
            _stream: stream,
            sample_rate,
            _paused: paused,
        })
    }

    /// Actual sample rate reported by the audio device.
    #[inline]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Number of stereo frames that can be pushed before the ring buffer
    /// is full and samples start being dropped.
    #[inline]
    pub fn producer_vacancy(&self) -> usize {
        self.producer.vacant_len()
    }
}

impl AudioSink for CpalSink {
    #[inline]
    fn write_sample(&mut self, sample: [f32; 2]) {
        let _ = self.producer.try_push(sample);
    }

    #[inline]
    fn write_batch(&mut self, samples: &[[f32; 2]]) {
        for &sample in samples {
            let _ = self.producer.try_push(sample);
        }
    }
}
