// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The orchestrator error contract.
//!
//! night-improve-21: split out of the core root (the no-mercy LOC cap
//! demands the SPLIT; this is a pure move, gates green in between) —
//! one theme per file, the zelynic layout law.

use std::fmt;

// ── OrchestratorError ───────────────────────────────────────────────────

/// Errors from the orchestrator.
#[derive(Debug)]
pub enum OrchestratorError {
    /// Audio output device could not be opened. The input half of this
    /// error contract (the former Input variant) died with the libinput
    /// capture in night-improve-23 — the kernel-side tap cannot fail at
    /// runtime from the orchestrator's perspective; a dead tap simply
    /// rings nothing, and the tap's own failures surface at attach.
    Audio(crate::audio::AudioError),
}

impl fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audio(e) => write!(f, "audio: {e}"),
        }
    }
}

impl std::error::Error for OrchestratorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Audio(e) => Some(e),
        }
    }
}
