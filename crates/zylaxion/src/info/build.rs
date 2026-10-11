// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The build-facts lane — everything the binary knows about its own
//! construction (the zelynic `info/build.rs` lineage): the canonical
//! build label chain (build_target → build_string → build_label), the
//! git-sha and build-time stamps build.rs injects, and the BUILD facts
//! the doctor's lead block reports ([`BuildInfo`]).
//!
//! Every field is compile-time truth that already rode the binary —
//! assembly, not runtime probing. No new dependencies, no bloat: just
//! the facts an owner needs before trusting a downloaded binary.

/// `CARGO_PKG_VERSION` — the crate version this binary was cut from.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The rustc that compiled this binary, stamped by build.rs from the
/// `RUSTC` env cargo itself drives — "rustc 1.98.1 (48a229cea
/// 2026-09-01)" shape, or "unknown" only when the compiler could not
/// answer `--version` (which cannot produce a binary either).
fn rustc_version() -> &'static str {
    option_env!("ZYLAXION_RUSTC_VERSION").unwrap_or("unknown")
}

/// The cargo profile mode this binary was built through: "release" or
/// "debug", stamped by build.rs from cargo's own PROFILE env —
/// compile-time truth, never a runtime guess.
fn profile_mode() -> &'static str {
    option_env!("ZYLAXION_PROFILE").unwrap_or("unknown")
}

/// The actual optimization level cargo used: "3" for the shipping
/// release contract, "0" for development builds. Stamped by build.rs
/// from cargo's OPT_LEVEL env — the real number, so a RUSTFLAGS
/// override shows up honestly.
fn opt_level() -> &'static str {
    option_env!("ZYLAXION_OPT_LEVEL").unwrap_or("?")
}

/// The [profile.release] contract as ONE composed literal: the four
/// facts cargo exposes no env for, read off Cargo.toml's pinned section
/// and drift-guarded by the
/// `release_profile_consts_pin_the_manifest_contract` test below — if
/// the manifest changes, the test fails before the doctor can report a
/// stale contract. Rendered only for release builds; a debug build must
/// not claim the release contract (see [`profile_line`]).
const RELEASE_PROFILE_CONTRACT: &str = "lto=thin, codegen-units=1, panic=unwind, strip=yes";

/// The one-line profile verdict for the doctor's BUILD block. Release
/// builds state the full contract — mode, the actual opt-level cargo
/// used, then the pinned optimization facts. Every other mode (debug,
/// unknown) states what it is WITHOUT claiming the release contract: a
/// development build reporting "lto=thin" would be the exact provenance
/// lie the claims discipline forbids.
fn profile_line() -> String {
    match profile_mode() {
        "release" => format!(
            "release, opt-level={}, {RELEASE_PROFILE_CONTRACT}",
            opt_level()
        ),
        other => format!(
            "{other}, opt-level={} (not the release contract)",
            opt_level()
        ),
    }
}

/// The build facts the doctor reports — version, commit, variant, build
/// time, rustc, profile. Every field is compile-time truth assembled by
/// build.rs stamps (no runtime probing, no new dependencies).
#[derive(Debug, Clone)]
pub(crate) struct BuildInfo {
    /// The crate version this binary was cut from (raw semver, no "v" —
    /// the human report adds the brand prefix).
    pub version: String,
    /// The commit this binary was built from (short git sha, or
    /// "unknown" only when git was unavailable outside a work tree).
    pub git_sha: String,
    /// The canonical build label — the `ZYLAXION_BUILD` env stamp
    /// (the release-matrix platform id, e.g. "linux-amd64-v3-gnu") or
    /// the detected arch+libc fallback for plain builds.
    pub variant: String,
    /// The UTC build timestamp (Hinnant algorithm, no time crate).
    pub build_time: String,
    /// The rustc that compiled this binary (see [`rustc_version`]).
    pub rustc_version: String,
    /// The one-line profile verdict (see [`profile_line`]).
    pub profile: String,
}

impl BuildInfo {
    /// Collect the compile-time build facts. Every field is already
    /// baked into the binary — this is assembly, not probing.
    pub(crate) fn collect() -> Self {
        Self {
            version: VERSION.to_string(),
            git_sha: build_hash().to_string(),
            variant: build_label(),
            build_time: build_time().to_string(),
            rustc_version: rustc_version().to_string(),
            profile: profile_line(),
        }
    }
}

/// Dynamic build target: detects arch + libc env at compile time.
/// Returns the `<arch>-<libc>` token — e.g. "amd64-gnu" (glibc,
/// dynamic) or "amd64-musl" (static) for x86_64 Linux builds — WITHOUT
/// the OS prefix: [`build_string`] composes the full
/// `linux-amd64-gnu`-shape label by joining `std::env::consts::OS` in
/// front (the zelynic chain shape; keeping an OS prefix here produced
/// the double-`linux` label the smoke run caught).
pub fn build_target() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "musl"))]
    {
        "amd64-musl"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    {
        "amd64-gnu"
    }
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        not(any(target_env = "musl", target_env = "gnu"))
    ))]
    {
        "amd64"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "musl"))]
    {
        "aarch64-musl"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "gnu"))]
    {
        "aarch64-gnu"
    }
    #[cfg(all(
        target_os = "linux",
        target_arch = "aarch64",
        not(any(target_env = "musl", target_env = "gnu"))
    ))]
    {
        "aarch64"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    {
        "unknown"
    }
}

/// The detected fallback label — the os-arch-libc shape the docs promise
/// (`linux-amd64-gnu` style).
fn build_string() -> String {
    format!("{}-{}", std::env::consts::OS, build_target())
}

/// Canonical build label for the `Build:` line of the version report and
/// the doctor's Variant fact.
///
/// Source of truth: the `ZYLAXION_BUILD` env var forwarded at compile
/// time by build.rs — exported by CI/release scripts (the platform id
/// verbatim, e.g. `linux-amd64-v3-gnu`). When unset (plain `cargo
/// build`), it falls back to the compile-time arch+libc detection.
pub(crate) fn build_label() -> String {
    match option_env!("ZYLAXION_BUILD") {
        Some(label) if !label.is_empty() => label.to_string(),
        _ => build_string(),
    }
}

/// The git commit hash injected at build time by build.rs.
pub(crate) fn build_hash() -> &'static str {
    option_env!("GIT_HASH").unwrap_or("unknown")
}

/// The build timestamp injected at build time by build.rs.
///
/// Computed by Howard Hinnant's civil_from_days algorithm inside
/// build.rs (night-improve-13, the zelynic NIGHT-hunt-6 lineage) —
/// `M/D/YYYY HH:MM (UTC)` — so no time crate (chrono/jiff and friends)
/// is needed anywhere in the dependency tree. Falls back to "unknown"
/// only when the build script could not read the system clock
/// (pre-UNIX_EPOCH host clock).
pub(crate) fn build_time() -> &'static str {
    option_env!("ZYLAXION_BUILD_TIME").unwrap_or("unknown")
}

#[cfg(test)]
#[path = "../../test/info/build_tests.rs"]
mod tests;
