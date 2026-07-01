// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Single voice representation in the Zactrix engine.
//!
//! A [`Voice`] is a lightweight container that pairs a cached [`KeyProfile`]
//! with a mutable [`SynthState`]. It does not perform DSP itself — that
//! responsibility belongs to the [`AcousticModel`](zactrix_profiles::AcousticModel)
//! called by the [`VoicePool`](super::VoicePool).

use zactrix_profiles::{KeyProfile, SynthState};

/// A single active or inactive voice in the polyphonic engine.
///
/// Each voice corresponds to one currently sounding (or recently released)
/// key. The voice holds the cached acoustic profile and all mutable DSP
/// state needed for sample rendering.
#[derive(Debug, Clone)]
pub struct Voice {
    /// Mutable synthesis state (filter states, envelope, noise generator).
    pub state: SynthState,
    /// Cached acoustic profile for this voice's key.
    pub profile: KeyProfile,
    /// Hardware scancode that triggered this voice.
    pub scancode: u32,
    /// Monotonic timestamp when this voice was triggered (for voice stealing).
    pub trigger_timestamp: u64,
}

impl Voice {
    /// Create a new inactive voice with default (silent) state.
    #[inline]
    pub fn new() -> Self {
        Self {
            state: SynthState::default(),
            profile: KeyProfile::default(),
            scancode: 0,
            trigger_timestamp: 0,
        }
    }

    /// Check if this voice is currently producing audio.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state.active
    }

    /// Reset the voice to an inactive, silent state.
    #[inline]
    pub fn reset(&mut self) {
        self.state = SynthState::default();
        self.scancode = 0;
        self.trigger_timestamp = 0;
    }
}

impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}
