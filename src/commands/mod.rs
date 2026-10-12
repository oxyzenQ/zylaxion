// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Command modules — each subcommand's handler lives in its own file.
//!
//! - `sound`   — `on`, `off`, `preset`, `volume`, `stats`, `status`
//!   (night-improve-23: the daemonless control plane — every verb is a
//!   short-lived map write or read against the pinned control plane)
//! - `attach`  — `attach`, `detach` (night-improve-23: the privileged
//!   one-shot that plants the kernel-side control plane, and its undo)
//! - `player`  — the hidden `_play` internal verb (the stateless player)
//! - `help`    — the single curated help reference (night-improve-15)
//! - `info`    — `doctor`, `list-presets`, `list-backends` (diagnostics)
//! - `update`  — `--check-update` (GitHub release check, v10.2.0+ — I9)

pub mod attach;
pub mod sound;
pub mod stats;

pub(crate) mod help;
pub mod info;
pub(crate) mod player;
pub mod update;
