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
//! Run manually:
//!   cargo run --release --example idle_cpu_sanity
//!
//! This example is NOT executed by `cargo test` or `build.sh --check-all`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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
    println!(
        "[idle_cpu_sanity] measuring the render loop idle cost over {}s...",
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
    let percent = 100.0 * cpu / wall_s;

    println!(
        "[idle_cpu_sanity] wall {wall_s:.2}s, cpu {cpu:.4}s, idle cost {percent:.3}% of one core"
    );
    if percent.is_nan() || percent > PASS_BAR_PERCENT {
        println!(
            "[idle_cpu_sanity] FAIL: idle cost {percent:.3}% exceeds the {PASS_BAR_PERCENT}% bar \
             — a poll is hiding in the idle path"
        );
        std::process::exit(1);
    }
    println!(
        "[idle_cpu_sanity] PASS: {percent:.3}% <= the {PASS_BAR_PERCENT}% bar — ps reads 0.0%"
    );
}
