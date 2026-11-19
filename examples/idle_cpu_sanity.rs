// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Idle-CPU sanity for the render loop — the guard for the "zero idle
//! cost" claim.
//!
//! Runs the real [`Orchestrator::run`] loop for a fixed wall window on
//! a channel that never delivers an event (the exact thread shape the
//! player's render loop occupies between keystrokes) and counts the
//! CPU time the process burns in that window via getrusage. The idle
//! loop is a PARKED thread (night-frontier-Z4: no timer while silent),
//! never a poll; the pass bar sits in the wide honest gap between the
//! two measured regimes.
//!
//! Evidence lane for the idle-CPU audit: the old flat 1 ms event poll
//! burned about 1.9 percent of one core around the clock (the
//! `ps aux` reading: 1.9 percent CPU, 76 CPU-minutes over a multi-day
//! mostly-idle session). The adaptive wait read 0.0 percent in ps.
//! night-frontier-Z4 goes further: the silent loop parks with NO
//! timer — the parked thread must emit ZERO context switches over the
//! window, and PHASE 1 witnesses exactly that from
//! /proc/self/task/<tid>/status.
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
    // Safety: `rusage` is a plain C aggregate with no pointers and no
    // valid-bit requirements; an all-zero value is the ABI-legal
    // starting state getrusage overwrites wholesale.
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

/// The Z4 witness reader: the named thread's (voluntary,
/// nonvoluntary) context switches from /proc/self/task, or None when
/// /proc is unavailable — the witness is then skipped with a warning,
/// never a silent pass.
fn thread_wakeups(name: &str) -> Option<(u64, u64)> {
    let tasks = std::fs::read_dir("/proc/self/task").ok()?;
    for entry in tasks.flatten() {
        let tid = entry.file_name();
        let comm =
            std::fs::read_to_string(format!("/proc/self/task/{}/comm", tid.display())).ok()?;
        if comm.trim_end() == name {
            let status =
                std::fs::read_to_string(format!("/proc/self/task/{}/status", tid.display()))
                    .ok()?;
            let mut vol = None;
            let mut nonvol = None;
            for line in status.lines() {
                if let Some(v) = line.strip_prefix("voluntary_ctxt_switches:") {
                    vol = v.trim().parse::<u64>().ok();
                }
                if let Some(v) = line.strip_prefix("nonvoluntary_ctxt_switches:") {
                    nonvol = v.trim().parse::<u64>().ok();
                }
            }
            return Some((vol?, nonvol?));
        }
    }
    None
}

/// One cpuidle snapshot: per (cpu, state) name -> (entries, time_us),
/// read from /sys/devices/system/cpu/*/cpuidle/state*/. The deep
/// C-state residency is a HOST property — a micro-VM guest cannot see
/// the host's idle states — so this witness belongs to this
/// owner-machine instrument, not the E2E battery. Absent sysfs (a
/// guest kernel without cpuidle, or a config without the driver)
/// returns None and the witness is honestly skipped, never faked.
fn cpuidle_snapshot() -> Option<Vec<(String, String, u64, u64)>> {
    let mut rows = Vec::new();
    let cpus = std::fs::read_dir("/sys/devices/system/cpu").ok()?;
    for cpu in cpus.flatten() {
        let cpu_name = cpu.file_name().to_string_lossy().to_string();
        if !cpu_name.starts_with("cpu") || cpu_name == "cpuidle" {
            continue;
        }
        let states = match std::fs::read_dir(cpu.path().join("cpuidle")) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for state in states.flatten() {
            let dir = state.path();
            let name = std::fs::read_to_string(dir.join("name"))
                .unwrap_or_default()
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let time: u64 = std::fs::read_to_string(dir.join("time"))
                .ok()?
                .trim()
                .parse()
                .unwrap_or(0);
            let usage: u64 = std::fs::read_to_string(dir.join("usage"))
                .ok()?
                .trim()
                .parse()
                .unwrap_or(0);
            rows.push((cpu_name.clone(), name, usage, time));
        }
    }
    if rows.is_empty() { None } else { Some(rows) }
}

