// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! night-netrunner-Z2: the proof-harness package's root (the sampled
//! tier, the zelynic night-frontier-Z6 lineage adapted to this tree).
//! The pure gain-law core and the clock core compile here from their
//! own files (#[path] wirings — the fuzz harnesses' include law), and
//! the sampled instrument attaches as a cfg-gated module:
//!
//!  * `property_tests` — #[cfg(test)]: proptest exhaustion of the
//!    cores' laws on stable (the standing lane: `cargo test`).
//!
//! The cores are law libraries: the instrument uses the slice of each
//! core its properties name, so dead-code silence is the honest crate
//! posture here (every unused fn is pinned by another tree).
#![allow(dead_code)]

// ── The included law files (the SAME bytes the binary ships — the
// include law the fuzz lane set, now ridden by the proof lane; the
// extraction that made it possible is night-netrunner-Z2's pure move
// of the gain laws into src/engine/gain.rs). One inclusion per file
// per binary; the harness modules consume these, never their own
// copies.

#[path = "../../src/engine/gain.rs"]
mod gain_core;

// The clock core's #[cfg(test)] test-module wiring (the Pattern C
// #[path] to test/info/clock_tests.rs) never compiles here — the
// fuzz lane's own documented exception to the include law (a
// #[cfg(test)] module is invisible to a non-test build, and this
// package's test build drives property_tests instead).
#[path = "../../src/info/clock.rs"]
mod clock_core;

// ── The instrument (cfg-gated; see the module doc).

#[cfg(test)]
#[path = "property_tests.rs"]
mod property_tests;
