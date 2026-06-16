// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Acoustic profile definitions and DSP models for the Zactrix engine.
//!
//! This crate provides the foundational types and traits that define how
//! keyboard key events are transformed into audio. It contains zero I/O
//! dependencies and is purely mathematical in nature.
//!
//! ## Architecture
//!
//! - [`AcousticModel`] trait — the interface every sound profile must implement.
//! - [`KeyProfile`] and sub-parameter structs — TOML-friendly DSP configuration.
//! - [`SynthState`] — zero-alloc per-voice state passed through the render path.
//! - [`MechanicalClick`] — the reference implementation for mechanical switches.
//! - [`TptSvf`] — topology-preserving SVF for numerically stable filtering.

use serde::{Deserialize, Serialize};

mod mechanical;
mod tpt;

pub use mechanical::MechanicalClick;
pub use tpt::TptSvf;

/// Default sample rate for the Zactrix engine (Hz).
pub const SAMPLE_RATE: f32 = 44_100.0;

/// Maximum simultaneous voices the engine can produce.
pub const MAX_POLYPHONY: usize = 16;

/// Represents a keyboard key event from the input layer.
///
/// This struct is the bridge between the input handler (e.g., `libinput`)
/// and the DSP engine. It carries the hardware scancode, the pressed/released
/// state, and the pre-computed stereo position based on the key's physical
/// location on the keyboard.
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    /// Hardware evdev scancode of the key.
    pub scancode: u32,
    /// `true` for key press, `false` for key release.
    pub pressed: bool,
    /// Stereo panning position: -1.0 (full left) to 1.0 (full right).
    pub stereo_position: f32,
}

/// Parameters controlling the initial click transient of a key press.
///
/// The click is modeled as a short burst of shaped white noise passed through
/// a TPT bandpass filter. These parameters define the filter characteristics
/// and the temporal envelope of the excitation burst.
///
/// # Typical Ranges
///
/// | Parameter | Cherry MX Blue | Cherry MX Red | Topre |
/// |-----------|---------------|---------------|-------|
/// | `frequency` | 4000–5500 Hz | 3500–4500 Hz | 2500–3500 Hz |
/// | `resonance` | 1.5–3.0 | 1.0–2.0 | 2.0–4.0 |
/// | `duration_ms` | 1.0–2.0 ms | 0.8–1.5 ms | 1.5–3.0 ms |
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ClickParams {
    /// Center frequency of the click bandpass filter (Hz).
    pub frequency: f32,
    /// Quality factor of the click filter. Higher values produce a narrower,
    /// more resonant click with longer ring-out.
    pub resonance: f32,
    /// Duration of the noise excitation burst in milliseconds.
    pub duration_ms: f32,
    /// Peak amplitude of the click transient (0.0–1.0).
    pub amplitude: f32,
}

/// Parameters controlling the spring/housing resonance after the initial click.
///
/// After the excitation burst ends, the TPT filter continues to ring at its
/// natural frequency, simulating the spring snapping back and the keycap
/// housing vibrating. The `mix` parameter controls how prominent this
/// resonance is relative to the click transient.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SpringParams {
    /// Resonance frequency of the spring element (Hz).
    pub frequency: f32,
    /// Quality factor. Higher values produce longer, more pronounced ringing.
    pub resonance: f32,
    /// Mix level of the spring component in the final output (0.0–1.0).
    pub mix: f32,
}

/// Parameters controlling the exponential decay envelope applied to the output.
///
/// The decay envelope models the natural energy dissipation of the mechanical
/// system. The coefficient is applied per-sample as a multiplicative factor,
/// so the effective decay time depends on both the coefficient and the sample rate.
///
/// # Decay Time Calculation
///
/// The time to reach -60 dB (inaudible) is approximately:
///
/// ```text
/// t_60 = -60 / (20 * log10(coefficient) * sample_rate)
/// ```
///
/// For `coefficient = 0.9994` at 44100 Hz, this gives roughly 180 ms.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DecayParams {
    /// Per-sample multiplicative decay factor. Must be in (0.0, 1.0).
    pub coefficient: f32,
    /// Amplitude threshold below which the voice is deactivated.
    pub voice_off_threshold: f32,
}

/// Complete acoustic profile for a single key event.
///
/// This struct aggregates all DSP parameters needed to synthesize one
/// keystroke sound. Profiles can be created manually, loaded from TOML
/// configuration files, or generated procedurally by an [`AcousticModel`]
/// implementation based on the scancode.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct KeyProfile {
    /// Click transient parameters.
    pub click: ClickParams,
    /// Spring resonance parameters.
    pub spring: SpringParams,
    /// Decay envelope parameters.
    pub decay: DecayParams,
}

/// Mutable DSP state for a single voice instance.
///
/// This struct is zero-alloc and designed to live on the stack or within a
/// pre-allocated array. It is passed to [`AcousticModel::render_sample`] on
/// every sample tick and persists across calls.
///
/// All filter coefficients and pre-computed values are stored here so that
/// the render path never needs to call `tan()` or perform division.
#[derive(Debug, Clone)]
pub struct SynthState {
    // --- TPT SVF filter state (click path) ---
    /// First integrator state for the click filter.
    pub click_ic1eq: f32,
    /// Second integrator state for the click filter.
    pub click_ic2eq: f32,
    /// Pre-computed `tan(pi * fc / fs)` for the click filter.
    pub click_g: f32,
    /// Pre-computed `1 / Q` for the click filter.
    pub click_k: f32,

