// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Live keyboard capture demo — **diagnostic build only**.
//!
//! By default this example prints ONLY an opaque "key event received"
//! counter per second, never the scancode. This keeps the demo safe to
//! run on a production system where the terminal might be logged.
//!
//! To opt into raw scancode output for hardware debugging, pass
//! `--dump-scancodes` on the command line. This flag is intentionally
//! verbose so it cannot be triggered by accident:
//!
//! ```text
//! cargo run --example listen_keys -p zylaxion-input -- --dump-scancodes
//! ```
//!
//! Run manually:
//!   cargo run --example listen_keys -p zylaxion-input
//!
//! This example is **not** executed by `cargo test` or
//! `build.sh --check-all` — it requires a running Linux system with
//! input-group membership and an attached keyboard.
//!
//! Press Ctrl+C to stop.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use zylaxion_input::{InputSource, LibinputSource};

/// Parse argv for `--dump-scancodes`. Any other argument is ignored.
fn wants_dump() -> bool {
    std::env::args().any(|a| a == "--dump-scancodes")
}

fn main() {
    let dump = wants_dump();

    println!("[listen_keys] initialising libinput on seat0 ...");
    if dump {
        eprintln!("[listen_keys] WARNING: --dump-scancodes is set.");
        eprintln!("[listen_keys] WARNING: raw scancodes will be printed to stdout.");
        eprintln!("[listen_keys] WARNING: do NOT run this on a system where the");
        eprintln!("[listen_keys] WARNING: terminal output is logged — scancode patterns");
        eprintln!("[listen_keys] WARNING: can be reconstructed into typed text.");
    } else {
        println!("[listen_keys] privacy mode: scancodes are NOT printed.");
        println!("[listen_keys] pass --dump-scancodes to print raw scancodes for debugging.");
    }

    let mut source = LibinputSource::new();
    let rx = match source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            eprintln!("[listen_keys] failed to start input capture: {e}");
            std::process::exit(1);
        }
    };

    println!("[listen_keys] listening — press any key (Ctrl+C to quit)\n");

    if dump {
        // Raw scancode output — diagnostic mode only.
        for event in rx.iter() {
            println!(
                "[input] scancode: {}, pressed: {}",
                event.scancode, event.pressed
            );
        }
        return;
    }

    // Privacy-respecting mode: print only an opaque per-second counter
    // so the demo can be run safely on a production system.
    let counter = AtomicU64::new(0);
    let mut last_report = Instant::now();
    for event in rx.iter() {
        let _ = event; // explicitly drop — do NOT inspect fields
        counter.fetch_add(1, Ordering::Relaxed);
        if last_report.elapsed() >= Duration::from_secs(1) {
            let n = counter.swap(0, Ordering::Relaxed);
            println!("[input] {n} key event(s) in the last second");
            last_report = Instant::now();
        }
    }
}
