// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Idle-CPU sanity for the render loop — the guard for the "zero idle
//! cost" claim.
//!
//! Runs the real [`Orchestrator::run`] loop for a fixed wall window on
//! a channel that never delivers an event (the exact thread shape the
//! player's render loop occupies between keystrokes) and counts the
//! CPU time the process burns in that window via getrusage. The idle
//! loop is allowed a small blocking-wait heartbeat, never a poll; the
//! pass bar sits in the wide honest gap between the two measured
//! regimes.
//!
//! Evidence lane for the idle-CPU audit: the old flat 1 ms event poll
//! burned about 1.9 percent of one core around the clock (the
//! `ps aux` reading: 1.9 percent CPU, 76 CPU-minutes over a multi-day
//! mostly-idle session). The adaptive wait reads 0.0 percent in ps.
//!
//! PHASE 2 (the DEPTHBORE audit's extension): the ACTIVE regime — the
//! render loop with voices actually decaying, paced by a
//! hardware-shaped back-pressure sink and a 5 keystrokes/second feed
//! (the same schedule the LTS footprint proof's STATE B measured).
//! Sound production is real work; this phase measures how big that
//! work is on the host machine, so the "zero idle cost" claim can
//! never be misread as "zero cost while playing".
//!
//! Run manually:
//!   cargo run --release --example idle_cpu_sanity
//!
//! This example is NOT executed by `cargo test` or `build.sh --check-all`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

// The ringbuf trait surface the PacedSink's producer/consumer calls
// ride (try_push, vacant_len, try_pop) — the same imports the real
// CpalSink module carries.
use ringbuf::traits::{Consumer, Observer, Producer, Split};

// night-improve-21 shape: the unified crate tree, wired by include.
// The example reassembles the modules it needs (opt-in harness, never
// a gate target).
#[allow(unused_imports, dead_code)]
// night-improve-21: the included tree re-exports its full root surface; the example consumes a subset.
#[path = "../src/audio/mod.rs"]
mod audio;
#[allow(unused_imports, dead_code)]
// night-improve-21: the included tree re-exports its full root surface; the example consumes a subset.
#[path = "../src/core/mod.rs"]
mod core;
#[allow(unused_imports, dead_code)]
// night-improve-21: the included tree re-exports its full root surface; the example consumes a subset.
#[path = "../src/engine/mod.rs"]
mod engine;
#[allow(unused_imports, dead_code)]
// night-improve-21: the included tree re-exports its full root surface; the example consumes a subset.
#[path = "../src/profiles/mod.rs"]
mod profiles;

use crate::core::Orchestrator;
use crate::profiles::MechanicalClick;

/// The measurement window: long enough for the per-cycle cost to
/// average out, short enough to stay a quick manual check.
const MEASURE_WINDOW: Duration = Duration::from_secs(10);

/// The pass bar, in percent of one core. The pre-fix loop measured
/// about 1.9 percent (1000 futex wakeups per second); the post-fix
/// loop measures about 0.02 percent (10 blocking waits per second).
/// The bar sits far below the old burn and far above the new one, so
/// it fails loudly on any regression and never flickers on scheduler
/// jitter.
const PASS_BAR_PERCENT: f64 = 0.5;

/// A sink that accepts and discards every sample: the render loop's
/// producer side without an audio device. Vacancy is unbounded and
/// the rate stays the trait default, so the loop exercises only its
/// own scheduling logic, never the hardware.
struct NullSink;

impl audio::AudioSink for NullSink {
    fn write_sample(&mut self, _sample: [f32; 2]) {}
}

/// The ACTIVE phase's pass bar, in percent of one core. Sound
/// production is real work paced at ~700-1000 loop wakeups per second
/// (the 64-frame chunk throughput floor at 44.1-48 kHz), so the bar
/// must tolerate host-speed scaling while still failing loudly on a
/// lost blocking wait: the LTS footprint proof measured 2.15 percent
/// on the certification Xeon, this audit's shared sandbox measured
/// 5.2 percent (same design, ~2.5x costlier wakeups), and a real spin
/// reads near 100. Ten percent sits far above both honest hosts and
/// far below any spin — a regression catcher, not a performance gate.
const ACTIVE_BAR_PERCENT: f64 = 10.0;