/// Print the residency delta rows between two snapshots, aggregated
/// by state name across cpus (the per-cpu rows carry the same states).
fn print_cpuidle_deltas(
    before: &Option<Vec<(String, String, u64, u64)>>,
    after: &Option<Vec<(String, String, u64, u64)>>,
) {
    let (Some(b), Some(a)) = (before, after) else {
        println!(
            "[idle_cpu_sanity] C-state witness: SKIP — cpuidle sysfs is absent on this host \
             (a guest kernel without the driver, or no deep states exposed)"
        );
        return;
    };
    // Aggregate by state name: sum usage and time across cpus.
    let mut names: Vec<String> = b.iter().map(|(_, n, _, _)| n.clone()).collect();
    names.sort_unstable();
    names.dedup();
    for name in &names {
        let used_before: u64 = b
            .iter()
            .filter(|(_, n, _, _)| n == name)
            .map(|(_, _, u, _)| u)
            .sum();
        let bt: u64 = b
            .iter()
            .filter(|(_, n, _, _)| n == name)
            .map(|(_, _, _, t)| t)
            .sum();
        let au: u64 = a
            .iter()
            .filter(|(_, n, _, _)| n == name)
            .map(|(_, _, u, _)| u)
            .sum();
        let at: u64 = a
            .iter()
            .filter(|(_, n, _, _)| n == name)
            .map(|(_, _, _, t)| t)
            .sum();
        let d_time = at.saturating_sub(bt);
        let d_use = au.saturating_sub(used_before);
        println!(
            "[idle_cpu_sanity] C-state residency: {name} +{d_use} entries, +{d_time} us over the silent window"
        );
    }
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
    let (stop_tx, stop_rx) = crossbeam_channel::unbounded::<crate::core::ControlStop>();

    let cpu_before = cpu_seconds();
    // The Z6 C-state witness: the residency snapshot brackets the same
    // parked window the wakeup witness counts, so the deep-idle rows
    // describe the exact regime the 0.004% claim rides.
    let cstates_before = cpuidle_snapshot();
    let wall = Instant::now();

    let handle = std::thread::Builder::new()
        .name("idle-loop".to_string())
        .spawn(move || {
            // The receiver never sees an event: the loop spends the
            // whole window PARKED in the silent select (no voices
            // active), the exact regime `ps` reports on between
            // keystrokes.
            orchestrator.run(&model, &rx, &stop_rx);
        })
        .expect("spawn the idle-loop thread");

    // The Z4 wakeup witness: let the loop reach its park, then count
    // the thread's context switches across the silent window. The
    // pre-Z4 loop's 100 ms timeout woke the thread about ten times
    // per second here; the parked loop must show ZERO.
    std::thread::sleep(Duration::from_secs(1));
    let wake_before = thread_wakeups("idle-loop");
    std::thread::sleep(MEASURE_WINDOW - Duration::from_secs(1));
    let wake_after = thread_wakeups("idle-loop");
    let cstates_after = cpuidle_snapshot();

    // Stop via the control stop lane — the same message `zylaxion
    // off` ends up riding (the watcher forwards one ControlStop per
    // stop decision). The dropped senders are the belt-and-braces
    // wake: a disconnect is a ready op, so no blocking wait wedges
    // the join.
    let _ = stop_tx.send(crate::core::ControlStop);
    drop(tx);
    drop(stop_tx);
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

    // The Z4 witness verdict (night-frontier-Z4): the parked loop's
    // context-switch delta over the silent window must be exactly
    // zero — the mechanical, /proc-counted proof that the idle wait
    // is a park, not a timer.
    match (wake_before, wake_after) {
        (Some((bv, bn)), Some((av, an))) => {
            let dv = av - bv;
            let dn = an - bn;
            println!(
                "[idle_cpu_sanity] phase 1 witness: voluntary +{dv}, nonvoluntary +{dn} over the silent window"
            );
            if dv != 0 || dn != 0 {
                println!(
                    "[idle_cpu_sanity] FAIL: the parked loop woke {dv} voluntary +{dn} nonvoluntary \
                     times — a timer hides in the idle wait (the Z4 park is broken)"
                );
                failed = true;
            } else {
                println!(
                    "[idle_cpu_sanity] PASS: the parked loop emitted ZERO wakeups — \
                     the idle wait is a park, not a timer"
                );
            }
        }
        _ => {
            println!(
                "[idle_cpu_sanity] WARN: the wakeup witness could not read /proc/self/task \
                 (the ctx-switch evidence is unavailable on this host; the CPU bar above still ran)"
            );
        }
    }

    // The Z6 residency rows: the physics half of the idle claim — the
    // parked loop lets the core fall into the machine's idle states and
    // STAY there. The rows are measurements (the states a machine
    // exposes are its own); the gate above is the CPU and wakeup bar.
    print_cpuidle_deltas(&cstates_before, &cstates_after);

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
    let feed_flag = Arc::new(AtomicBool::new(false));
    // Main's own handle to the feeder's flag — the feeder moves its
    // clone into the closure; the stop below stores through this one.
    let feed_flag_main = Arc::clone(&feed_flag);
    let (stop_tx, stop_rx) = crossbeam_channel::unbounded::<crate::core::ControlStop>();

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

    let handle = std::thread::Builder::new()
        .name("active-loop".to_string())
        .spawn(move || {
            orchestrator.run(&model, &rx, &stop_rx);
        })
        .expect("spawn the active-loop thread");

    std::thread::sleep(MEASURE_WINDOW);

    let _ = stop_tx.send(crate::core::ControlStop);
    feed_flag_main.store(true, Ordering::Relaxed);
    drop(tx);
    drop(stop_tx);
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
