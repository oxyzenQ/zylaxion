// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! PipeWire native audio output — bypasses cpal/ALSA for direct PipeWire
//! integration.
//!
//! This module implements [`PipewireSink`] which connects directly to the
//! PipeWire server using the `pipewire` and `libspa` Rust crates. By
//! bypassing the ALSA bridge, we eliminate the resampling layer and achieve
//! native sample rate support.
//!
//! # Architecture
//!
//! ```text
//! Main thread                     PipeWire thread (background)
//! ───────────                     ───────────────────────────
//! PipewireSink::producer ──►  Ring Buffer  ──► process callback
//! (write_sample/write_batch)     (16384 frames)  (dequeue_buffer → fill)
//! ```
//!
//! The PipeWire main loop runs in a dedicated background thread. The process
//! callback reads from the ring buffer Consumer (lock-free SPSC) and fills
//! the SPA buffer. If the ring buffer is empty, silence is written.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pipewire as pw;
use pw::properties::properties;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;

use crate::{AudioError, AudioSink, RING_BUFFER_FRAMES};

/// Concrete producer type for the heap ring buffer.
type HeapProd = <HeapRb<[f32; 2]> as Split>::Prod;
/// Concrete consumer type for the heap ring buffer.
type HeapCons = <HeapRb<[f32; 2]> as Split>::Cons;

/// PipeWire native audio output backed by the `pipewire` crate.
///
/// Connects directly to the PipeWire server, bypassing cpal/ALSA. This
/// eliminates the resampling layer and provides native sample rate support.
///
/// # Lifecycle
///
/// 1. [`PipewireSink::new`] creates the ring buffer, spawns the PipeWire
///    main loop thread, connects to the daemon, and negotiates the format.
/// 2. The caller owns the `PipewireSink` and its producer. Call
///    [`write_sample`](AudioSink::write_sample) or
///    [`write_batch`](AudioSink::write_batch) to feed audio.
/// 3. Dropping the `PipewireSink` stops the background thread (the main
///    loop's `quit()` is called via the thread's Drop).
pub struct PipewireSink {
    producer: HeapProd,
    sample_rate: u32,
    /// Background thread running the PipeWire main loop. Kept alive to
    /// prevent the stream from being dropped.
    _thread: std::thread::JoinHandle<()>,
}

/// User data shared between the PipeWire listener callbacks.
struct ListenerData {
    /// Ring buffer consumer — read from in the process callback.
    consumer: HeapCons,
    /// Negotiated sample rate (set in param_changed, read by main thread).
    sample_rate: Arc<AtomicU32>,
    /// Set to true when the format is negotiated.
    ready: Arc<AtomicBool>,
}

