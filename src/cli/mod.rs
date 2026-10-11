// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI definitions: argument parsing, version output, flag wiring.
//!
//! The version report (night-improve-13) follows the zelynic reference
//! contract: brand header (`zylaxion: v<ver>` + the one-line tagline),
//! then the plain build-facts body — `Build:`, `Build-time:`,
//! `Copyright:`, `License:`, `Source:`. The build facts come from
//! [`crate::info::build`] (the Hinnant clock stamps them, no time
//! crate). Two surfaces share ONE body so they cannot drift:
//! `-V` / `--version` intercepted in `main.rs` renders the brand header
//! in purple on a TTY via [`print_version_report`], and clap's own
//! version rendering (every other path) carries the same lines through
//! [`long_version`].
//!
//! The help surface is the zelynic single-tier law (night-improve-15):
//! clap's auto help and its `help` subcommand are disabled — the only
//! help surface is the curated reference in [`crate::commands::help`],
//! dispatched from the [`Cli::help`] field. Subcommand-level `--help`
//! is an unknown-argument error, never a second help wall.
//!
//! Fatal parse errors render through [`ux`] (night-improve-19, the
//! zelynic UX bridge lineage): brand-styled, one canonical footer,
//! the case-insensitive typo rescue behind clap's own engine.

use clap::{Parser, Subcommand};
use std::sync::OnceLock;

pub mod styles;
pub mod suggestion;
pub mod ux;

/// The one-line tagline under the version header — the same string the
/// clap `about` surface carries, kept as a const so the report header
/// and the help header can never drift.
const ABOUT: &str = "Real-time mechanical keyboard acoustic synthesizer for Linux";

/// The plain-text body of the version report (everything after the
/// brand header). Separated from [`long_version`] /
/// [`print_version_report`] so tests can assert the full line set
/// without capturing output.
///
/// `Build-time:` is stamped by `build.rs` via Howard Hinnant's
/// civil_from_days algorithm — `M/D/YYYY HH:MM (UTC)` — so no time
/// crate is needed anywhere in the dependency tree (night-improve-13).
fn version_body() -> String {
    format!(
        "Build: {} ({})\n\
         Build-time: {}\n\
         Copyright: (c) 2026 rezky_nightky (oxyzenQ)\n\
         License: GPL-3.0-only\n\
         Source: https://github.com/oxyzenQ/zylaxion",
        crate::info::build::build_label(),
        crate::info::build::build_hash(),
        crate::info::build::build_time(),
    )
}

/// The full multi-line version string clap renders for its own version
/// path (e.g. a `--verbose -V` fallthrough): the brand header plus the
/// shared body. Plain text — clap paints it through its own engine.
///
/// Built lazily at first access because clap's `version` attribute wants
/// a `&'static str` (clap's `Str` type optimizes for static strings and
/// does not take an owned `String` here), while the body needs runtime
/// `format!`.
static LONG_VERSION_CELL: OnceLock<String> = OnceLock::new();

fn long_version() -> &'static str {
    LONG_VERSION_CELL
        .get_or_init(|| {
            format!(
                "zylaxion: v{}\n{}\n{}",
                crate::info::build::VERSION,
                ABOUT,
                version_body()
            )
        })
        .as_str()
}

/// Print the full version report for `-V` / `--version`.
///
/// zelynic masterclass layout: brand header (name + version, then the
/// one-line tagline), followed by the plain build-facts body. The header
/// is rendered in brand purple `#A855F7` bold on a TTY (the same palette
/// and capability gate the error lanes use, stdout-scoped); when piped
/// (non-TTY) everything is plain text so ANSI codes never leak into
/// scripts or log files. The body stays on plain lines so it is easy to
/// grep from scripts (`zylaxion -V | grep Build-time`).
pub fn print_version_report() {
    let header = format!("zylaxion: v{}\n{}", crate::info::build::VERSION, ABOUT);
    crate::output::print_stdout_brand(&header);
    crate::output::print_stdout_line(&version_body());
}

#[cfg(test)]
#[path = "../../test/cli/version_report_tests.rs"]
mod tests;
/// zylaxion — real-time mechanical keyboard acoustic synthesizer for Linux.
///
/// The `styles` attribute wires the brand palette (purple headings,
/// grey grammar tier for every flag spelling and command synopsis) into
/// every surface clap renders — see `cli_styles` for the contract.
/// The help flag is disabled here and re-declared as the [`Cli::help`]
/// field below: one curated reference, the zelynic single-tier law.
#[derive(Parser)]
#[command(
    name = "zylaxion",
    version = long_version(),
    about = ABOUT,
    long_about = None,
    disable_help_flag = true,
    disable_help_subcommand = true,
    styles = crate::cli::styles::styles()
)]
pub struct Cli {
    /// Print the end-to-end reference (commands, examples, flags)
    ///
    /// Single-tier help surface (night-improve-15): the former clap
    /// auto-help and its `help` subcommand are disabled; this field is
    /// the only help surface, intercepted in `main` to print the
    /// curated reference from `commands::help`. At subcommand level
    /// (`zylaxion start --help`) there is no help flag at all — clap
    /// answers unknown-argument with a suggestion, exit 2.
    #[arg(short = 'h', long = "help", global = false)]
    pub help: bool,

    /// Check the latest upstream GitHub release
    ///
    /// Network surface (night-improve-16): the check shells out to
    /// curl, so it must never ride root privileges — `update::
    /// run_check_update` refuses euid 0 with a "do not use sudo" tip
    /// before any network I/O happens. Top-level only: under a
    /// subcommand this spelling is an unknown-argument error, which is
    /// the honest reading of `zylaxion start --check-update`.
    #[arg(long = "check-update", global = false)]
    pub check_update: bool,

    /// Live mode: refresh the overview every 2 seconds (Ctrl+C to exit)
    ///
    /// Only effective without a subcommand.
    #[arg(long)]
    pub live: bool,

    /// Without a subcommand: show a quick status overview (daemon
    /// state, active preset, audio device). With a subcommand: run
    /// that subcommand. (v10.2.0+ — user feedback)
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run in the foreground (press Ctrl+C to quit)
    Start {
        /// Select the acoustic preset by name (from the compiled
        /// registry). If omitted, the ZYLAXION_PRESET environment
        /// variable is used, then the compiled default.
        #[arg(long)]
        preset: Option<String>,
    },

    /// Run as a background daemon (controlled via Unix socket).
    Daemon {
        /// Select the acoustic preset by name (from the compiled
        /// registry). If omitted, the ZYLAXION_PRESET environment
        /// variable is used, then the compiled default.
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

    /// List available acoustic presets from the compiled registry
    /// (night-improve-22: no config file — the registry is the source).
    ListPresets,

    /// List available audio backends
    ListBackends,
}
