// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The binary's self-knowledge (night-improve-14, the zelynic `info/`
//! concern): the build facts the version report and the doctor lead
//! with, and the Hinnant clock that stamps runtime log lines.
//!
//! - [`build`] — compile-time truth (version, commit, variant, build
//!   time, rustc, profile), assembled by the `build.rs` epoch suite.
//! - [`clock`] — Howard Hinnant's `civil_from_days` runtime clock
//!   (ISO millis stamps), zero time crates in the dependency tree.

pub mod build;
pub mod clock;