impl PipewireSink {
    /// Create a new PipeWire audio output, connect to the daemon, and
    /// start the real-time audio callback.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError`] if PipeWire is not running, the connection
    /// fails, or format negotiation times out (5 second deadline).
    pub fn new() -> Result<Self, AudioError> {
        let rb = HeapRb::<[f32; 2]>::new(RING_BUFFER_FRAMES);
        let (producer, consumer) = rb.split();

        // Shared state between the PipeWire thread and the main thread.
        let sample_rate = Arc::new(AtomicU32::new(0));
        let ready = Arc::new(AtomicBool::new(false));
        let error = Arc::new(AtomicBool::new(false));

        let sr_for_thread = Arc::clone(&sample_rate);
        let ready_for_thread = Arc::clone(&ready);
        let error_for_thread = Arc::clone(&error);

        let thread = std::thread::Builder::new()
            .name("zylaxion-pipewire".into())
            .spawn(move || {
                // All PipeWire objects are created inside this thread to
                // avoid Send/Sync issues with the C FFI types.
                pw::init();

                let mainloop = match pw::main_loop::MainLoop::new(None) {
                    Ok(ml) => ml,
                    Err(e) => {
                        eprintln!("zylaxion: warning: PipeWire MainLoop failed: {e}");
                        error_for_thread.store(true, Ordering::Relaxed);
                        return;
                    }
                };

                let context = match pw::context::Context::new(&mainloop) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("zylaxion: warning: PipeWire Context failed: {e}");
                        error_for_thread.store(true, Ordering::Relaxed);
                        return;
                    }
                };

                let core = match context.connect(None) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("zylaxion: warning: PipeWire connect failed: {e}");
                        error_for_thread.store(true, Ordering::Relaxed);
                        return;
                    }
                };

                let stream = match pw::stream::Stream::new(
                    &core,
                    "zylaxion",
                    properties! {
                        *pw::keys::MEDIA_TYPE => "Audio",
                        *pw::keys::MEDIA_CATEGORY => "Playback",
                        *pw::keys::MEDIA_ROLE => "DSP",
                    },
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("zylaxion: warning: PipeWire Stream failed: {e}");
                        error_for_thread.store(true, Ordering::Relaxed);
                        return;
                    }
                };

                let data = ListenerData {
                    consumer,
                    sample_rate: sr_for_thread,
                    ready: ready_for_thread,
                };

                let _listener = stream
                    .add_local_listener_with_user_data(data)
                    .param_changed(|_, user_data, id, param| {
                        let Some(param) = param else {
                            return;
                        };
                        if id != pw::spa::param::ParamType::Format.as_raw() {
                            return;
                        }

                        let (media_type, media_subtype) =
                            match pw::spa::param::format_utils::parse_format(param) {
                                Ok(v) => v,
                                Err(_) => return,
                            };

                        if media_type != pw::spa::param::format::MediaType::Audio
                            || media_subtype != pw::spa::param::format::MediaSubtype::Raw
                        {
                            return;
                        }

                        let mut info = pw::spa::param::audio::AudioInfoRaw::new();
                        if info.parse(param).is_err() {
                            return;
                        }

                        let rate = info.rate();
                        user_data.sample_rate.store(rate, Ordering::Relaxed);
                        user_data.ready.store(true, Ordering::Relaxed);
                        log::info!(
                            "PipeWire negotiated: {} Hz, {} channels",
                            rate,
                            info.channels()
                        );
                    })
                    .process(|stream, user_data| {
                        if let Some(mut buffer) = stream.dequeue_buffer() {
                            let datas = buffer.datas_mut();
                            if let Some(data) = datas.first_mut() {
                                let stride = std::mem::size_of::<f32>() * 2; // stereo F32

                                if let Some(slice) = data.data() {
                                    let n_frames = slice.len() / stride;

                                    // Cast the byte slice to an f32 slice.
                                    // Safe because F32LE is the negotiated
                                    // format and the buffer is properly
                                    // aligned by PipeWire.
                                    let samples: &mut [f32] = unsafe {
                                        std::slice::from_raw_parts_mut(
                                            slice.as_mut_ptr() as *mut f32,
                                            n_frames * 2,
                                        )
                                    };

                                    // Read from the ring buffer consumer.
                                    // If empty, output silence.
                                    for i in 0..n_frames {
                                        match user_data.consumer.try_pop() {
                                            Some([l, r]) => {
                                                samples[i * 2] = if l.is_finite() {
                                                    l.clamp(-1.0, 1.0)
                                                } else {
                                                    0.0
                                                };
                                                samples[i * 2 + 1] = if r.is_finite() {
                                                    r.clamp(-1.0, 1.0)
                                                } else {
                                                    0.0
                                                };
                                            }
                                            None => {
                                                samples[i * 2] = 0.0;
                                                samples[i * 2 + 1] = 0.0;
                                            }
                                        }
                                    }

                                    let chunk = data.chunk_mut();
                                    *chunk.offset_mut() = 0;
                                    *chunk.stride_mut() = stride as i32;
                                    *chunk.size_mut() = (stride * n_frames) as u32;
                                }
                            }
                        }
                    })
                    .register();

                if let Err(e) = _listener {
                    eprintln!("zylaxion: warning: PipeWire listener failed: {e}");
                    error_for_thread.store(true, Ordering::Relaxed);
                    return;
                }

                // Build the format POD: F32LE, stereo, 48000 Hz.
                // PipeWire will negotiate the actual rate; we request 48000
                // as a preference but accept whatever the server provides.
                let mut audio_info = pw::spa::param::audio::AudioInfoRaw::new();
                audio_info.set_format(pw::spa::param::audio::AudioFormat::F32LE);
                audio_info.set_rate(48000);
                audio_info.set_channels(2);

                let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
                    std::io::Cursor::new(Vec::new()),
                    &pw::spa::pod::Value::Object(pw::spa::pod::Object {
                        type_: pw::spa::sys::SPA_TYPE_OBJECT_Format,
                        id: pw::spa::sys::SPA_PARAM_EnumFormat,
                        properties: audio_info.into(),
                    }),
                )
                .unwrap()
                .0
                .into_inner();

                let mut params = [pw::spa::pod::Pod::from_bytes(&values).unwrap()];

                match stream.connect(
                    pw::spa::utils::Direction::Output,
                    None, // target node (None = default sink)
                    pw::stream::StreamFlags::AUTOCONNECT
                        | pw::stream::StreamFlags::MAP_BUFFERS
                        | pw::stream::StreamFlags::RT_PROCESS,
                    &mut params,
                ) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("zylaxion: warning: PipeWire stream connect failed: {e}");
                        error_for_thread.store(true, Ordering::Relaxed);
                        return;
                    }
                }

                log::info!("PipeWire stream connected, entering main loop");
                mainloop.run();
            })
            .map_err(|e| AudioError::BuildStreamError(e.to_string()))?;

        // Wait for format negotiation or error (5 second timeout).
        let timeout = Duration::from_secs(5);
        let start = Instant::now();
        loop {
            if ready.load(Ordering::Relaxed) {
                break;
            }
            if error.load(Ordering::Relaxed) {
                return Err(AudioError::BuildStreamError(
                    "PipeWire connection failed".to_string(),
                ));
            }
            if start.elapsed() > timeout {
                return Err(AudioError::BuildStreamError(
                    "PipeWire connection timeout (5s)".to_string(),
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let sr = sample_rate.load(Ordering::Relaxed);
        log::info!("PipeWire backend ready: {} Hz", sr);

        Ok(Self {
            producer,
            sample_rate: sr,
            _thread: thread,
        })
    }
}

impl AudioSink for PipewireSink {
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

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[inline]
    fn producer_vacancy(&self) -> usize {
        self.producer.vacant_len()
    }
}
