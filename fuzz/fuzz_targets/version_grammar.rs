// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Fuzz target: the version grammar (night-dragon-maintenance-5).
//!
//! The update check parses host-controlled text twice: the upstream
//! `tag_name` out of a GitHub JSON payload, and both tags through the
//! SemVer grammar before comparison. A malicious or corrupted payload
//! must answer Option/UpdateStatus, never a panic. The harness drives
//! the pure trio over arbitrary bytes - [`extract_tag_name`],
//! [`SemVer::parse`], [`normalize_version`], [`compare_versions`] -
//! including the hostile classes: unterminated strings, deep nesting
//!-shaped junk, `v` prefixes and their absence, pre-release suffixes,
//! four-field versions (the grammar's named None), and enormous
//! numeric fields.
//!
//! The source is path-included, not copied (see fuzz/Cargo.toml for
//! the full rationale): the harness compiles the same bytes the
//! binary ships, so the guarantee the lane proves is the guarantee
//! the binary carries.

#![no_main]

// The include law pulls the whole file; the harness exercises the
// grammar, not the network/CLI surface next to it. The allow rides
// the HARNESS module (never the shipped source).
#[allow(dead_code)]
#[path = "../../src/info/version.rs"]
mod version;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    // The tag extraction: Some or None, never a panic. A found tag
    // is always a real substring of the input.
    if let Some(tag) = version::extract_tag_name(text) {
        assert!(
            text.contains(&tag),
            "extract_tag_name invented a tag not present in the input"
        );
    }

    // The grammar: Some or None, never a panic. A parse that accepts
    // a four-field stable part is the named bug (the grammar returns
    // None when a fourth field exists). The pre-release split mirrors
    // the grammar's own order - strip 'v', then split_once('-') -
    // so a dot inside a pre-release suffix never fakes a fourth
    // stable field.
    let parsed_current = version::SemVer::parse(text);
    let trimmed = text.trim();
    let stable = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let stable = stable.split_once('-').map_or(stable, |(s, _)| s);
    if stable.split('.').count() > 3 {
        assert!(
            parsed_current.is_none(),
            "the grammar accepted a four-field version: {text}"
        );
    }

    // The comparison: total over the parseable domain - the verdict
    // against the same string is always UpToDate (the identity law).
    if parsed_current.is_some() {
        assert_eq!(
            version::compare_versions(text, text),
            version::UpdateStatus::UpToDate,
            "compare_versions(x, x) must be UpToDate for every parseable x: {text}"
        );
    }

    // The normalizer: idempotent, and its output always parses the
    // same as its input (the render law the report's rows ride).
    let normalized = version::normalize_version(text);
    assert_eq!(
        version::normalize_version(&normalized),
        normalized,
        "normalize_version must be idempotent: {text}"
    );
    assert_eq!(
        version::SemVer::parse(text),
        version::SemVer::parse(&normalized),
        "normalization must not change the parsed value: {text}"
    );
});
