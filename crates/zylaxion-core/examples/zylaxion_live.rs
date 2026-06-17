// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Zylaxion live — the moment of truth.
//!
//! Press any key on your keyboard and hear it through your speakers
//! in real time.  This is the full pipeline: kernel → libinput →
//! VoicePool → ring buffer → cpal → speakers.
//!
//! Run manually:
//!   cargo run --example zylaxion_live -p zylaxion-core
//!
//! Prerequisites:
//!   - Audio server running (PipeWire / PulseAudio)
//!   - User in the `input` group: `sudo usermod -aG input $USER`
//!
//! Press Ctrl+C to stop.

use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use arc_swap::ArcSwap;
use zactrix_profiles::MechanicalClick;
use zylaxion_core::Orchestrator;
use zylaxion_input::{InputSource, LibinputSource};

fn main() {
    println!("=== Zylaxion Live ===\n");

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
    // wrapper for consistency with the daemon mode.
    let model = Arc::new(ArcSwap::from_pointee(MechanicalClick::new()));
    println!("[zylaxion-live] ready — press any key to hear it!");
    println!("[zylaxion-live] Ctrl+C to quit\n");

    // Dummy stop flag — never set, runs until Ctrl+C disconnects input.
    let stop_flag = Arc::new(AtomicBool::new(false));

    orchestrator.run(&model, &event_rx, stop_flag);

    println!("\n[zylaxion-live] goodbye.");
}
