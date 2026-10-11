// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// build_tests.rs — the info/build.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

/// Fallback contract: in a plain `cargo test` build ZYLAXION_BUILD is
/// not forwarded by build.rs, so the canonical label must equal the
/// compile-time arch+libc detection. The override path (CI builds
/// reporting the release-matrix platform id) is verified live by
/// reading `zylaxion -V` on such a build.
#[test]
fn build_label_falls_back_to_detected_target_when_env_unset() {
    match option_env!("ZYLAXION_BUILD") {
        Some(_) => { /* CI/override path, verified live */ }
        None => assert_eq!(build_label(), build_string()),
    }
}

/// The detected fallback label must stay lowercase and carry the
/// dash-joined single-token `linux-<arch>-<libc>` shape the docs
/// promise, so a drift in build_target()/build_string() cannot
/// silently reshape the Build: line — and it must never double the
/// OS prefix (the `linux-linux-` regression the -V smoke caught).
#[test]
fn detected_build_label_shape_is_os_arch_libc() {
    let label = build_string();
    assert!(label.contains('-'), "label must be dash-joined: {label}");
    assert!(
        !label.contains(char::is_whitespace),
        "label must be a single token: {label}"
    );
    assert_eq!(label, label.to_lowercase());
    assert_eq!(
        label.matches("linux").count(),
        1,
        "OS prefix must appear exactly once: {label}"
    );
    assert!(
        !build_target().starts_with("linux-"),
        "build_target is the arch-libc token, OS-free: {}",
        build_target()
    );
}

/// The BUILD block's rustc fact is the full "rustc X.Y.Z (...)" shape
/// (or the documented degraded "unknown") — never empty, never a bare
/// version number, so the provenance line stays machine-parseable.
#[test]
fn rustc_version_is_the_full_shape_or_unknown() {
    let v = rustc_version();
    if v == "unknown" {
        return; // degraded path, documented
    }
    assert!(v.starts_with("rustc "), "must lead with the tool name: {v}");
    assert!(v.contains('('), "must carry the hash-date suffix: {v}");
}

/// The profile verdict states the release contract ONLY for release
/// builds — every other mode says what it is and explicitly declines
/// the contract. A debug build claiming "lto=thin" would be the
/// provenance lie the claims discipline forbids; a release build
/// missing the contract would be silence where the owner asked for
/// completeness.
#[test]
fn profile_line_claims_the_release_contract_only_in_release() {
    let line = profile_line();
    match profile_mode() {
        "release" => {
            assert!(line.starts_with("release, opt-level="), "got: {line}");
            for fact in ["lto=thin", "codegen-units=1", "panic=unwind", "strip=yes"] {
                assert!(line.contains(fact), "release must state `{fact}`: {line}");
            }
        }
        other => {
            assert!(line.contains(other), "the mode must be stated: {line}");
            assert!(
                !line.contains("lto=thin"),
                "a non-release build must not claim the release contract: {line}"
            );
        }
    }
}

/// Drift guard: RELEASE_PROFILE_CONTRACT is a hand-pinned literal, so
/// a future Cargo.toml profile change MUST land here too or this test
/// fails — the doctor reports the manifest's truth, never a stale
/// memory of it. The section slice is everything between
/// `[profile.release]` and the next section header; the tokens are
/// the exact `key = value` shapes the manifest writes.
#[test]
fn release_profile_consts_pin_the_manifest_contract() {
    let manifest = include_str!("../../Cargo.toml");
    let profile_section = manifest
        .split("[profile.release]")
        .nth(1)
        .expect("[profile.release] section exists in the root manifest")
        .split('[')
        .next()
        .unwrap_or_default();
    for pinned in [
        "opt-level = 3",
        "debug = false",
        "strip = true",
        "lto = \"thin\"",
        "codegen-units = 1",
        "panic = \"unwind\"",
    ] {
        assert!(
            profile_section.contains(pinned),
            "the release-profile contract drifted: [profile.release] no longer carries `{pinned}` — update RELEASE_PROFILE_CONTRACT in lockstep"
        );
    }
}

/// The BUILD facts block is complete — every field populated (the
/// degraded "unknown" lanes are honest values, never empty strings),
/// and the version fact is the raw semver (no "v" prefix — the human
/// report adds the brand prefix).
#[test]
fn build_info_is_complete_with_raw_semver() {
    let info = BuildInfo::collect();
    for field in [
        &info.version,
        &info.git_sha,
        &info.variant,
        &info.build_time,
        &info.rustc_version,
        &info.profile,
    ] {
        assert!(!field.is_empty(), "no BUILD field may be empty: {info:?}");
    }
    assert!(
        !info.version.starts_with('v'),
        "raw semver, no brand prefix: {info:?}"
    );
    assert!(
        info.version.split('.').count() >= 3,
        "X.Y.Z shape (pre-release suffix allowed): {info:?}"
    );
}