/// A back-pressure sink shaped like the real hardware contract: the
/// same 16384-frame stereo ring, drained by a wall-clock thread at the
/// desktop's typical ALSA quantum (1024 frames at 48 kHz, ~21.3 ms per
/// period). The render loop's vacancy checks and the SLEEP_THRESHOLD
/// pacing branch now exercise exactly the path a real CpalSink drives,
/// with the device clock replaced by a sleeper — the same substitution
/// the LTS footprint proof's PacedNullSink made (the ALSA null PCM is
/// POLLOUT-always-ready and would spin; the real device does not).
struct PacedSink {
    producer: <ringbuf::HeapRb<[f32; 2]> as ringbuf::traits::Split>::Prod,
}

impl PacedSink {
    const RING_FRAMES: usize = 16384;
    const PERIOD_FRAMES: usize = 1024;
    const SAMPLE_RATE: u32 = 48_000;

    fn new() -> Self {
        let rb = ringbuf::HeapRb::<[f32; 2]>::new(Self::RING_FRAMES);
        let (producer, mut consumer) = rb.split();
        let period = Duration::from_nanos(
            (Self::PERIOD_FRAMES as u64 * 1_000_000_000) / Self::SAMPLE_RATE as u64,
        );
        std::thread::Builder::new()
            .name("device-clock".to_string())
            .spawn(move || {
                loop {
                    let mut popped = 0usize;
                    while popped < Self::PERIOD_FRAMES && consumer.try_pop().is_some() {
                        popped += 1;
                    }
                    std::thread::sleep(period);
                }
            })
            .expect("spawn the device-clock thread");
        Self { producer }
    }
}

impl audio::AudioSink for PacedSink {
    fn write_sample(&mut self, sample: [f32; 2]) {
        let _ = self.producer.try_push(sample);
    }
    fn producer_vacancy(&self) -> usize {
        self.producer.vacant_len()
    }
    fn sample_rate(&self) -> u32 {
        Self::SAMPLE_RATE
    }
}

/// Process CPU time (user + system) in seconds, via getrusage.
fn cpu_seconds() -> f64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // Safety: getrusage(RUSAGE_SELF) only fills `usage`, a valid,
    // fully initialized allocation of exactly the ABI's own type; the
    // call stores no pointer and aliases nothing.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    if rc != 0 {
        return f64::NAN;
    }
    let user = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 / 1e6;
    let sys = usage.ru_stime.tv_sec as f64 + usage.ru_stime.tv_usec as f64 / 1e6;
    user + sys
}

