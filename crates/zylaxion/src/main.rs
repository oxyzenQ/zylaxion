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
    let cli = cli::Cli::parse();

    // --check-updated: placeholder — prints message and exits.
    if cli.check_updated {
        println!("Checking for updates...");
        println!("(placeholder: will query GitHub releases API in a future version)");
        std::process::exit(0);
    }

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