    // --- TPT SVF filter state (spring path) ---
    /// First integrator state for the spring filter.
    pub spring_ic1eq: f32,
    /// Second integrator state for the spring filter.
    pub spring_ic2eq: f32,
    /// Pre-computed `tan(pi * fc / fs)` for the spring filter.
    pub spring_g: f32,
    /// Pre-computed `1 / Q` for the spring filter.
    pub spring_k: f32,

    // --- Excitation noise generator (xorshift32) ---
    /// Internal state of the xorshift32 PRNG used for noise excitation.
    pub noise_state: u32,

    // --- Envelope ---
    /// Current envelope amplitude value.
    pub envelope_value: f32,
    /// Number of samples rendered since the voice was triggered.
    pub sample_count: u32,
    /// Duration of the noise excitation burst in samples.
    pub excitation_samples: u32,
    /// Pre-computed per-sample decay coefficient (from profile).
    pub decay_coeff: f32,
    /// Pre-computed voice-off threshold (from profile).
    pub voice_off_threshold: f32,

    // --- Stereo pan gains (pre-computed) ---
    /// Left channel gain from equal-power pan law.
    pub pan_left: f32,
    /// Right channel gain from equal-power pan law.
    pub pan_right: f32,
    /// Pre-computed spring mix level (from profile).
    pub spring_mix: f32,

    // --- Voice lifecycle ---
    /// Whether this voice is currently producing audio.
    pub active: bool,
}

impl Default for SynthState {
    fn default() -> Self {
        Self {
            click_ic1eq: 0.0,
            click_ic2eq: 0.0,
            click_g: 0.0,
            click_k: 0.0,
            spring_ic1eq: 0.0,
            spring_ic2eq: 0.0,
            spring_g: 0.0,
            spring_k: 0.0,
            noise_state: 1,
            envelope_value: 0.0,
            sample_count: 0,
            excitation_samples: 0,
            decay_coeff: 0.9994,
            voice_off_threshold: 1e-5,
            pan_left: std::f32::consts::FRAC_1_SQRT_2,
            pan_right: std::f32::consts::FRAC_1_SQRT_2,
            spring_mix: 0.6,
            active: false,
        }
    }
}

/// Trait for defining how a keyboard key sounds.
///
/// Implementors provide the DSP logic that transforms key events into audio
/// samples. The trait is designed for zero-allocation render paths: all
/// mutable state lives in [`SynthState`], which is allocated once per voice
/// and reused across sample ticks.
///
/// # Lifecycle
///
/// 1. [`get_profile`](Self::get_profile) is called once when a voice is triggered.
/// 2. [`init_state`](Self::init_state) pre-computes coefficients from the profile.
/// 3. [`render_sample`](Self::render_sample) is called per-sample until the
///    voice deactivates itself (sets `state.active = false`).
pub trait AcousticModel: Send + Sync {
    /// Return the acoustic profile for a given key event.
    ///
    /// Called once when a voice is triggered. The returned [`KeyProfile`] is
    /// cached by the voice pool and used to initialize the voice state.
    fn get_profile(&self, event: &KeyEvent) -> KeyProfile;

    /// Initialize the synthesis state from a profile.
    ///
    /// Called once before the first [`render_sample`](Self::render_sample) call.
    /// Implementors should pre-compute all filter coefficients, pan gains, and
    /// timing values here so that `render_sample` is free of division and
    /// transcendentals.
    fn init_state(&self, profile: &KeyProfile, state: &mut SynthState, stereo_position: f32);

    /// Render a single stereo sample.
    ///
    /// Called once per sample tick for each active voice. **Must not allocate.**
    ///
    /// # Returns
    /// A stereo sample `[left, right]` in `[-1.0, 1.0]`.
    fn render_sample(&self, state: &mut SynthState) -> [f32; 2];
}

impl Default for ClickParams {
    fn default() -> Self {
        Self {
            frequency: 4500.0,
            resonance: 2.0,
            duration_ms: 1.5,
            amplitude: 0.8,
        }
    }
}

impl Default for SpringParams {
    fn default() -> Self {
        Self {
            frequency: 1800.0,
            resonance: 3.5,
            mix: 0.6,
        }
    }
}

impl Default for DecayParams {
    fn default() -> Self {
        Self {
            coefficient: 0.9994,
            voice_off_threshold: 1e-5,
        }
    }
}

// ── Profile loading ──────────────────────────────────────────────────

/// Load a [`KeyProfile`] from a TOML file.
///
/// The file must contain a `[profile]` table with `click`, `spring`, and
/// `decay` sub-tables matching the DSP parameter structure.
///
/// # Errors
///
/// Returns a human-readable error string if the file cannot be read or
/// parsed.  This function is intentionally fallible — callers should
/// fall back to a hardcoded default on failure.
pub fn load_profile_from_file(path: &std::path::Path) -> Result<KeyProfile, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    load_profile_from_str(&content)
}

/// Parse a [`KeyProfile`] from a TOML string.
///
/// Expects the standard `[profile]` top-level table.
pub fn load_profile_from_str(toml: &str) -> Result<KeyProfile, String> {
    #[derive(Deserialize)]
    struct ProfileFile {
        profile: KeyProfile,
    }
    let file: ProfileFile =
        toml::from_str(toml).map_err(|e| format!("failed to parse profile TOML: {e}"))?;
    Ok(file.profile)
}
