// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! CLI definitions: argument parsing, version output, flag wiring.

use clap::{Parser, Subcommand};

/// Multi-line version string emitted by `-V` / `--version`.
///
/// `GIT_HASH` is injected at compile time by `build.rs` via
/// `git rev-parse --short HEAD`. Falls back to `"unknown"` if git is
/// unavailable or the crate is built outside a git work tree.
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

/// Zylaxion — real-time mechanical keyboard acoustic synthesizer for Linux.
#[derive(Parser)]
#[command(
    name = "zylaxion",
    version = LONG_VERSION,
    about = "Real-time mechanical keyboard acoustic synthesizer for Linux",
    after_help = "License: GPL-3.0-or-later | https://github.com/oxyzenQ/zylaxion"
)]
pub struct Cli {
    /// Check for upstream updates on GitHub.
    #[arg(long, global = true)]
    #[allow(dead_code)]
    pub check_update: bool,

    /// Enable verbose (debug-level) logging.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run in the foreground (press Ctrl+C to quit).
    Start {
        /// Override the active preset from config.toml. If omitted, the
        /// `preset.tuning` value is used.
        #[arg(long)]
        preset: Option<String>,
    },

    /// Run as a background daemon (controlled via Unix socket).
    Daemon {
        /// Override the active preset from config.toml. If omitted, the
        /// `preset.tuning` value is used.
        #[arg(long)]
        preset: Option<String>,

        /// Skip the fork/setsid daemonization and run the daemon logic
        /// in the foreground (block the main thread).
        ///
        /// This is intended for process supervisors like systemd, which
        /// expect the launched process to stay in the foreground and be
        /// supervised by the manager. The `zylaxion.service` unit uses
        /// `Type=simple` + `zylaxion daemon --foreground` so systemd
        /// can track the live PID directly. Without this flag, the
        /// `daemon` subcommand forks to the background and the parent
        /// exits — which makes systemd think the service died.
        ///
        /// When `--foreground` is set:
        /// - `daemonize()` (fork + setsid) is skipped.
        /// - `close_std_fds()` is skipped so logs still reach journald
        ///   via the standard streams systemd wires up.
        /// - PID file, IPC socket, signal handlers, config-watcher, and
        ///   the orchestrator loop all run inline on the main thread.
        #[arg(long, default_value_t = false)]
        foreground: bool,
    },

    /// Stop a running daemon
    Stop,

    /// Show daemon status
    Status,

    /// Print system health diagnostic
    Doctor,

    /// Validate config.toml syntax and parameter ranges
    Testconf,

    /// List available acoustic presets from config.toml
    ListPresets,

    /// List available audio backends
    ListBackends,
}
