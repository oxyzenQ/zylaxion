// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Real-time click demo: triggers scancode 30 every 500 ms for 3 seconds.
//!
//! Run manually:
//!   cargo run --example realtime_click
//!
//! This example is NOT executed by `cargo test` or `build.sh --check-all`.

use std::thread;
use std::time::{Duration, Instant};

// night-improve-21: the unified crate tree, wired by include. The
// example reassembles the engine + profiles + audio modules it needs
// (opt-in harness, never a gate target).
#[allow(unused_imports, dead_code)]
// night-improve-21: the included tree re-exports its full root surface; the example consumes a subset.
#[path = "../src/audio/mod.rs"]
mod audio;
#[allow(unused_imports, dead_code)]
// night-improve-21: the included tree re-exports its full root surface; the example consumes a subset.
#[path = "../src/engine/mod.rs"]
mod engine;
#[allow(unused_imports, dead_code)]
// night-improve-21: the included tree re-exports its full root surface; the example consumes a subset.
#[path = "../src/profiles/mod.rs"]
mod profiles;

use audio::{AudioSink, CpalSink};
use engine::VoicePool;
use profiles::{KeyTrigger, MechanicalClick};

fn main() {
    println!("[realtime_click] initializing audio output...");

    let mut sink = match CpalSink::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[realtime_click] failed to open audio device: {e}");
            eprintln!("[realtime_click] make sure PipeWire/PulseAudio is running");
            std::process::exit(1);
        }
    };

    let device_rate = sink.sample_rate();
    println!("[realtime_click] device sample rate: {device_rate} Hz");
    println!("[realtime_click] playing 3 seconds of key clicks...\n");

    let model = MechanicalClick::new(44100);
    let mut pool = VoicePool::new();

    /// Render batch size (frames). Small enough to keep latency low,
    /// large enough that `thread::sleep` overhead is negligible.
    const BATCH: usize = 64;
    let batch_dur = Duration::from_secs_f32(BATCH as f32 / device_rate as f32);

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut next_trigger = Instant::now();

    while Instant::now() < deadline {
        // Trigger a new key press every 500 ms.
        if Instant::now() >= next_trigger {
            pool.trigger(
                &model,
                &KeyTrigger {
                    scancode: 30,
                    pressed: true,
                    stereo_position: 0.0,
                    velocity: None,
                },
            );
            next_trigger += Duration::from_millis(500);
            println!("[realtime_click]   tek!");
        }

        // Render a small batch from the VoicePool.
        let mut batch = [[0.0f32; 2]; BATCH];
        for frame in batch.iter_mut() {
            *frame = pool.process_sample(&model);
        }
        sink.write_batch(&batch);

        // Sleep to match real-time pace and yield the CPU.
        thread::sleep(batch_dur);
    }

    // Let the last voice ring out for a moment before dropping the sink.
    thread::sleep(Duration::from_millis(200));
    println!("\n[realtime_click] done.");
}
