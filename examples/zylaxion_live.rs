// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! zylaxion live — the moment of truth.
//!
//! Press any key on your keyboard and hear it through your speakers
//! in real time.  This is the full pipeline: kernel → libinput →
//! VoicePool → ring buffer → cpal → speakers.
//!
//! Run manually:
//!   cargo run --example zylaxion_live
//!
//! Prerequisites:
//!   - Audio server running (PipeWire / PulseAudio)
//!   - User in the `input` group: `sudo usermod -aG input $USER`
//!
//! Press Ctrl+C to stop.

use std::process;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use arc_swap::ArcSwap;
// night-improve-21: the unified crate tree, wired by include. The
// example reassembles the full dragon spine (profiles, engine, audio,
// core, input) the Orchestrator drives (opt-in harness, never a gate
// target).
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
#[path = "../src/input/mod.rs"]
mod input;
#[allow(unused_imports, dead_code)]
// night-improve-21: the included tree re-exports its full root surface; the example consumes a subset.
#[path = "../src/profiles/mod.rs"]
mod profiles;

use crate::core::Orchestrator;
use crate::input::{InputSource, LibinputSource};
use profiles::MechanicalClick;

fn main() {
    println!("=== zylaxion Live ===\n");

    // ── 1. Start input capture ────────────────────────────────────
    println!("[zylaxion-live] starting keyboard capture...");
    let mut input_source = LibinputSource::new();
    let event_rx = match input_source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            eprintln!("[zylaxion-live] input error: {e}");
            process::exit(1);
        }
    };
    println!("[zylaxion-live] keyboard capture active (seat0)\n");

    // ── 2. Create the orchestrator (audio + engine) ───────────────
    println!("[zylaxion-live] initialising audio engine...");
    let mut orchestrator = match Orchestrator::new() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[zylaxion-live] failed: {e}");
            eprintln!("[zylaxion-live] make sure PipeWire/PulseAudio is running");
            process::exit(1);
        }
    };

    // ── 3. Run the main loop (blocks until Ctrl+C) ────────────────
    // The model is wrapped in Arc<ArcSwap<>> for hot-reload support.
    // In this example we never swap it, but the API requires the
    // wrapper for consistency with the daemon mode. The sample rate
    // comes from the audio device so DSP math is accurate at any rate.
    let sample_rate = orchestrator.sample_rate();
    let model = Arc::new(ArcSwap::from_pointee(MechanicalClick::new(sample_rate)));
    println!("[zylaxion-live] sample rate: {sample_rate} Hz");
    println!("[zylaxion-live] ready — press any key to hear it!");
    println!("[zylaxion-live] Ctrl+C to quit\n");

    // Dummy stop flag — never set, runs until Ctrl+C disconnects input.
    let stop_flag = Arc::new(AtomicBool::new(false));

    orchestrator.run(&model, &event_rx, stop_flag);

    println!("\n[zylaxion-live] goodbye.");
}
