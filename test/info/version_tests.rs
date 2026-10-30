// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// version_tests.rs — the info/version.rs test pins
// (night-dragon-maintenance-T1, the zelynic Pattern C test tree:
// every *.rs test file lives under the crate's test/ directory,
// wired via #[path]). The grammar triple (SemVer::parse,
// normalize_version, compare_versions) rides the fuzz lane's
// no-panic proof; THIS file adds the deterministic pins the fuzz
// lane cannot make: the canonical GitHub payload parses to the
// exact tag, and the hostile-but-structured shapes answer None
// instead of a wrong tag.

use super::UpdateStatus::{CurrentIsNewer, UpToDate, UpdateAvailable};
use super::*;

/// The canonical `releases/latest` payload shape GitHub answers a
/// 200 with — nested objects before the field, the usual metadata
/// rows around it. The one extraction the `--check-update` lane
/// lives on must return the bare tag string.
#[test]
fn extract_tag_name_parses_the_canonical_release_payload() {
    let payload = r#"{
  "url": "https://api.github.com/repos/oxyzenQ/zylaxion/releases/1",
  "tag_name": "v80.0.2",
  "target_commitish": "main",
  "name": "v80.0.2",
  "author": { "login": "oxyzenQ", "type": "User" },
  "assets": [ { "name": "zylaxion-v80.0.2-linux-amd64-v3-gnu.tar.gz" } ],
  "prerelease": false
}"#;
    assert_eq!(
        extract_tag_name(payload).as_deref(),
        Some("v80.0.2"),
        "the canonical payload must yield the bare tag"
    );
}

/// Field order in the payload is not contractual: `tag_name` may sit
/// anywhere in the object — the parse anchors on the key, not on
/// byte offsets.
#[test]
fn extract_tag_name_tolerates_field_order() {
    let payload = r#"{"name":"zylaxion v81.0.0","tag_name":"v81.0.0","draft":false}"#;
    assert_eq!(extract_tag_name(payload).as_deref(), Some("v81.0.0"));
}

/// Compact single-line payloads (proxies, cached responses) parse
/// the same as the pretty-printed canonical shape.
#[test]
fn extract_tag_name_parses_the_compact_payload() {
    assert_eq!(
        extract_tag_name(r#"{"tag_name":"v80.0.1"}"#).as_deref(),
        Some("v80.0.1")
    );
}

/// An untagged draft, a rate-limit body, an error object, and an
/// empty string all answer None — the caller renders its
/// "could not parse" verdict instead of a wrong tag.
#[test]
fn extract_tag_name_rejects_payloads_without_the_key() {
    for payload in [
        "",
        "{}",
        r#"{"message":"API rate limit exceeded"}"#,
        r#"{"tagname":"v80.0.2"}"#,
        r#"{"target_commitish":"main"}"#,
    ] {
        assert!(
            extract_tag_name(payload).is_none(),
            "expected None for: {payload}"
        );
    }
}

/// Structured-but-broken values answer None, never a partial tag:
/// the unterminated string (a truncated download) and the
/// non-string value both fall through.
#[test]
fn extract_tag_name_rejects_broken_values() {
    for payload in [
        r#"{"tag_name":"v80.0.2"#,
        r#"{"tag_name":8002}"#,
        r#"{"tag_name":}"#,
        r#"{"tag_name": null}"#,
    ] {
        assert!(
            extract_tag_name(payload).is_none(),
            "expected None for: {payload}"
        );
    }
}

/// The extraction returns the RAW tag string — whitespace inside the
/// quotes is the tag's own problem (GitHub tags never carry it), so
/// the SemVer layer downstream sees exactly what the API sent.
#[test]
fn extract_tag_name_returns_the_raw_quoted_span() {
    assert_eq!(
        extract_tag_name(r#"{"tag_name":"v80.0.0-beta.1"}"#).as_deref(),
        Some("v80.0.0-beta.1"),
        "pre-release suffixes ride through untouched"
    );
}

/// The SemVer comparison verdicts — the three-way contract the
/// Status: line renders — stay exact at the boundaries.
#[test]
fn compare_versions_answers_the_three_verdicts() {
    assert_eq!(
        compare_versions("80.0.1", "v80.0.1"),
        UpToDate,
        "identical versions with mixed v-prefix are up to date"
    );
    assert_eq!(
        compare_versions("80.0.1", "v80.0.2"),
        UpdateAvailable,
        "a newer patch is an update"
    );
    assert_eq!(
        compare_versions("80.0.2", "v80.0.1"),
        CurrentIsNewer,
        "a locally newer build says so honestly"
    );
    assert_eq!(
        compare_versions("80.0.1", "v80.1.0"),
        UpdateAvailable,
        "a newer minor is an update"
    );
}

/// Pre-release suffixes never flip the verdict: a beta of the same
/// stable triple is the same version for comparison purposes (the
/// suffix is stripped before the numeric compare).
#[test]
fn compare_versions_strips_pre_release_suffixes() {
    assert_eq!(
        compare_versions("80.0.0-beta.1", "v80.0.0"),
        UpToDate,
        "the beta suffix strips before comparison"
    );
}
