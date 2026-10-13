// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The session contract (night-improve-14, the zelynic concern-folder
//! discipline): everything that binds zylaxion to the invoking user's
//! session. zylaxion is a user-session program — every byte of state
//! it writes belongs inside that session, and each module here guards
//! one edge of that promise:
//!
//! - [`preset`] — WHICH sound governs (night-improve-22, the
//!   no-config law: the CLI flag / `ZYLAXION_PRESET` env / compiled
//!   registry resolution — no config file, no watcher).
//! - [`kernel`] — WHETHER the host kernel can carry the eBPF-only
//!   architecture at all (night-improve-23: the 5.15 floor, the
//!   "verified supported" / "verified unsupported" verdicts).
//! - [`privilege`] — WHO may run the privileged surfaces (the
//!   night-improve-16 root-usage audit, extended with the night-improve-23
//!   attach REQUIRE row).
//! - [`signals`] — how the session ends (graceful shutdown handlers).
//!
//! night-improve-23: the flock (`instance_lock`) died with the daemon —
//! the single-instance marker is the pinned control map's player pid
//! (see `crate::commands::player`), read and written in one place, with
//! no file to go stale.

pub mod kernel;
pub mod orphan;
pub mod preset;
pub mod privilege;
pub mod signals;
