// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Live keyboard capture demo.
//!
//! Prints every key event received from libinput to stdout.
//!
//! Run manually:
//!   cargo run --example listen_keys -p zylaxion-input
//!
//! This example is **not** executed by `cargo test` or
//! `build.sh --check-all` — it requires a running Linux system with
//! input-group membership and an attached keyboard.
//!
//! Press Ctrl+C to stop.

use zylaxion_input::{InputSource, LibinputSource};

fn main() {
    println!("[listen_keys] initialising libinput on seat0 ...");

    let mut source = LibinputSource::new();
    let rx = match source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            eprintln!("[listen_keys] failed to start input capture: {e}");
            std::process::exit(1);
        }
    };

    println!("[listen_keys] listening — press any key (Ctrl+C to quit)\n");

    for event in rx.iter() {
        println!(
            "[input] scancode: {}, pressed: {}",
            event.scancode, event.pressed
        );
    }
}
