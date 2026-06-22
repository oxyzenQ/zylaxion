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
mod error_format;
mod instance_lock;
mod signals;

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
        cli::Commands::Daemon { preset, foreground } => {
            commands::daemon::cmd_daemon(preset, foreground)
        }
        cli::Commands::Stop => commands::daemon::cmd_stop(),
        cli::Commands::Status => daemon::cmd_status(),
        cli::Commands::Doctor => commands::info::cmd_doctor(),
        cli::Commands::Testconf => commands::info::cmd_testconf(),
        cli::Commands::ListPresets => commands::info::cmd_list_presets(),
        cli::Commands::ListBackends => commands::info::cmd_list_backends(),
    }
}

/// GitHub API endpoint for the latest published release of `oxyzenQ/zylaxion`.
const GITHUB_API_URL: &str = "https://api.github.com/repos/oxyzenQ/zylaxion/releases/latest";

/// Human-readable releases URL (printed as the `Source:` line in output).
const RELEASES_URL: &str = "https://github.com/oxyzenQ/zylaxion/releases/latest";

/// Maximum seconds to wait for the GitHub API to respond.
const CHECK_UPDATE_TIMEOUT_SECS: u32 = 15;

/// Compile-time current package version, used to avoid hardcoding version
/// strings in the output (consistent with the version-anti-pattern rule).
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Result of comparing the running version against the latest upstream tag.
#[derive(Debug, PartialEq, Eq)]
enum UpdateStatus {
    UpToDate,
    UpdateAvailable,
    CurrentIsNewer,
}

/// Minimal SemVer (major.minor.patch) for version comparison.
/// Pre-release suffixes are stripped before comparison.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
}

impl SemVer {
    fn parse(version: &str) -> Option<Self> {
        let version = version.trim();
        let version = version.strip_prefix('v').unwrap_or(version);
        let version = version
            .split_once('-')
            .map_or(version, |(stable, _)| stable);
        let mut parts = version.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

/// Ensure a version string has exactly one leading `v`.
fn normalize_version(version: &str) -> String {
    let version = version.trim();
    if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

/// Compare two version strings, returning the update status.
fn compare_versions(current: &str, latest: &str) -> UpdateStatus {
    match (SemVer::parse(current), SemVer::parse(latest)) {
        (Some(current), Some(latest)) if current == latest => UpdateStatus::UpToDate,
        (Some(current), Some(latest)) if current > latest => UpdateStatus::CurrentIsNewer,
        _ => UpdateStatus::UpdateAvailable,
    }
}

/// Extract the `"tag_name"` value from a GitHub releases JSON payload using
/// plain string matching — no JSON parser dependency required.
fn extract_tag_name(json: &str) -> Option<String> {
    const KEY: &str = "\"tag_name\"";
    let rest = json.get(json.find(KEY)? + KEY.len()..)?;
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Map curl's exit code to a human-readable message.
fn interpret_curl_exit(code: i32) -> &'static str {
    match code {
        6 => "DNS resolution failed",
        7 => "connection refused",
        28 => "network request timed out",
        35 => "SSL/TLS handshake failed",
        _ => "network request failed",
    }
}

/// Map HTTP status code from the GitHub API to a human-readable message.
fn interpret_http_status(code: u16) -> &'static str {
    match code {
        403 => "GitHub API request was rate-limited or forbidden",
        404 => "no latest GitHub release found for oxyzenQ/zylaxion",
        _ => "GitHub API returned an unexpected error",
    }
}

/// Implements `zylaxion --check-update`.
///
/// Shells out to `curl` (pre-installed on ~99% of Linux distros) to fetch the
/// GitHub releases/latest JSON, then extracts the `"tag_name"` field with a
/// tiny string parser — no `serde_json`, no `ureq`, no `rustls`, no `ring`,
/// no `webpki`. This keeps the dependency tree lean and eliminates the supply
/// chain surface area of pulling in a TLS stack just to read one URL.
///
/// Output format (matches the oxyzenQ ecosystem standard):
///
/// ```text
/// zylaxion update check
/// Current: vX.Y.Z
/// Latest:  vX.Y.Z
/// Status:  up to date
/// Source:  https://github.com/oxyzenQ/zylaxion/releases/latest
/// ```
///
/// Network errors, curl-not-installed, non-200 responses, and malformed JSON
/// are all reported gracefully — the command never panics, only prints a
/// human message and exits 0 (this is an informational flag, not a critical op).
fn run_check_update() {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--max-time",
            &CHECK_UPDATE_TIMEOUT_SECS.to_string(),
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            &format!("User-Agent: zylaxion/{}", CURRENT_VERSION),
            "--write-out",
            "\n%{http_code}",
            GITHUB_API_URL,
        ])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!("zylaxion update check failed: curl is not available on PATH");
            } else {
                eprintln!("zylaxion update check failed: {e}");
            }
            return;
        }
    };

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        eprintln!(
            "zylaxion update check failed: {}",
            interpret_curl_exit(code)
        );
        return;
    }

    let raw = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("zylaxion update check failed: response was not valid UTF-8");
            return;
        }
    };

    let (body, status_str) = match raw.rsplit_once('\n') {
        Some(pair) => pair,
        None => {
            eprintln!("zylaxion update check failed: GitHub API response was malformed");
            return;
        }
    };
    let status: u16 = status_str.trim().parse().unwrap_or(0);
    if status != 200 {
        eprintln!(
            "zylaxion update check failed: {}",
            interpret_http_status(status)
        );
        return;
    }

    let latest_tag = match extract_tag_name(body) {
        Some(t) => t,
        None => {
            eprintln!("zylaxion update check failed: could not parse latest release tag from GitHub response");
            return;
        }
    };

    let status_text = match compare_versions(CURRENT_VERSION, &latest_tag) {
        UpdateStatus::UpToDate => "up to date",
        UpdateStatus::UpdateAvailable => "update available",
        UpdateStatus::CurrentIsNewer => "current is newer than latest release",
    };

    println!("zylaxion update check");
    println!("Current: {}", normalize_version(CURRENT_VERSION));
    println!("Latest:  {}", normalize_version(&latest_tag));
    println!("Status:  {status_text}");
    println!("Source:  {RELEASES_URL}");
}
