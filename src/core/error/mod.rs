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
    /// Audio output device could not be opened.
    Audio(crate::audio::AudioError),
    /// Input source could not be started.
    // night-improve-21: production surfaces input errors from listen()
    // directly; the variant stays as the designed error contract and
    // the core test tree constructs it, hence the allow.
    #[allow(dead_code)]
    Input(crate::input::InputError),
}

impl fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audio(e) => write!(f, "audio: {e}"),
            Self::Input(e) => write!(f, "input: {e}"),
        }
    }
}

impl std::error::Error for OrchestratorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Audio(e) => Some(e),
            Self::Input(e) => Some(e),
        }
    }
}
