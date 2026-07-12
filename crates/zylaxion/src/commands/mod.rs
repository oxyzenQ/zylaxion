// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Command modules — each subcommand's handler lives in its own file.
//!
//! - `daemon`  — `start`, `daemon`, `stop` (run / background / quit)
//! - `info`    — `doctor`, `testconf`, `list-presets`, `list-backends` (diagnostics)
//! - `update`  — `--check-update` (GitHub release check, v10.2.0+ — I9)

pub mod daemon;
pub mod info;
pub mod update;
