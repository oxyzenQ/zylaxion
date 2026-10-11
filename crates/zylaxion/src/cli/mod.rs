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

use clap::{Parser, Subcommand};
use std::sync::OnceLock;

pub mod styles;

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
mod tests {
    use super::*;

    /// night-improve-13 contract: the version report carries every
    /// documented line exactly once, in the zelynic reference order —
    /// Build, then Build-time directly after it, then Copyright,
    /// License, Source.
    #[test]
    fn version_report_carries_the_zelynic_line_contract() {
        let body = version_body();
        for line in [
            "Copyright: (c) 2026 rezky_nightky (oxyzenQ)",
            "License: GPL-3.0-only",
            "Source: https://github.com/oxyzenQ/zylaxion",
        ] {
            let count = body.lines().filter(|l| l.contains(line)).count();
            assert_eq!(count, 1, "line must appear exactly once: {line}");
        }
        // Ordering: Build-time sits directly after the Build line.
        let lines: Vec<&str> = body.lines().collect();
        let build_idx = lines
            .iter()
            .position(|l| l.starts_with("Build: "))
            .expect("Build: line present");
        assert!(
            lines[build_idx + 1].starts_with("Build-time: "),
            "Build-time must directly follow Build: {lines:?}"
        );
    }

    /// The Build-time stamp carries the Hinnant shape
    /// `M/D/YYYY HH:MM (UTC)` (or the documented "unknown" fallback) —
    /// never an empty string or a foreign format.
    #[test]
    fn build_time_stamp_shape_is_m_d_yyyy_hh_mm_utc() {
        let line = version_body()
            .lines()
            .find(|l| l.starts_with("Build-time: "))
            .expect("Build-time line present")
            .trim_start_matches("Build-time: ")
            .to_string();
        if line == "unknown" {
            return; // degraded path, documented
        }
        assert!(line.ends_with(" (UTC)"), "shape: {line}");
        let body = line.trim_end_matches(" (UTC)");
        let (date, time) = body.split_once(' ').expect("M/D/YYYY HH:MM split");
        assert_eq!(date.matches('/').count(), 2, "M/D/YYYY shape: {date}");
        assert_eq!(time.matches(':').count(), 1, "HH:MM shape: {time}");
    }

    /// The clap surface and the intercepted surface share ONE body —
    /// both carry the Build facts, so the two paths can never disagree
    /// about provenance.
    #[test]
    fn long_version_embeds_the_shared_body() {
        let body = version_body();
        for line in body.lines() {
            assert!(
                long_version().contains(line),
                "clap surface must embed the shared body line: {line}"
            );
        }
    }
}

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
        ///
        /// If omitted, searches the standard config paths.
        #[arg(long = "file", short = 'f')]
        file: Option<String>,
    },

    /// List available acoustic presets from config.toml
    ListPresets,

    /// List available audio backends
    ListBackends,
}
