// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! CLI definitions: argument parsing, version output, flag wiring.
//!
//! All clap structs and the masterclass version format live here.
//! `main.rs` only calls `Cli::parse()` and dispatches subcommands.

use clap::{Parser, Subcommand};

/// Masterclass multi-line version string.
///
/// `GIT_HASH` is injected at compile time by `build.rs`, which runs
/// `git rev-parse --short HEAD`. If git is unavailable or the crate is
/// built outside a git work tree, `build.rs` falls back to `"unknown"`.
const LONG_VERSION: &str = concat!(
    "Version: v",
    env!("CARGO_PKG_VERSION"),
    "\n",
    "Build: linux-x86_64 (",
    env!("GIT_HASH"),
    ")\n",
    "Copyright: (c) 2026 rezky_nightky (oxyzenQ)\n",
    "License: GPL-3.0-or-later\n",
    "Source: https://github.com/oxyzenQ/zylaxion"
);

/// Zylaxion — mechanical keyboard acoustic synthesizer
///
/// Note on version output: we deliberately bind ONLY `version` (not `long_version`)
/// to the multi-line `LONG_VERSION` string. In Clap 4, when both are set, `-V` prints
/// the short string and `--version` prints the long one — which would re-introduce
/// the inconsistency we just fixed. Binding only `version` makes `-V` and `--version`
/// emit the identical multi-line masterclass output.
#[derive(Parser)]
#[command(
    name = "zylaxion",
    version = LONG_VERSION,
    about = "Real-time mechanical keyboard acoustic synthesizer for Linux",
    after_help = "License: GPL-3.0-or-later | https://github.com/oxyzenQ/zylaxion"
)]
pub struct Cli {
    /// Check for upstream updates on GitHub.
    ///
    /// This flag is intercepted in `main.rs` BEFORE `Cli::parse()` runs, so it can
    /// be invoked without a subcommand (`zylaxion --check-updated`). The field is
    /// retained on the struct purely for `--help` discoverability.
    #[arg(long, global = true)]
    #[allow(dead_code)]
    pub check_updated: bool,

    /// Enable verbose (debug-level) logging.
    ///
    /// When set, `RUST_LOG=debug` is exported before subcommand dispatch,
    /// causing `env_logger` (initialised inside `cmd_start` / `cmd_daemon`)
    /// to emit debug-level lines from all crates in the workspace. Without
    /// this flag, the default level is `info`.
    #[arg(short, long, global = true)]
    pub verbose: bool,

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