fn main() {
    // ── PHASE 1: the idle regime (the original guard) ──────────────
    println!(
        "[idle_cpu_sanity] phase 1: measuring the render loop idle cost over {}s...",
        MEASURE_WINDOW.as_secs()
    );

    let mut orchestrator = Orchestrator::new_with(NullSink);
    let model = Arc::new(arc_swap::ArcSwap::from_pointee(MechanicalClick::new(
        44_100,
    )));
    let (tx, rx) = crossbeam_channel::unbounded::<crate::core::KeyEvent>();
    let stop_flag = Arc::new(AtomicBool::new(false));

    let cpu_before = cpu_seconds();
    let wall = Instant::now();

    let loop_flag = Arc::clone(&stop_flag);
    let handle = std::thread::Builder::new()
        .name("idle-loop".to_string())
        .spawn(move || {
            // The receiver never sees an event: the loop spends the
            // whole window in its idle branch (no voices active), the
            // exact regime `ps` reports on between keystrokes.
            orchestrator.run(&model, &rx, loop_flag);
        })
        .expect("spawn the idle-loop thread");

    std::thread::sleep(MEASURE_WINDOW);

    // Stop via the flag — the same lane `zylaxion off` rides (the
    // control watcher clears enabled=0, the watcher sets this flag).
    // The sender drops too, so no blocking wait can wedge the join.
    stop_flag.store(true, Ordering::Relaxed);
    drop(tx);
    handle.join().expect("idle loop exits cleanly on stop");

    let wall_s = wall.elapsed().as_secs_f64();
    let cpu = cpu_seconds() - cpu_before;
    let idle_percent = 100.0 * cpu / wall_s;

    println!(
        "[idle_cpu_sanity] wall {wall_s:.2}s, cpu {cpu:.4}s, idle cost {idle_percent:.3}% of one core"
    );
    let mut failed = false;
    if idle_percent.is_nan() || idle_percent > PASS_BAR_PERCENT {
        println!(
            "[idle_cpu_sanity] FAIL: idle cost {idle_percent:.3}% exceeds the {PASS_BAR_PERCENT}% bar \
             — a poll is hiding in the idle path"
        );
        failed = true;
    } else {
        println!(
            "[idle_cpu_sanity] PASS: {idle_percent:.3}% <= the {PASS_BAR_PERCENT}% bar — ps reads 0.0%"
        );
    }

    // ── PHASE 2: the active regime (the DEPTHBORE extension) ───────
    // Voices actually decaying, hardware-shaped back-pressure, 5
    // keystrokes per second — the cost of PRODUCING sound, which is
    // real work and is reported as such (the LTS footprint proof's
    // STATE B schedule). The two helper threads (device clock, key
    // feed) sleep over 99 percent of the window; their cost is
    // included in the process-wide getrusage delta and is negligible.
    println!(
        "[idle_cpu_sanity] phase 2: measuring the ACTIVE cost over {}s (5 keys/s, paced ring)...",
        MEASURE_WINDOW.as_secs()
    );

    let mut orchestrator = Orchestrator::new_with(PacedSink::new());
    let model = Arc::new(arc_swap::ArcSwap::from_pointee(MechanicalClick::new(
        PacedSink::SAMPLE_RATE,
    )));
    let (tx, rx) = crossbeam_channel::unbounded::<crate::core::KeyEvent>();
    let stop_flag = Arc::new(AtomicBool::new(false));

    let feed_flag = Arc::clone(&stop_flag);
    // The feeder owns a clone of the sender; main keeps the original so
    // the drop below (plus the feeder's exit) disconnects the channel —
    // the same shutdown shape phase 1 rides.
    let feed_tx = tx.clone();
    let feeder = std::thread::Builder::new()
        .name("key-feed".to_string())
        .spawn(move || {
            // 5 keys/s with 300 ms holds: a press every 200 ms, each
            // released 300 ms later — holds overlap, the LTS STATE B
            // schedule. A 100 ms tick drives both lanes (press every
            // 2 ticks, release 3 ticks after its press). Scancodes
            // cycle A/S/D/F/G (30-34); the timestamps are wall-clock
            // microseconds, CLOCK_MONOTONIC's unit in the real tap.
            let mut tick = 0u64;
            let start = Instant::now();
            let keys = [30u32, 31, 32, 33, 34];
            let mut held: Vec<(u32, u64)> = Vec::new();
            while !feed_flag.load(Ordering::Relaxed) {
                held.retain(|(scancode, pressed_at)| {
                    if tick - pressed_at >= 3 {
                        let t = start.elapsed().as_micros() as u64;
                        let _ = feed_tx.send(crate::core::KeyEvent {
                            scancode: *scancode,
                            pressed: false,
                            timestamp: t,
                        });
                        false
                    } else {
                        true
                    }
                });
                if tick.is_multiple_of(2) {
                    let scancode = keys[(tick / 2) as usize % keys.len()];
                    let t = start.elapsed().as_micros() as u64;
                    let _ = feed_tx.send(crate::core::KeyEvent {
                        scancode,
                        pressed: true,
                        timestamp: t,
                    });
                    held.push((scancode, tick));
                }
                tick += 1;
                std::thread::sleep(Duration::from_millis(100));
            }
        })
        .expect("spawn the key-feed thread");

    let cpu_before = cpu_seconds();
    let wall = Instant::now();

    let loop_flag = Arc::clone(&stop_flag);
    let handle = std::thread::Builder::new()
        .name("active-loop".to_string())
        .spawn(move || {
            orchestrator.run(&model, &rx, loop_flag);
        })
        .expect("spawn the active-loop thread");

    std::thread::sleep(MEASURE_WINDOW);

    stop_flag.store(true, Ordering::Relaxed);
    drop(tx);
    handle.join().expect("active loop exits cleanly on stop");
    let _ = feeder.join();

    let wall_s = wall.elapsed().as_secs_f64();
    let cpu = cpu_seconds() - cpu_before;
    let active_percent = 100.0 * cpu / wall_s;

    println!(
        "[idle_cpu_sanity] wall {wall_s:.2}s, cpu {cpu:.4}s, ACTIVE cost {active_percent:.3}% of one core"
    );
    if active_percent.is_nan() || active_percent > ACTIVE_BAR_PERCENT {
        println!(
            "[idle_cpu_sanity] FAIL: active cost {active_percent:.3}% exceeds the \
             {ACTIVE_BAR_PERCENT}% bar — the render pacing lost its blocking shape \
             (a spin is hiding in the active path)"
        );
        failed = true;
    } else {
        println!(
            "[idle_cpu_sanity] PASS: {active_percent:.3}% <= the {ACTIVE_BAR_PERCENT}% bar — \
             sound production costs real work, and this machine's amount is honest"
        );
    }

    if failed {
        std::process::exit(1);
    }
}
