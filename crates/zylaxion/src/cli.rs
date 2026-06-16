// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! CLI definitions: argument parsing, version output, flag wiring.
//!
//! All clap structs and the masterclass version format live here.
//! `main.rs` only calls `Cli::parse()` and dispatches subcommands.

use clap::{Parser, Subcommand};

/// Build hash placeholder — replaced at release time by CI.
#[allow(dead_code)]
const BUILD_HASH: &str = "a1b2c3d";

/// Masterclass multi-line version string.
const LONG_VERSION: &str = concat!(
    "Version: v",
    env!("CARGO_PKG_VERSION"),
    "\n",
    "Build: linux-x86_64 (a1b2c3d)\n",
    "Copyright: (c) 2026 rezky_nightky (oxyzenQ)\n",
    "License: GPL-3.0-or-later\n",
    "Source: https://github.com/oxyzenQ/zylaxion"
);

/// Zylaxion — mechanical keyboard acoustic synthesizer
#[derive(Parser)]
#[command(
    name = "zylaxion",
    version = concat!("v", env!("CARGO_PKG_VERSION")),
    long_version = LONG_VERSION,
    about = "Real-time mechanical keyboard acoustic synthesizer for Linux",
    after_help = "License: GPL-3.0-or-later | https://github.com/oxyzenQ/zylaxion"
)]
pub struct Cli {
    /// Check for upstream updates on GitHub (placeholder)
    #[arg(long, global = true)]
    pub check_updated: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run in the foreground (press Ctrl+C to quit)
    Start {
        /// Acoustic profile name (e.g. technical, classic, studio, elegant, whisper)
        #[arg(long, global = true)]
        profile: Option<String>,
    },

    /// Run as a background daemon (controlled via Unix socket)
    Daemon {
        /// Acoustic profile name (e.g. technical, classic, studio, elegant, whisper)
        #[arg(long, global = true)]
        profile: Option<String>,
    },

    /// Stop a running daemon
    Stop,

    /// Show daemon status
    Status,

    /// Print system health diagnostic
    Doctor,

    /// List available acoustic profiles
    ListProfiles,

    /// List available audio backends
    ListBackends,
}
