// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! PROOF 6 — resource footprint (night-private-research-2xblade-3.5).
//!
//! The instrument runs the REAL production audio stack —
//! [`crate::audio::CpalSink`] + the [`crate::core::Orchestrator`] run
//! loop + the real `VoicePool` — inside this process, paced in real
//! time, and samples its own `/proc` footprint at the prompt's
//! anchors. The measured process carries the player's exact thread
//! topology: the orchestrator render loop (main), the event source
//! (the bridge stand-in), the control watcher, and the cpal audio
//! callback thread.
//!
//! The sandbox substitutions, named once and shipped everywhere:
//!
//! - The pinned control map cannot exist here (no privileges to
//!   attach the tap). The watcher reads an `AtomicU32` control cell
//!   on the player's 100 ms cadence; the `enabled=0` stop write that
//!   the CLI's `off` performs is performed by the measurement window
//!   timer. The `bpf()` syscalls themselves (10-20 per second,
//!   microseconds each) are the named residue this instrument
//!   cannot measure.
//! - The events ring buffer cannot exist here either. The event
//!   source paces keystrokes on the wall clock (STATE B) or sends
//!   nothing (STATE C); its 100 ms cadence mirrors the bridge's
//!   poll timeout, and the blocking sleep stands in for the
//!   blocking poll.
//! - There is no audio device. The FIRST instrument run used the
//!   ALSA null PCM for every state — and its own evidence refuted
//!   it: the null device is POLLOUT-always-ready, so the cpal
//!   callback thread (`cpal_alsa_out`) spun at ~75-100% of one core
//!   in EVERY state including idle, while the orchestrator loop
//!   itself cost 8 ticks per 10 s (0.8%). The spin is the null
//!   device's semantics, not the player's. STATE B and STATE C
//!   therefore run the same production Orchestrator loop against
//!   [`PacedNullSink`]: the real ring, the real back-pressure, the
//!   real render path — with the device's callback thread replaced
//!   by a wall-clock drain at the desktop's typical quantum
//!   (1024 frames at 48 kHz, ~21.3 ms). The cpal callback's own
//!   per-frame cost (try_pop + is_finite + clamp, ~47-93 firings/s
//!   on a real device) is the named excluded residue, already
//!   bounded by PROOF 3's audit. Cold-start keeps the REAL null-PCM
//!   open path — the open is the player's own work, and the null
//!   device's fast open is named.
//!
//! Three modes (dispatched from main.rs):
//!
//! - `--footprint-state-b` — sound on, 5 keystrokes/s for 60 s;
//!   `/proc` samples at t = 5, 30, 60.
//! - `--footprint-state-c` — sound on, zero keystrokes for 60 s
//!   (the state the README's zero-idle-cost claim refers to);
//!   samples at t = 0, 60.
//! - `--footprint-cold` — the cold-start anchors: process start →
//!   device open → first sample pushed → first rendered sample
//!   pushed.

