// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Command modules — each subcommand's handler lives in its own file.
//!
//! - `daemon`  — `start`, `daemon`, `stop` (run / background / quit;
//!   night-improve-23 retires this module)
//! - `attach`  — `attach`, `detach` (night-improve-23: the privileged
//!   one-shot that plants the kernel-side control plane, and its undo)
//! - `help`    — the single curated help reference (night-improve-15)
//! - `info`    — `doctor`, `list-presets`, `list-backends` (diagnostics)
//! - `update`  — `--check-update` (GitHub release check, v10.2.0+ — I9)

pub mod attach;
pub mod daemon;
pub(crate) mod help;
pub mod info;
pub(crate) mod player;
pub mod update;
