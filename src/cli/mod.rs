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

/// The one-line tagline under the version header — the Cargo.toml
/// `description` itself, pulled through the compile-time environment
/// (night-hunter-2, the zelynic family contract: `about =
/// env!("CARGO_PKG_DESCRIPTION")`). One string, two consumers — the
/// version report and the help banner — and neither can drift from
/// the registry-facing description a `cargo search` shows.
pub(crate) const ABOUT: &str = env!("CARGO_PKG_DESCRIPTION");

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
/// The help flag is disabled here and redeclared as the [`Cli::help`]
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

    /// Without a subcommand: print the quick overview (kernel
    /// verdict, tap residency, active posture, audio device) — a
    /// static print-once render; the deep read is `status --verbose`.
    /// With a subcommand: run that subcommand. (v10.2.0+ — user
    /// feedback; night-improve-24: the `--live` refresh loop is
    /// deleted — live output was the wrong shape for a status
    /// surface).
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Turn the keyboard sound on (spawns the stateless player).
    ///
    /// Requires the tap attached (`sudo zylaxion attach`, once per
    /// boot). The preset resolves from this flag, the ZYLAXION_PRESET
    /// environment variable, then the compiled default (night-improve-22,
    /// the no-config law).
    On {
        /// Select the acoustic preset by name (from the compiled
        /// registry). If omitted, the ZYLAXION_PRESET environment
        /// variable is used, then the compiled default.
        #[arg(long)]
        preset: Option<String>,
    },

    /// Turn the keyboard sound off — the player self-exits, zero
    /// processes remain (the tap stays attached; `detach` removes it).
    Off,

    /// Switch the acoustic preset live (one map write against the
    /// pinned control plane).
    Preset {
        /// The preset name to switch to (see `zylaxion list-presets`).
        /// night-improve-24: renamed from `--name` — the verb switches,
        /// the flag should say so.
        #[arg(long, short)]
        switch: String,
    },

    /// Set the master volume as a percent of the compiled default
    /// (100 = the laptop-speaker-tuned 5.5x gain).
    Volume {
        /// Volume percent, 0 (silence) to 200 (the night-improve-27
        /// safety cap). Above 200 the session gate refuses — values
        /// that loud can damage speakers — unless `--force-this`
        /// rides along, which lifts the ceiling to 10000 (the force
        /// cap; anything past it is refused outright).
        #[arg(long, short)]
        percent: u32,
        /// Override the 200% safety cap, up to the 10000 force
        /// ceiling (night-improve-27). Above 10000 nothing passes.
        #[arg(long)]
        force_this: bool,
    },

    /// Show the one-screen truth: kernel verdict, tap residency,
    /// player state, active posture — the master status report
    /// (night-improve-24: the former `stats` verb merged in; the
    /// deep read is `--verbose`).
    Status {
        /// Also print the kernel-side typing telemetry — the per-key
        /// counters the tap accumulated since attach, at zero
        /// userspace cost (the merged `stats` surface). Static by
        /// design: prints once and exits, never a live refresh.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Print system health diagnostic
    Doctor,

    /// Load and pin the eBPF tap (the kernel-side control plane).
    /// Run under sudo — the one privileged surface; the pins are handed
    /// to your user afterwards (night-improve-23).
    Attach,

    /// Unpin everything — after this, nothing of zylaxion remains in
    /// the kernel (night-improve-23).
    Detach,

    /// The internal stateless player (night-improve-23). Hidden: users
    /// start sound with `on`; this verb is the process `on` execs. It
    /// reads every byte of state from the pinned maps and stops itself
    /// when the control map clears the enabled flag.
    ///
    /// The explicit `name = "_play"` is load-bearing (night-improve-25,
    /// the hunt find under the Pro mode work): clap derives `play` from
    /// the bare variant, but every call site spells the verb with the
    /// underscore — sound.rs spawns `.arg("_play")`, its startup-error
    /// strings teach `zylaxion _play`, and player.rs names it the same.
    /// Before the pin the spawn was a verb clap rejects ("unrecognized
    /// subcommand '_play'"), so `zylaxion on` could never start the
    /// player — masked until now because the broken attach path sat
    /// upstream of the spawn. The underscore prefix is the internal-verb
    /// convention; the parse pin in test/cli/ux_tests.rs holds it shut.
    #[command(hide = true, name = "_play")]
    Play,

    /// List available acoustic presets from the compiled registry
    /// (night-improve-22: no config file — the registry is the source).
    ListPresets,

    /// List available audio backends
    ListBackends,
}