use crate::audio::{AudioSink, CpalSink};
use crate::core::{KeyEvent, Orchestrator};
use crate::engine::VoicePool;
use crate::profiles::{DEFAULT_PRESET, MechanicalClick, registry_lookup};
use crate::proof::rss_kib;
use arc_swap::ArcSwap;
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The measurement window, seconds (the prompt locks 60 for both
/// states).
const WINDOW_S: u64 = 60;

/// The watcher's cadence — the player's own CONTROL_POLL_MS
/// (src/commands/player.rs): worst-case stop latency, ~10 Hz of tiny
/// control reads.
const CONTROL_POLL_MS: u64 = 100;

/// Keystroke cadence for STATE B: 5 keys/s (the prompt's number).
const KEY_INTERVAL_MS: u64 = 200;

/// Press-to-release hold for STATE B (a real typing tap, not a
/// keydown-only stream; the orchestrator triggers on press and
/// releases on release — both events ride the measured path).
const HOLD_MS: u64 = 80;

/// The keystroke scancode round-robin (KEY_A..KEY_H, evdev codes —
/// the same physical-key shape reload.rs drives; the codes spread
/// the pan field across the stereo field).
const SCANCODES: [u32; 8] = [30, 31, 32, 33, 34, 35, 36, 37];

/// The orchestrator's pre-fill frame count — mirrored locally from
/// core::PREFILL_SILENCE_FRAMES (2048) because the cold-start mode
/// replicates run()'s prelude exactly and the main crate's constants
/// are private. A sourced mirror, not a re-definition of behavior.
const PREFILL_FRAMES: usize = 2048;

/// The orchestrator's render chunk — the mirror of
/// core::MAX_RENDER_CHUNK (64), same sourcing rule as above.
const COLD_CHUNK: usize = 64;

/// STATE B's /proc sample anchors, seconds from window start.
const STATE_B_SAMPLES: [u64; 3] = [5, 30, 60];

/// STATE C's /proc sample anchors: t = 0 and t = 60.
const STATE_C_SAMPLES: [u64; 2] = [0, 60];

// ── The process-start anchor (the cold-start clock) ──────────────────

/// Stamped by main() before anything else, so the cold-start delta
/// begins at the first statement of the process. The epoch twin lets
/// the wrapper compute the exec+dispatch overhead from its own spawn
/// stamp.
pub struct T0Anchor {
    /// Monotonic anchor for the internal deltas.
    pub at: Instant,
    /// Wall-clock anchor (µs since the epoch) for the wrapper's
    /// external spawn delta.
    pub epoch_us: u128,
}

static T0: std::sync::OnceLock<T0Anchor> = std::sync::OnceLock::new();

/// Stamp the process-start anchor. Called first in main(); the first
/// caller wins (OnceLock), every mode pays one Instant + one
/// SystemTime call for it.
pub fn stamp_t0() {
    let _ = T0.set(T0Anchor {
        at: Instant::now(),
        epoch_us: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0),
    });
}

fn t0() -> &'static T0Anchor {
    T0.get().expect("T0 anchor must be stamped in main()")
}

// ── /proc readers (the prompt's instrument, read from inside) ────────

/// utime + stime in scheduler ticks from /proc/self/stat — the CPU
/// half of the footprint evidence. The wrapper recomputes the same
/// files from outside for the cross-check.
fn cpu_ticks() -> u64 {
    let raw = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // comm may contain spaces and parentheses; everything after the
    // LAST ')' is positional from state onward (field 3). utime is
    // field 14, stime field 15 (1-based) → offsets 11 and 12 here.
    let after = match raw.rfind(')') {
        Some(i) => &raw[i + 1..],
        None => return 0,
    };
    let fields: Vec<&str> = after.split_whitespace().collect();
    let utime = fields
        .get(11)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let stime = fields
        .get(12)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    utime + stime
}

/// One measurement line — the exact anchor shape the wrapper parses:
/// `FOOTPRINT <tag> t=<s> ticks=<u+s> rss_kib=<n>`.
fn sample(tag: &str, start: Instant) {
    println!(
        "FOOTPRINT {tag} t={:.3} ticks={} rss_kib={}",
        start.elapsed().as_secs_f32(),
        cpu_ticks(),
        rss_kib(),
    );
}

