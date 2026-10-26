// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The version grammar: SemVer parsing, the update verdict, the
//! tag-name extraction. Pure string math, std-only, zero crate-root
//! references - the fuzz lane path-includes this file, so the
//! harness compiles the exact bytes the binary ships (the include
//! law: no crate-root paths in an included file, the rule the fuzz
//! workflow's precheck greps for).

/// Result of comparing the running version against the latest
/// upstream tag.
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    UpToDate,
    UpdateAvailable,
    CurrentIsNewer,
}

/// Minimal SemVer (major.minor.patch) for version comparison.
/// Pre-release suffixes are stripped before comparison.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
}

impl SemVer {
    pub fn parse(version: &str) -> Option<Self> {
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
pub fn normalize_version(version: &str) -> String {
    let version = version.trim();
    if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

/// Compare two version strings, returning the update status.
pub fn compare_versions(current: &str, latest: &str) -> UpdateStatus {
    match (SemVer::parse(current), SemVer::parse(latest)) {
        (Some(current), Some(latest)) if current == latest => UpdateStatus::UpToDate,
        (Some(current), Some(latest)) if current > latest => UpdateStatus::CurrentIsNewer,
        _ => UpdateStatus::UpdateAvailable,
    }
}

/// Extract the `"tag_name"` value from a GitHub releases JSON payload using
/// plain string matching — no JSON parser dependency required.
pub fn extract_tag_name(json: &str) -> Option<String> {
    const KEY: &str = "\"tag_name\"";
    let rest = json.get(json.find(KEY)? + KEY.len()..)?;
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
