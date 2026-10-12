// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The acoustic parameter structs and their compiled-in defaults.
//!
//! Split from the crate root in night-improve-18 (the no-mercy LOC
//! law): the param tables are the config surface, the crate root
//! stays the wiring + re-export home. Every path is re-exported at
//! the crate root, so external users see no change.

pub const SAMPLE_RATE: f32 = 44_100.0;

/// Maximum simultaneous voices the engine can produce.
pub const MAX_POLYPHONY: usize = 16;

/// A trigger event sent to the acoustic model to start a voice.
///
/// `KeyTrigger` is the DSP-side concept: it carries the scancode,
/// press/release state, pre-computed stereo position, and optional
/// velocity for analog switches. The orchestrator translates
/// `crate::core::KeyEvent` → `KeyTrigger` by computing the stereo
/// position from the scancode.
///
/// # Velocity (v11.0.0+ — analog switch support)
///
/// `velocity: Option<f32>` enables Hall-effect / analog switch
/// support (Wooting Lekker, Topre R3, Razer Analog Optical, etc.).
/// - `None` = digital switch (default). The DSP treats this as full
///   velocity (1.0) — standard mechanical keyboards.
/// - `Some(v)` where `v` is in `[0.0, 1.0]` = analog switch. The
///   DSP scales amplitude, resonance, and excitation burst duration
///   proportionally. A light tap (`Some(0.3)`) produces a quieter,
///   softer click; a hard press (`Some(1.0)`) matches the digital
///   behavior.
///
/// The standard libinput input path always sends `None`. A future
/// Hall-effect input source would fill in the velocity from the
/// device's analog data.
#[derive(Debug, Clone, Copy)]
pub struct KeyTrigger {
    /// Hardware evdev scancode of the key.
    pub scancode: u32,
    /// `true` for key press, `false` for key release.
    // night-improve-21: carried for evdev fidelity — the orchestrator
    // branches on the input KeyEvent's flag, the profile lane keeps
    // the mirror field so the trigger contract stays event-faithful.
    #[allow(dead_code)]
    pub pressed: bool,
    /// Stereo panning position: -1.0 (full left) to 1.0 (full right).
    pub stereo_position: f32,
    /// Analog key velocity: `None` for digital switches (treated as
    /// 1.0), `Some(0.0..=1.0)` for Hall-effect / analog switches.
    /// (v11.0.0+)
    pub velocity: Option<f32>,
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
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone, Copy)]
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
///
/// # Two-stage decay (v10.2.0+ — dragonzen audit N7)
///
/// Real key sounds have a fast initial transient (~5 ms) + slower tail
/// (~150 ms). A single multiplicative coefficient forces a compromise:
/// `0.9994` (180 ms) feels "boomy"; `0.9990` (110 ms) feels "thin".
/// The optional `coefficient_fast` + `fast_samples_ms` fields enable a
/// two-stage envelope that captures both regimes:
///
/// 1. **Fast stage**: for the first `fast_samples_ms` of audio (default
///    ~5 ms), apply `coefficient_fast` (e.g. 0.997 — much steeper
///    decay). This captures the rapid energy drop of the click
///    transient.
/// 2. **Slow stage**: after the fast stage ends, switch to `coefficient`
///    (the existing field, e.g. 0.9994). This carries the long tail
///    (spring ring, housing thock) at the existing decay rate.
///
/// Both fields default to `0.0` / `0.0` which disables the fast stage
/// — backward-compatible with existing configs that don't set them.
#[derive(Debug, Clone, Copy)]
pub struct DecayParams {
    /// Per-sample multiplicative decay factor for the slow (tail) stage.
    /// Must be in (0.0, 1.0). Used after `fast_samples_ms` elapses, or
    /// for the entire voice if `fast_samples_ms = 0.0`.
    pub coefficient: f32,
    /// Amplitude threshold below which the voice is deactivated.
    pub voice_off_threshold: f32,
    /// Per-sample multiplicative decay factor for the fast (initial)
    /// stage (v10.2.0+ — N7). Must be in (0.0, 1.0). Used for the
    /// first `fast_samples_ms` of audio, then the voice switches to
    /// `coefficient`. Default `0.0` disables the fast stage (single-
    /// stage decay, backward-compatible).
    ///
    /// Typical value: `0.997` — at 44.1 kHz this reaches -60 dB in
    /// ~23 ms, capturing the click transient's rapid energy drop.
    pub coefficient_fast: f32,
    /// Duration of the fast (initial) stage in milliseconds (v10.2.0+ —
    /// N7). After this many milliseconds, the voice switches from
    /// `coefficient_fast` to `coefficient`. Default `0.0` disables the
    /// fast stage.
    ///
    /// Typical value: `5.0` — the click transient's initial energy
    /// drop completes within ~5 ms, then the spring/housing tail takes
    /// over.
    pub fast_samples_ms: f32,
}

