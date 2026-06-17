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
        run_check_updated();
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

/// GitHub API endpoint for the latest published release of `oxyzenQ/zylaxion`.
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/oxyzenQ/zylaxion/releases/latest";

/// Subset of the GitHub `GET /releases/latest` JSON payload that we care about.
#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
}

/// Implements `zylaxion --check-updated`.
///
/// Performs an HTTP GET against the GitHub API for the latest published
/// release of `oxyzenQ/zylaxion`, parses the `tag_name` field, and compares
/// it against the current crate version (prefixed with `v` to match
/// GitHub's `vX.Y.Z` release-tag convention).
///
/// Output format:
///   - On latest:  `You are running the latest version (vX.Y.Z).`
///   - Behind:     `Update available: <tag_name>. Please check https://github.com/oxyzenQ/zylaxion/releases.`
///   - On error:   `Failed to check for updates: <error>.`
///
/// Network errors, non-200 responses, and JSON decode failures are all
/// reported gracefully — the command never panics, only prints a human
/// message and exits 0 (this is an informational flag, not a critical op).
fn run_check_updated() {
    let current = format!("v{}", env!("CARGO_PKG_VERSION"));

    println!("Checking for updates...");

    let release = match fetch_latest_release() {
        Ok(r) => r,
        Err(err) => {
            println!("Failed to check for updates: {}.", err);
            return;
        }
    };

    if release.tag_name == current {
        println!("You are running the latest version ({}).", current);
    } else {
        println!(
            "Update available: {}. Please check https://github.com/oxyzenQ/zylaxion/releases.",
            release.tag_name
        );
    }
}

/// Fetches and decodes the latest release payload from the GitHub API.
///
/// Uses `ureq` with a 5-second timeout and a custom `User-Agent` (GitHub
/// rejects requests without one). Returns an `anyhow`-free error string
/// suitable for direct display to the user.
fn fetch_latest_release() -> Result<GithubRelease, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let response = agent
        .get(LATEST_RELEASE_URL)
        .set(
            "User-Agent",
            &format!("zylaxion/{}", env!("CARGO_PKG_VERSION")),
        )
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    response
        .into_json::<GithubRelease>()
        .map_err(|e| format!("failed to decode release payload: {e}"))
}
