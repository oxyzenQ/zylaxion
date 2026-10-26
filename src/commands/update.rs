// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `zylaxion --check-update` subcommand handler.
//!
//! Extracted from `main.rs` in v10.2.0 (dragonzen audit I9) to keep
//! `main.rs` focused on bootstrap + wiring. The check-update logic is
//! ~210 LOC (network call, SemVer parsing, GitHub JSON extraction) —
//! substantial enough to deserve its own module.
//!
//! # Implementation notes
//!
//! Shells out to `curl` (pre-installed on ~99% of Linux distros) to
//! fetch the GitHub releases/latest JSON, then extracts the
//! `"tag_name"` field with a tiny string parser — no `serde_json`, no
//! `ureq`, no `rustls`, no `ring`, no `webpki`. This keeps the
//! dependency tree lean and eliminates the supply-chain surface area
//! of pulling in a TLS stack just to read one URL.
//!
//! Network errors, curl-not-installed, non-200 responses, and
//! malformed JSON are all reported gracefully — the command never
//! panics, only prints a human message and returns. The caller
//! (`main.rs`) exits 0 for those because this is an informational
//! flag, not a critical op. The one hard-exit surface is the
//! privilege guard (night-improve-16): under euid 0 the check
//! refuses with the "do not use sudo" verdict and exits 1 BEFORE
//! any network I/O — see [`crate::session::privilege::network_refusal`].

use std::process::Command;

use crate::info::version::{UpdateStatus, compare_versions, extract_tag_name, normalize_version};

/// GitHub API endpoint for the latest published release of `oxyzenQ/zylaxion`.
const GITHUB_API_URL: &str = "https://api.github.com/repos/oxyzenQ/zylaxion/releases/latest";

/// Human-readable releases URL (printed as the `Source:` line in output).
const RELEASES_URL: &str = "https://github.com/oxyzenQ/zylaxion/releases/latest";

/// Maximum seconds to wait for the GitHub API to respond.
const CHECK_UPDATE_TIMEOUT_SECS: u32 = 15;

/// Compile-time current package version, used to avoid hardcoding version
/// strings in the output (consistent with the version-anti-pattern rule).
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

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
/// human message and returns. The caller exits 0 for those (informational
/// flag). The euid-0 refusal exits 1 (night-improve-16, the module docs
/// carry the contract).
pub fn run_check_update() {
    // Privilege guard first (night-improve-16, the zelynic placement):
    // refuse euid 0 before any network I/O — this is the one command
    // where root is the hazard instead of the requirement. The refusal
    // is a usage violation, not an informational failure, so it exits
    // 1 (the informational failure paths below keep exit 0).
    if let Some(refusal) =
        crate::session::privilege::network_refusal(crate::session::privilege::euid_is_root())
    {
        crate::output::error(refusal);
        std::process::exit(1);
    }

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
                // night-improve-6: every failure rides the labeled lane.
                crate::output::error("update check failed: curl is not available on PATH");
            } else {
                crate::output::error(format!("update check failed: {e}"));
            }
            return;
        }
    };

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        crate::output::error(format!(
            "update check failed: {}",
            interpret_curl_exit(code)
        ));
        return;
    }

    let raw = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => {
            crate::output::error("update check failed: response was not valid UTF-8");
            return;
        }
    };

    let (body, status_str) = match raw.rsplit_once('\n') {
        Some(pair) => pair,
        None => {
            crate::output::error("update check failed: GitHub API response was malformed");
            return;
        }
    };
    let status: u16 = status_str.trim().parse().unwrap_or(0);
    if status != 200 {
        crate::output::error(format!(
            "update check failed: {}",
            interpret_http_status(status)
        ));
        return;
    }

    let latest_tag = match extract_tag_name(body) {
        Some(t) => t,
        None => {
            crate::output::error(
                "update check failed: could not parse latest release tag from GitHub response",
            );
            return;
        }
    };

    // The report opens with the family banner (the zelynic output
    // law, night-dragon-maintenance-1): the update check is a report
    // surface, so it reads as one report alongside the doctor and the
    // overview — one frame, one key/value geometry.
    // night-dragon-maintenance-4: the Status verdict rides its tier —
    // green for the affirmative up-to-date, yellow for a finding with
    // action implied (an update exists), grey for the passive
    // informational shape — instead of bare white.
    let status_colored = match compare_versions(CURRENT_VERSION, &latest_tag) {
        UpdateStatus::UpToDate => crate::output::ok_line("up to date"),
        UpdateStatus::UpdateAvailable => crate::output::warn_line("update available"),
        UpdateStatus::CurrentIsNewer => {
            crate::output::tip_line("current is newer than latest release")
        }
    };
    println!("{}\n", crate::output::section("zylaxion update check"));
    println!(
        "{}",
        crate::output::kv("Current:", &normalize_version(CURRENT_VERSION))
    );
    println!(
        "{}",
        crate::output::kv("Latest:", &normalize_version(&latest_tag))
    );
    println!("{}", crate::output::kv("Status:", &status_colored));
    println!("{}", crate::output::kv("Source:", RELEASES_URL));
}
