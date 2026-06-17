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
//! - [`config`]    — filesystem search for the central `config.toml`
//! - [`commands`]  — subcommand handlers (`start`, `daemon`, `stop`, `doctor`, …)
//! - [`daemon`]    — POSIX daemonization, PID file, signal handling, IPC

use clap::Parser;
use std::process::Command;

mod cli;
mod commands;
mod config;
mod daemon;
mod instance_lock;

fn main() {
    // Early-exit flags — intercepted BEFORE `Cli::parse()` to bypass Clap's
    // subcommand requirement. This is the bulletproof way to handle early-exit
    // flags without fighting the parser: `zylaxion --check-update` (with no
    // subcommand) would otherwise error out before the flag is ever processed.
    if std::env::args().any(|a| a == "--check-update") {
        run_check_update();
        std::process::exit(0);
    }

    let cli = cli::Cli::parse();

    // Wire `--verbose` to env_logger via RUST_LOG. We set the env var here so
    // that both `cmd_start` (which calls `env_logger::init()`) and `cmd_daemon`
    // (which uses `Builder::from_env`) pick up the requested level. Default to
    // `info`; `--verbose` upgrades to `debug`.
    //
    // Safety: set_var is process-wide and we are still single-threaded here.
    // Subcommand handlers must not race this — they all run on the main thread
    // (or, in `cmd_daemon`, after `daemonize()` which preserves env vars).
    if cli.verbose {
        std::env::set_var("RUST_LOG", "debug");
    } else {
        std::env::set_var("RUST_LOG", "info");
    }

    match cli.command {
        cli::Commands::Start { preset } => commands::daemon::cmd_start(preset),
        cli::Commands::Daemon { preset } => commands::daemon::cmd_daemon(preset),
        cli::Commands::Stop => commands::daemon::cmd_stop(),
        cli::Commands::Status => daemon::cmd_status(),
        cli::Commands::Doctor => commands::info::cmd_doctor(),
        cli::Commands::Testconf => commands::info::cmd_testconf(),
        cli::Commands::ListBackends => commands::info::cmd_list_backends(),
    }
}

/// GitHub API endpoint for the latest published release of `oxyzenQ/zylaxion`.
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/oxyzenQ/zylaxion/releases/latest";

/// Maximum seconds to wait for the GitHub API to respond.
const CHECK_UPDATE_TIMEOUT_SECS: u32 = 5;

/// Implements `zylaxion --check-update`.
///
/// Shells out to `curl` (pre-installed on ~99% of Linux distros) to fetch the
/// GitHub releases/latest JSON, then extracts the `"tag_name"` field with a
/// tiny string parser — no `serde_json`, no `ureq`, no `rustls`, no `ring`,
/// no `webpki`. This keeps the dependency tree lean and eliminates the supply
/// chain surface area of pulling in a TLS stack just to read one URL.
///
/// The fetched `tag_name` is compared against the current crate version
/// (prefixed with `v` to match GitHub's `vX.Y.Z` release-tag convention).
///
/// Output format:
///   - On latest:  `You are running the latest version (vX.Y.Z).`
///   - Behind:     `Update available: <tag_name>. Please check https://github.com/oxyzenQ/zylaxion/releases.`
///   - On error:   `Failed to check for updates: curl is not installed or network error.`
///
/// Network errors, curl-not-installed, non-200 responses, and malformed JSON
/// are all reported gracefully — the command never panics, only prints a
/// human message and exits 0 (this is an informational flag, not a critical op).
fn run_check_update() {
    let current = format!("v{}", env!("CARGO_PKG_VERSION"));

    println!("Checking for updates...");

    let body = match fetch_latest_release_body() {
        Ok(b) => b,
        Err(err) => {
            println!("Failed to check for updates: {err}");
            return;
        }
    };

    let tag = match extract_tag_name(&body) {
        Some(t) => t,
        None => {
            println!("Failed to check for updates: malformed GitHub response.");
            return;
        }
    };

    if tag == current {
        println!("You are running the latest version ({current}).");
    } else {
        println!(
            "Update available: {tag}. Please check https://github.com/oxyzenQ/zylaxion/releases."
        );
    }
}

/// Runs `curl -s --max-time <N> -H <UA> -H <Accept> <URL>` and returns the
/// response body as a string.
///
/// Returns a single human-readable error string covering all failure modes:
///   - `curl` binary not found (most common — fresh minimal containers).
///   - curl exited non-zero (network error, 404, 403 rate-limit, etc.).
///   - curl produced non-UTF-8 bytes (shouldn't happen for JSON, but be safe).
fn fetch_latest_release_body() -> Result<String, String> {
    let user_agent = format!("zylaxion/{}", env!("CARGO_PKG_VERSION"));

    let output = Command::new("curl")
        // -sS: silent (no progress bar) but still show errors on stderr.
        // Plain -s would swallow curl's own diagnostic lines, leaving us
        // unable to tell "curl binary missing" from "HTTP 403 rate-limit".
        .arg("-sS")
        // Fail fast on HTTP 4xx/5xx so the user sees the real status code
        // (e.g. 403 rate-limit, 404 no-releases-yet) instead of a generic
        // "malformed response" message. curl exits 22 on HTTP errors and
        // writes a human-readable line to stderr, which we surface verbatim.
        .arg("-f")
        .arg("--max-time")
        .arg(CHECK_UPDATE_TIMEOUT_SECS.to_string())
        .arg("-H")
        .arg(user_agent)
        .arg("-H")
        .arg("Accept: application/vnd.github+json")
        .arg(LATEST_RELEASE_URL)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "curl is not installed or network error.".to_string()
            } else {
                format!("failed to invoke curl: {e}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_trim = stderr.trim();
        if stderr_trim.is_empty() {
            return Err("curl is not installed or network error.".to_string());
        }
        return Err(format!("curl failed: {stderr_trim}"));
    }

    String::from_utf8(output.stdout).map_err(|e| format!("curl returned non-UTF-8 body: {e}"))
}

/// Extracts the `"tag_name"` value from a GitHub releases JSON payload using
/// plain string matching — no JSON parser dependency required.
///
/// Handles both compact (`"tag_name":"v0.1.0"`) and pretty-printed
/// (`"tag_name": "v0.1.0"`) JSON. Tag values are simple identifiers like
/// `v0.1.0` and never contain escape sequences, so naive quote-pair matching
/// is sufficient.
///
/// Returns `None` if the key isn't found or the value isn't a quoted string.
fn extract_tag_name(body: &str) -> Option<String> {
    const KEY: &str = "\"tag_name\"";

    let key_pos = body.find(KEY)?;
    let after_key = &body[key_pos + KEY.len()..];

    // Skip optional whitespace, expect a colon, skip optional whitespace.
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_quote = after_colon.trim_start().strip_prefix('"')?;

    // Find the closing quote — tag names like `v0.1.0` have no escapes.
    let end = after_quote.find('"')?;
    Some(after_quote[..end].to_string())
}
