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

        let supported = device
            .default_output_config()
            .map_err(|e| AudioError::DefaultStreamConfigError(e.to_string()))?;

        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels() as usize;
        let sample_format = supported.sample_format();

        let rb = HeapRb::<[f32; 2]>::new(RING_BUFFER_FRAMES);
        let (producer, consumer) = rb.split();

        let stream_config: cpal::StreamConfig = supported.into();
        // Let cpal / PipeWire / ALSA negotiate the optimal hardware
        // fragment size.  Forcing a small Fixed buffer causes frequent
        // xruns on non-RT kernels; the Default lets the audio server
        // pick its preferred period size (typically 1024–4096 frames).
        let err_fn = |err: cpal::StreamError| eprintln!("[zylaxion-output] stream error: {err}");

        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let mut cons = consumer;
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                            for frame in data.chunks_exact_mut(channels) {
                                match cons.try_pop() {
                                    Some([l, r]) => {
                                        frame[0] = l;
                                        if channels > 1 {
                                            frame[1] = r;
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
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                            for frame in data.chunks_exact_mut(channels) {
                                match cons.try_pop() {
                                    Some([l, r]) => {
                                        frame[0] = (l.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                                        if channels > 1 {
                                            frame[1] =
                                                (r.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
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
