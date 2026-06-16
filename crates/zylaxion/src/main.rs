// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! **Zylaxion** — real-time mechanical keyboard acoustic synthesizer for Linux.
//!
//! Transforms every keystroke into a spatially-accurate click sound
//! through your speakers, using the kernel's evdev interface and
//! low-latency audio via cpal / PipeWire.
//!
//! # Architecture
//!
//! - [`cli`]       — clap argument definitions & masterclass version format
//! - [`profile`]   — filesystem search for acoustic profile TOMLs
//! - [`commands`]  — subcommand handlers (`start`, `daemon`, `stop`, `doctor`, …)
//! - [`daemon`]    — POSIX daemonization, PID file, signal handling, IPC

use clap::Parser;

mod cli;
mod commands;
mod daemon;
mod profile;

fn main() {
    // Early-exit flags — intercepted BEFORE `Cli::parse()` to bypass Clap's
    // subcommand requirement. This is the bulletproof way to handle early-exit
    // flags without fighting the parser: `zylaxion --check-updated` (with no
    // subcommand) would otherwise error out before the flag is ever processed.
    if std::env::args().any(|a| a == "--check-updated") {
        println!("Checking for updates...");
        std::process::exit(0);
    }

    let cli = cli::Cli::parse();

    match cli.command {
        cli::Commands::Start { profile } => commands::daemon::cmd_start(profile),
        cli::Commands::Daemon { profile } => commands::daemon::cmd_daemon(profile),
        cli::Commands::Stop => commands::daemon::cmd_stop(),
        cli::Commands::Status => daemon::cmd_status(),
        cli::Commands::Doctor => commands::info::cmd_doctor(),
        cli::Commands::ListProfiles => commands::info::cmd_list_profiles(),
        cli::Commands::ListBackends => commands::info::cmd_list_backends(),
    }
}