/// CLOCK_MONOTONIC microseconds — the tap's timestamp contract
/// (bpf_ktime_get_ns / 1000; the same clock and unit the player
/// receives on the KeyEvent).
fn monotonic_us() -> u64 {
    // Instant has no epoch, so the timestamp rides SystemTime's
    // monotonic-enough boot clock for the measured path; the
    // orchestrator consumes it for the inter-keystroke interval law
    // only, and STATE B's pacing is wall-clock regardless.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

// ── The paced null sink (the instrument fix) ────────────────────────

/// The ring capacity — the mirror of audio::RING_BUFFER_FRAMES
/// (16384, private in the main crate), sourced like the pre-fill
/// mirror above.
const RING_FRAMES: usize = 16_384;

/// The drain clock: 48 kHz at the desktop's typical 1024-frame
/// quantum (~21.3 ms per period) — the consumption pattern a real
/// output device presents to the orchestrator's ring. PROOF 1 and
/// PROOF 5 run the same rate.
const DRAIN_RATE: u32 = 48_000;
const DRAIN_PERIOD: usize = 1024;

type HeapProd = <HeapRb<[f32; 2]> as Split>::Prod;
type HeapCons = <HeapRb<[f32; 2]> as Split>::Cons;

/// The device stand-in for STATE B and STATE C: the same 16384-frame
/// stereo ring `CpalSink` builds, drained on the wall clock at the
/// desktop's period cadence. The orchestrator's loop, back-pressure
/// (`vacancy < 128` → the 500 µs sleep), render path, and pool are
/// the production code, unchanged; only the callback thread is
/// replaced (see the module doc for the measured artifact that
/// forced this substitution).
struct PacedNullSink {
    producer: HeapProd,
    sample_rate: u32,
    stop: Arc<AtomicBool>,
    _drain: std::thread::JoinHandle<()>,
}

impl PacedNullSink {
    fn new() -> Self {
        let rb = HeapRb::<[f32; 2]>::new(RING_FRAMES);
        let (producer, mut consumer) = rb.split();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_drain = Arc::clone(&stop);
        let period = Duration::from_nanos((DRAIN_PERIOD as f64 / DRAIN_RATE as f64 * 1e9) as u64);
        let drain = std::thread::Builder::new()
            .name("footprint-drain".to_string())
            .spawn(move || {
                let mut next = Instant::now() + period;
                loop {
                    if stop_drain.load(Ordering::Relaxed) {
                        return;
                    }
                    let now = Instant::now();
                    if next > now {
                        std::thread::sleep(next - now);
                    } else {
                        // Fell behind (scheduler hiccup): resync.
                        next = now + period;
                    }
                    for _ in 0..DRAIN_PERIOD {
                        // The device consumes; an empty ring is the
                        // callback's ring-empty silence, not an error.
                        let _ = consumer.try_pop();
                    }
                    next += period;
                }
            })
            .expect("drain thread");
        Self {
            producer,
            sample_rate: DRAIN_RATE,
            stop,
            _drain: drain,
        }
    }
}

impl Drop for PacedNullSink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl AudioSink for PacedNullSink {
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
    fn producer_vacancy(&self) -> usize {
        self.producer.vacant_len()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

// ── Shared setup (the player's exact constructor sequence) ───────────

/// Build the player's audio stack at the player's default posture:
/// preset 0 (the registry default), volume 100% → the 5.5x laptop
/// default gain. Generic over the sink so the states run the paced
/// stand-in while cold-start runs the real null-PCM open. Returns
/// the orchestrator, the model cell, and the volume factor handle —
/// in the player's own order (the handle is taken before run()
/// takes `&mut self`).
fn build_player<S: AudioSink>(sink: S) -> (Orchestrator<S>, Arc<ArcSwap<MechanicalClick>>) {
    let gain = VoicePool::DEFAULT_MASTER_VOLUME; // 100% → 5.5x
    // The testing constructor: same pool posture as the player's
    // with_master_volume, with the injected sink.
    let orchestrator = Orchestrator::new_with_volume(sink, gain);
    let sample_rate = orchestrator.sample_rate();
    // The player's log line, same shape (src/commands/player.rs).
    println!("[footprint] audio device sample rate: {sample_rate} Hz");
    let profile = registry_lookup(DEFAULT_PRESET)
        .map(|e| e.profile)
        .expect("the compiled default preset must exist");
    let model = Arc::new(ArcSwap::from_pointee(MechanicalClick::with_profile(
        profile,
        sample_rate,
    )));
    (orchestrator, model)
}

// ── STATE B / STATE C ────────────────────────────────────────────────

/// Run one 60-second state and print the /proc sample lines. `with_keys`
/// selects the state: STATE B (5 keys/s, sound on) or STATE C (zero
/// keystrokes, sound on — the zero-idle-cost claim's state).
pub fn run_state(with_keys: bool) {
    let tag = if with_keys { "state-b" } else { "state-c" };
    let window_start = Instant::now();
    println!(
        "[footprint] mode={tag} pid={} window={WINDOW_S}s keys_per_s={}",
        std::process::id(),
        if with_keys { 5 } else { 0 },
    );
    // The sampler runs first so STATE C's t=0 sample lands before the
    // device opens — the window is the whole process lifetime.
    let sampler_start = window_start;
    let sampler = {
        let anchors: Vec<u64> = if with_keys {
            STATE_B_SAMPLES.to_vec()
        } else {
            STATE_C_SAMPLES.to_vec()
        };
        std::thread::Builder::new()
            .name("footprint-sample".to_string())
            .spawn(move || {
                for t in anchors {
                    let due = sampler_start + Duration::from_secs(t);
                    if let Some(wait) = due.checked_duration_since(Instant::now()) {
                        std::thread::sleep(wait);
                    }
                    sample(tag, sampler_start);
                }
            })
            .expect("sampler thread")
    };

    // The watcher's control cell: enabled=1 (the `on` verb's write,
    // mirrored). The 60s force-clear is the measurement window's
    // stand-in for the CLI's `off` write — named in the module doc.
    let control_enabled = Arc::new(AtomicU32::new(1));
    let stop_flag = Arc::new(AtomicBool::new(false));

    // The control watcher: the player's exact cadence and exit rule
    // (enabled ≠ 1 → stop), plus the window force-clear.
    let watcher = {
        let control_enabled = Arc::clone(&control_enabled);
        let stop_flag = Arc::clone(&stop_flag);
        std::thread::Builder::new()
            .name("footprint-watch".to_string())
            .spawn(move || {
                let started = Instant::now();
                loop {
                    std::thread::sleep(Duration::from_millis(CONTROL_POLL_MS));
                    if control_enabled.load(Ordering::Relaxed) != 1
                        || started.elapsed() >= Duration::from_secs(WINDOW_S)
                    {
                        stop_flag.store(true, Ordering::Relaxed);
                        return;
                    }
                }
            })
            .expect("watcher thread")
    };

    // The event source: the bridge stand-in. STATE B paces 5 keys/s
    // (press + release 80 ms later, scancodes round-robin); STATE C
    // sends nothing for the whole window. Both ride the 100 ms
    // blocking cadence the bridge's poll timeout gives the player.
    let (tx, rx) = crossbeam_channel::unbounded::<KeyEvent>();
    let source = {
        let stop_flag = Arc::clone(&stop_flag);
        std::thread::Builder::new()
            .name("footprint-source".to_string())
            .spawn(move || {
                let started = Instant::now();
                let window = Duration::from_secs(WINDOW_S);
                if !with_keys {
                    // STATE C: the bridge's poll stands in as a sleep.
                    while !stop_flag.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(CONTROL_POLL_MS));
                    }
                    return;
                }
                let mut presses = 0usize;
                let mut releases = 0usize;
                let mut next_press = started + Duration::from_millis(KEY_INTERVAL_MS);
                let mut next_release: Option<(Instant, u32)> = None;
                let window_end = started + window;
                loop {
                    if stop_flag.load(Ordering::Relaxed) || Instant::now() >= window_end {
                        println!("[footprint] source done: presses={presses} releases={releases}");
                        return;
                    }
                    let now = Instant::now();
                    // Fire the due release first (holds never overlap
                    // presses on this schedule — 80 ms hold, 200 ms
                    // interval — but the two lanes stay independent
                    // so scheduler jitter cannot wedge either).
                    if let Some((at, scancode)) = next_release
                        && now >= at
                    {
                        let _ = tx.send(KeyEvent {
                            scancode,
                            pressed: false,
                            timestamp: monotonic_us(),
                        });
                        releases += 1;
                        next_release = None;
                    }
                    // Fire the due press.
                    if now >= next_press {
                        let scancode = SCANCODES[presses % SCANCODES.len()];
                        let _ = tx.send(KeyEvent {
                            scancode,
                            pressed: true,
                            timestamp: monotonic_us(),
                        });
                        presses += 1;
                        next_release = Some((now + Duration::from_millis(HOLD_MS), scancode));
                        next_press += Duration::from_millis(KEY_INTERVAL_MS);
                    }
                    // Sleep to the next deadline (press, release, or
                    // window end), capped at the bridge's poll cadence
                    // so the stop flag stays responsive.
                    let mut deadline = next_press;
                    if let Some((at, _)) = next_release
                        && at < deadline
                    {
                        deadline = at;
                    }
                    if window_end < deadline {
                        deadline = window_end;
                    }
                    let cap = Duration::from_millis(CONTROL_POLL_MS);
                    std::thread::sleep(deadline.saturating_duration_since(now).min(cap));
                }
            })
            .expect("source thread")
    };

    // The player's run loop, verbatim: the orchestrator owns the
    // render pacing, the ring, and the cpal callback. The receiver
    // drops when the source thread exits — the same two exit paths
    // the player has (stop flag, channel disconnect), both through
    // the fade-out.
    let (mut orchestrator, model) = build_player(PacedNullSink::new());
    orchestrator.run(&model, &rx, Arc::clone(&stop_flag));

    let _ = sampler.join();
    let _ = watcher.join();
    let _ = source.join();
    // The final sample: the process at rest, post-fade-out, pre-drop.
    sample(&format!("{tag}-final"), window_start);
    println!("[footprint] mode={tag} complete");
}

// ── Cold-start anchors ───────────────────────────────────────────────

/// The cold-start sequence: process start → device open → first
/// sample pushed (the pre-fill's first frame — the stream is already
/// running when CpalSink::new returns) → first rendered sample
/// pushed. The sequence replicates Orchestrator::run's prelude
/// exactly (same constructor, same pre-fill count, same first-chunk
/// shape) because the anchors must live INSIDE the sequence.
pub fn cold_start() {
    let t0 = t0();
    println!(
        "[footprint] mode=cold pid={} t0_epoch_us={}",
        std::process::id(),
        t0.epoch_us,
    );

    // ── Device open + stream start (CpalSink::new starts playback).
    let mut sink =
        CpalSink::new().unwrap_or_else(|e| panic!("footprint cold-start cannot open audio: {e}"));
    let t1 = Instant::now();
    let sample_rate = sink.sample_rate();

    // ── The player's stack at the default posture.
    let mut pool = VoicePool::with_volume(VoicePool::DEFAULT_MASTER_VOLUME);
    let profile = registry_lookup(DEFAULT_PRESET)
        .map(|e| e.profile)
        .expect("the compiled default preset must exist");
    let model = MechanicalClick::with_profile(profile, sample_rate);

    // ── The pre-fill: the FIRST write_sample is the first output
    //    sample pushed into the live ring — the moment sound
    //    "reaches the device" from the player's seat.
    sink.write_sample([0.0, 0.0]);
    let t_first_push = Instant::now();
    for _ in 1..PREFILL_FRAMES {
        sink.write_sample([0.0, 0.0]);
    }

    // ── The first rendered chunk: run()'s exact first-iteration
    //    shape (trigger, 64-frame process, push).
    pool.trigger(
        &model,
        &crate::profiles::KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        },
    );
    let mut batch = [[0.0f32; 2]; COLD_CHUNK];
    for frame in batch.iter_mut() {
        *frame = pool.process_sample(&model);
    }
    sink.write_batch(&batch);
    let t3 = Instant::now();

    let device_open_ms = t1.duration_since(t0.at).as_secs_f64() * 1000.0;
    let first_push_ms = t_first_push.duration_since(t0.at).as_secs_f64() * 1000.0;
    let first_render_ms = t3.duration_since(t0.at).as_secs_f64() * 1000.0;
    println!(
        "FOOTPRINT cold device_open_ms={device_open_ms:.3} first_push_ms={first_push_ms:.3} \
         first_render_push_ms={first_render_ms:.3} rate={sample_rate}"
    );
    println!("[footprint] mode=cold complete");
    // The sink drops here: the stream stops, the null PCM forgets.
}
