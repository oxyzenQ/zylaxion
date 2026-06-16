// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command modules — each subcommand's handler lives in its own file.
//!
//! - `daemon`  — `start`, `daemon`, `stop` (run / background / quit)
//! - `info`    — `doctor`, `list-profiles`, `list-backends` (diagnostics)

pub mod daemon;
pub mod info;