/// Parameters controlling the ambient case-rattle / hollow-housing noise.
///
/// Real mechanical keyboards produce a secondary "rattle" sound when a
/// key is pressed: the keycap stem hits the switch housing, the PCB
/// flexes slightly, and the hollow case amplifies the impact. This is
/// distinct from the click transient (which is the switch mechanism
/// itself) and the spring resonance (which is the spring vibrating).
///
/// The ambient noise is modeled as a short burst of high-pass filtered
/// white noise with its own decay envelope, mixed into the final output.
/// When `enabled` is `false`, no ambient noise is generated (zero CPU
/// cost).
#[derive(Debug, Clone, Copy)]
pub struct AmbientParams {
    /// Master enable for the ambient rattle path. When `false`, the
    /// engine skips ambient noise generation entirely.
    pub enabled: bool,
    /// Peak amplitude of the ambient noise burst (0.0 = silent,
    /// 1.0 = full). Typical values: 0.05 (subtle) to 0.3 (heavy rattle).
    pub noise_level: f32,
    /// Per-sample multiplicative decay factor for the ambient envelope.
    /// Controls how long the rattle persists after the keypress.
    /// MUST be < 1.0 (same constraint as `DecayParams::coefficient`).
    /// Lower values = faster rattle decay; higher values = longer ring.
    pub noise_decay: f32,
}

/// Parameters controlling the housing "thock" — the deep, low-frequency
/// impact of the keycap bottoming out against the switch housing / PCB.
///
/// Real mechanical keyboards have a distinct low-mid "buk" / "thock"
/// sound (typically 150–800 Hz) that sits underneath the click transient.
/// Without this layer, synthesized keyboard sounds feel "thin" — the
/// high-frequency click is present but the body / weight is missing.
/// The housing layer fixes this by adding a third TPT SVF bandpass
/// driven by the same noise excitation, tuned to low frequencies with
/// fast natural decay (the keycap impact is a brief event, not a
/// sustained ring).
///
/// # Layer separation
///
/// The three DSP layers serve distinct acoustic roles:
/// - [`ClickParams`] — the sharp, high-frequency transient (2–5 kHz).
///   Models the switch mechanism actuating.
/// - [`SpringParams`] — the resonant ring of the spring / housing walls
///   (1–3 kHz). Models the spring vibrating after the click.
/// - `HousingParams` — the deep, low-mid impact "thock" (150–800 Hz).
///   Models the keycap hitting the switch housing / PCB at bottom-out.
///
/// All three are driven by the same noise excitation burst and share
/// the main decay envelope — they differ only in their filter
/// frequency, resonance Q, and output mix level.
#[derive(Debug, Clone, Copy)]
pub struct HousingParams {
    /// Resonant frequency of the housing / PCB impact (Hz).
    ///
    /// Real keyboards land in the 150–800 Hz range:
    /// - Thick aluminum / brass cases: 200–400 Hz (deeper thock).
    /// - Plastic / FR4 PCB: 400–600 Hz (mid thock).
    /// - Low-profile switches: 600–800 Hz (tighter, less body).
    pub frequency: f32,
    /// Quality factor of the housing bandpass filter. Higher values
    /// produce a more pronounced, sustained thock; lower values a
    /// broader, muffled impact.
    pub resonance: f32,
    /// Mix level of the housing layer in the final output (0.0–1.0).
    ///
    /// - Linear / thocky switches (Topre, Gateron Yellow): 0.7–0.9.
    /// - Tactile switches (MX Clear, Zealios): 0.4–0.6.
    /// - Clicky switches (MX Blue, buckling-spring): 0.2–0.3 (the click
    ///   is the dominant character, thock should not mask it).
    pub mix: f32,
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
            // Default to disabled — backward-compatible with pre-v10.2.0
            // configs. Users opt in by setting both fields.
            coefficient_fast: 0.0,
            fast_samples_ms: 0.0,
        }
    }
}

impl Default for AmbientParams {
    fn default() -> Self {
        Self {
            enabled: false,
            noise_level: 0.1,
            noise_decay: 0.99,
        }
    }
}

impl Default for HousingParams {
    fn default() -> Self {
        Self {
            frequency: 400.0,
            resonance: 2.0,
            mix: 0.4,
        }
    }
}

// ── Validation & clamping ───────────────────────────────────────────
