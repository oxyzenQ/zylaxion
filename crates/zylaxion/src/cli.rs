// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI definitions: argument parsing, version output, flag wiring.

use clap::{Parser, Subcommand};
use std::sync::OnceLock;

/// Dynamic build target: detects arch + libc env at compile time.
/// Returns e.g. "linux-amd64-gnu" (glibc, dynamic) or "linux-amd64-musl"
/// (static) for x86_64 Linux builds.
const BUILD_TARGET: &str = {
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "musl"))]
    {
        "linux-amd64-musl"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    {
        "linux-amd64-gnu"
    }
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        not(any(target_env = "musl", target_env = "gnu"))
    ))]
    {
        "linux-amd64"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "musl"))]
    {
        "linux-aarch64-musl"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "gnu"))]
    {
        "linux-aarch64-gnu"
    }
    #[cfg(all(
        target_os = "linux",
        target_arch = "aarch64",
        not(any(target_env = "musl", target_env = "gnu"))
    ))]
    {
        "linux-aarch64"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    {
        "unknown"
    }
};

/// Multi-line version string emitted by `-V` / `--version`.
///
/// `GIT_HASH` is injected at compile time by `build.rs` via
/// `git rev-parse --short HEAD`. Falls back to `"unknown"` if git is
/// unavailable or the crate is built outside a git work tree.
///
/// Built lazily at first access because `BUILD_TARGET` is selected via
/// `cfg!` at compile time but the full string needs runtime `format!`.
static LONG_VERSION_CELL: OnceLock<String> = OnceLock::new();

fn long_version() -> &'static str {
    LONG_VERSION_CELL.get_or_init(|| {
        format!(
            "Version: v{}\nBuild: {} ({})\nCopyright: (c) 2026 rezky_nightky (oxyzenQ)\nLicense: GPL-3.0-only\nSource: https://github.com/oxyzenQ/zylaxion",
            env!("CARGO_PKG_VERSION"),
            BUILD_TARGET,
            option_env!("GIT_HASH").unwrap_or("unknown")
        )
    }).as_str()
}

/// Zylaxion — real-time mechanical keyboard acoustic synthesizer for Linux.
#[derive(Parser)]
#[command(
    name = "zylaxion",
    version = long_version(),
    about = "Real-time mechanical keyboard acoustic synthesizer for Linux",
    after_help = "License: GPL-3.0-only | https://github.com/oxyzenQ/zylaxion"
)]
pub struct Cli {
    /// Check for upstream updates on GitHub.
    #[arg(long, global = true)]
    #[allow(dead_code)]
    pub check_update: bool,

    /// Enable verbose (debug-level) logging.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Without a subcommand: show a quick status overview (daemon
    /// state, active preset, audio device). With a subcommand: run
    /// that subcommand. (v10.2.0+ — user feedback)
    #[command(subcommand)]
    pub command: Option<Commands>,
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

    /// Validate config.toml syntax and parameter ranges.
    ///
    /// Without an argument: validates the config found via the search
    /// path (~/.config → /etc → /usr/local/share → ./).
    /// With a path argument: validates that specific file instead.
    Testconf {
        /// Optional path to a config.toml file to validate.
        /// If omitted, searches the standard config paths.
        #[arg(long = "file", short = 'f')]
        file: Option<String>,
    },

    /// List available acoustic presets from config.toml
    ListPresets,

    /// List available audio backends
    ListBackends,
}
