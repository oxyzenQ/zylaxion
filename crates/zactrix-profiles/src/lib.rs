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
    #[serde(default = "default_click_frequency")]
    pub frequency: f32,
    /// Quality factor of the click filter. Higher values produce a narrower,
    /// more resonant click with longer ring-out.
    #[serde(default = "default_click_resonance")]
    pub resonance: f32,
    /// Duration of the noise excitation burst in milliseconds.
    #[serde(default = "default_click_duration_ms")]
    pub duration_ms: f32,
    /// Peak amplitude of the click transient (0.0–1.0).
    #[serde(default = "default_click_amplitude")]
    pub amplitude: f32,
}

fn default_click_frequency() -> f32 {
    4500.0
}
fn default_click_resonance() -> f32 {
    2.0
}
fn default_click_duration_ms() -> f32 {
    1.5
}
fn default_click_amplitude() -> f32 {
    0.8
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
    #[serde(default = "default_spring_frequency")]
    pub frequency: f32,
    /// Quality factor. Higher values produce longer, more pronounced ringing.
    #[serde(default = "default_spring_resonance")]
    pub resonance: f32,
    /// Mix level of the spring component in the final output (0.0–1.0).
    #[serde(default = "default_spring_mix")]
    pub mix: f32,
}

fn default_spring_frequency() -> f32 {
    1800.0
}
fn default_spring_resonance() -> f32 {
    3.5
}
fn default_spring_mix() -> f32 {
    0.6
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
    #[serde(default = "default_decay_coefficient")]
    pub coefficient: f32,
    /// Amplitude threshold below which the voice is deactivated.
    #[serde(default = "default_decay_voice_off_threshold")]
    pub voice_off_threshold: f32,
}

fn default_decay_coefficient() -> f32 {
    0.9994
}
fn default_decay_voice_off_threshold() -> f32 {
    1e-5
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AmbientParams {
    /// Master enable for the ambient rattle path. When `false`, the
    /// engine skips ambient noise generation entirely.
    #[serde(default)]
    pub enabled: bool,
    /// Peak amplitude of the ambient noise burst (0.0 = silent,
    /// 1.0 = full). Typical values: 0.05 (subtle) to 0.3 (heavy rattle).
    #[serde(default = "default_ambient_noise_level")]
    pub noise_level: f32,
    /// Per-sample multiplicative decay factor for the ambient envelope.
    /// Controls how long the rattle persists after the keypress.
    /// MUST be < 1.0 (same constraint as `DecayParams::coefficient`).
    /// Lower values = faster rattle decay; higher values = longer ring.
    #[serde(default = "default_ambient_noise_decay")]
    pub noise_decay: f32,
}

fn default_ambient_noise_level() -> f32 {
    0.1
}
fn default_ambient_noise_decay() -> f32 {
    0.99
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HousingParams {
    /// Resonant frequency of the housing / PCB impact (Hz).
    ///
    /// Real keyboards land in the 150–800 Hz range:
    /// - Thick aluminum / brass cases: 200–400 Hz (deeper thock).
    /// - Plastic / FR4 PCB: 400–600 Hz (mid thock).
    /// - Low-profile switches: 600–800 Hz (tighter, less body).
    #[serde(default = "default_housing_frequency")]
    pub frequency: f32,
    /// Quality factor of the housing bandpass filter. Higher values
    /// produce a more pronounced, sustained thock; lower values a
    /// broader, muffled impact.
    #[serde(default = "default_housing_resonance")]
    pub resonance: f32,
    /// Mix level of the housing layer in the final output (0.0–1.0).
    ///
    /// - Linear / thocky switches (Topre, Gateron Yellow): 0.7–0.9.
    /// - Tactile switches (MX Clear, Zealios): 0.4–0.6.
    /// - Clicky switches (MX Blue, buckling-spring): 0.2–0.3 (the click
    ///   is the dominant character, thock should not mask it).
    #[serde(default = "default_housing_mix")]
    pub mix: f32,
}

fn default_housing_frequency() -> f32 {
    400.0
}
fn default_housing_resonance() -> f32 {
    2.0
}
fn default_housing_mix() -> f32 {
    0.4
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
    #[serde(default)]
    pub click: ClickParams,
    /// Spring resonance parameters.
    #[serde(default)]
    pub spring: SpringParams,
    /// Decay envelope parameters.
    #[serde(default)]
    pub decay: DecayParams,
    /// Ambient case-rattle parameters.
    #[serde(default)]
    pub ambient: AmbientParams,
    /// Housing "thock" parameters (v4.2.0+).
    #[serde(default)]
    pub housing: HousingParams,
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

    // --- TPT SVF filter state (housing path, v4.2.0+) ---
    /// First integrator state for the housing filter.
    pub housing_ic1eq: f32,
    /// Second integrator state for the housing filter.
    pub housing_ic2eq: f32,
    /// Pre-computed `tan(pi * fc / fs)` for the housing filter.
    pub housing_g: f32,
    /// Pre-computed `1 / Q` for the housing filter.
    pub housing_k: f32,
    /// Pre-computed housing mix level (from profile).
    pub housing_mix: f32,
    /// Pre-computed housing excitation length in samples.
    ///
    /// The housing filter is tuned to low frequencies (100-1000 Hz)
    /// where the filter's natural response time (`1/fc`) is much
    /// longer than the click excitation burst (1-3 ms). Driving a
    /// 250 Hz bandpass with a 2 ms noise burst produces almost no
    /// output — the filter has no time to ring up.
    ///
    /// To fix this, the housing layer gets its own longer excitation
    /// window, sized as `max(click_excitation, sample_rate / fc * 4)`
    /// — i.e. four periods of the housing fundamental. For a 250 Hz
    /// housing at 44100 Hz, this is `4 * 44100/250 = 706 samples ≈ 16 ms`,
    /// long enough for the filter to ring up to its steady-state Q
    /// gain before the excitation fades.
    pub housing_excitation_samples: u32,

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

    // --- Ambient rattle path (optional, enabled per-profile) ---
    /// Whether the ambient rattle path is active for this voice.
    /// Copied from `KeyProfile::ambient::enabled` at init time. When
    /// `false`, the render path skips all ambient computation (zero
    /// cost).
    pub ambient_enabled: bool,
    /// Pre-computed peak amplitude of the ambient noise burst (from
    /// `KeyProfile::ambient::noise_level`).
    pub ambient_level: f32,
    /// Pre-computed per-sample decay coefficient for the ambient
    /// envelope (from `KeyProfile::ambient::noise_decay`).
    pub ambient_decay: f32,
    /// Current ambient envelope amplitude value. Decays per-sample via
    /// `ambient_decay`. When it falls below `ambient_level * 1e-4`,
    /// ambient generation stops for this voice.
    pub ambient_envelope: f32,
    /// Separate xorshift32 state for the ambient noise generator so
    /// it doesn't correlate with the click excitation noise.
    pub ambient_noise_state: u32,
    /// High-pass filter state for the ambient noise (one-pole).
    /// Stores the previous input sample for the difference equation
    /// `y[n] = x[n] - x[n-1] + hp_coeff * y[n-1]`.
    pub ambient_hp_prev_input: f32,
    /// High-pass filter state for the ambient noise (one-pole).
    /// Stores the previous output sample.
    pub ambient_hp_prev_output: f32,
    /// Pre-computed high-pass filter coefficient. Derived from a fixed
    /// cutoff (e.g. 2 kHz) at the sample rate.
    pub ambient_hp_coeff: f32,

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
            housing_ic1eq: 0.0,
            housing_ic2eq: 0.0,
            housing_g: 0.0,
            housing_k: 0.0,
            housing_mix: 0.4,
            housing_excitation_samples: 0,
            noise_state: 1,
            envelope_value: 0.0,
            sample_count: 0,
            excitation_samples: 0,
            decay_coeff: 0.9994,
            voice_off_threshold: 1e-5,
            pan_left: std::f32::consts::FRAC_1_SQRT_2,
            pan_right: std::f32::consts::FRAC_1_SQRT_2,
            spring_mix: 0.6,
            ambient_enabled: false,
            ambient_level: 0.0,
            ambient_decay: 0.99,
            ambient_envelope: 0.0,
            ambient_noise_state: 1,
            ambient_hp_prev_input: 0.0,
            ambient_hp_prev_output: 0.0,
            ambient_hp_coeff: 0.0,
            active: false,
        }
    }
}

/// Trait for defining how a keyboard key sounds.
///
/// Implementers provide the DSP logic that transforms key events into audio
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
    /// Implementers should pre-compute all filter coefficients, pan gains, and
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
/// Expects the standard `[profile]` top-level table. The parsed profile
/// is validated and clamped to safe DSP ranges before being returned.
pub fn load_profile_from_str(toml: &str) -> Result<KeyProfile, String> {
    #[derive(Deserialize)]
    struct ProfileFile {
        profile: KeyProfile,
    }
    let mut file: ProfileFile =
        toml::from_str(toml).map_err(|e| format!("failed to parse profile TOML: {e}"))?;
    file.profile.validate_and_clamp();
    Ok(file.profile)
}

// ── Validation & clamping ───────────────────────────────────────────

/// Safe DSP parameter ranges.
///
/// These are enforced by [`KeyProfile::validate_and_clamp`] when loading
/// user-supplied TOML profiles. The hardcoded default profile bypasses
/// validation (it is constructed in code and known to be safe).
///
/// # Why these bounds
///
/// - `click_freq` / `spring_freq`: 500–8000 Hz keeps the bandpass within
///   the audible range and prevents the TPT `tan(pi * fc / fs)` pre-compute
///   from saturating at Nyquist.
/// - `resonance` (Q factor): 0.1–10.0. The render path computes
///   `k = 1.0 / resonance`, so:
///   - `resonance = 0.0` would divide by zero (panic).
///   - `resonance < 0.1` produces a very wide filter that lets through
///     too much noise (click loses its percussive character).
///   - `resonance > 10.0` produces a very narrow filter that becomes
///     numerically unstable in single-precision TPT SVF (state variables
///     can grow unbounded).
/// - `decay_coefficient`: 0.0–0.9999. MUST be strictly less than 1.0 to
///   prevent infinite loops (a coefficient >= 1.0 means the envelope never
///   decays, so the voice never deactivates and consumes a polyphony slot
///   forever).
/// - `amplitude` / `mix`: 0.0–1.0 are sane audible ranges.
/// - `duration_ms`: 0.1–10.0 ms covers the shortest mechanical tick to
///   the longest practical spring ring-out burst.
/// - `voice_off_threshold`: 1e-7–1e-2. Too small and voices never
///   deactivate; too large and they cut off audibly.
pub mod ranges {
    pub const CLICK_FREQ_MIN: f32 = 500.0;
    pub const CLICK_FREQ_MAX: f32 = 8000.0;

    pub const SPRING_FREQ_MIN: f32 = 500.0;
    pub const SPRING_FREQ_MAX: f32 = 8000.0;

    /// Minimum resonance (Q). Below this the filter is too wide.
    pub const RESONANCE_MIN: f32 = 0.1;
    /// Maximum resonance (Q). Above this the TPT SVF becomes numerically
    /// unstable in single-precision float.
    pub const RESONANCE_MAX: f32 = 10.0;

    pub const AMPLITUDE_MIN: f32 = 0.0;
    pub const AMPLITUDE_MAX: f32 = 1.0;

    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;

    pub const DURATION_MS_MIN: f32 = 0.1;
    pub const DURATION_MS_MAX: f32 = 10.0;

    pub const DECAY_COEFF_MIN: f32 = 0.0;
    pub const DECAY_COEFF_MAX: f32 = 0.9999;

    pub const VOICE_OFF_THRESHOLD_MIN: f32 = 1e-7;
    pub const VOICE_OFF_THRESHOLD_MAX: f32 = 1e-2;

    /// Ambient noise level range. 0.0 = silent, 1.0 = full.
    pub const AMBIENT_NOISE_LEVEL_MIN: f32 = 0.0;
    pub const AMBIENT_NOISE_LEVEL_MAX: f32 = 1.0;

    /// Ambient noise decay. Same constraint as DECAY_COEFF: must be < 1.0
    /// to prevent infinite rattle.
    pub const AMBIENT_NOISE_DECAY_MIN: f32 = 0.0;
    pub const AMBIENT_NOISE_DECAY_MAX: f32 = 0.9999;

    /// Housing "thock" frequency range (v4.2.0+).
    ///
    /// 100 Hz is the lower bound of perceived "body" — below this the
    /// housing impact blends into sub-bass rumble and loses its
    /// percussive character. 1000 Hz is the upper bound — above this
    /// the housing layer starts overlapping with the spring layer's
    /// frequency range and the two become indistinguishable.
    pub const HOUSING_FREQ_MIN: f32 = 100.0;
    pub const HOUSING_FREQ_MAX: f32 = 1000.0;
}

/// Helper: clamp a value into `[min, max]`, treating NaN and infinities
/// as out-of-bounds. Returns `(clamped_value, was_clamped)`.
///
/// - `NaN` is replaced by `min`.
/// - `+inf` is replaced by `max`.
/// - `-inf` is replaced by `min`.
/// - Finite values outside the range are clamped to the nearest bound.
fn clamp_finite(value: f32, min: f32, max: f32) -> (f32, bool) {
    if value.is_nan() {
        return (min, true);
    }
    if value.is_infinite() {
        return (if value > 0.0 { max } else { min }, true);
    }
    if value < min {
        return (min, true);
    }
    if value > max {
        return (max, true);
    }
    (value, false)
}

impl KeyProfile {
    /// Validate and clamp all DSP parameters to safe ranges.
    ///
    /// This is the **guardrail** between user-supplied TOML configs and
    /// the real-time DSP render path. Without it, a user typing
    /// `decay = 9999` or `decay = infinity` in their profile would
    /// cause the TPT filter to blow up (audio cracking) or the voice
    /// envelope to never decay (CPU hang in an infinite render loop).
    ///
    /// # Behaviour
    ///
    /// - `NaN`, `Infinity`, and out-of-bounds values are silently clamped
    ///   to the nearest safe boundary.
    /// - Each clamped parameter emits a `log::warn!` line identifying
    ///   the field, the offending value, and the clamped replacement.
    /// - The method is idempotent: running it on an already-valid
    ///   profile is a no-op.
    ///
    /// # When to call
    ///
    /// - **Always** when loading from a TOML file (`load_*` functions).
    /// - **Never** on the hardcoded default profile (it is constructed
    ///   in code and already safe — calling this is harmless but
    ///   wasteful).
    pub fn validate_and_clamp(&mut self) {
        // Click parameters
        let (v, c) = clamp_finite(
            self.click.frequency,
            ranges::CLICK_FREQ_MIN,
            ranges::CLICK_FREQ_MAX,
        );
        if c {
            log::warn!(
                "Invalid click.frequency {}, clamping to {}",
                self.click.frequency,
                v
            );
        }
        self.click.frequency = v;

        let (v, c) = clamp_finite(
            self.click.resonance,
            ranges::RESONANCE_MIN,
            ranges::RESONANCE_MAX,
        );
        if c {
            log::warn!(
                "Invalid click.resonance {}, clamping to {}",
                self.click.resonance,
                v
            );
        }
        self.click.resonance = v;

        let (v, c) = clamp_finite(
            self.click.duration_ms,
            ranges::DURATION_MS_MIN,
            ranges::DURATION_MS_MAX,
        );
        if c {
            log::warn!(
                "Invalid click.duration_ms {}, clamping to {}",
                self.click.duration_ms,
                v
            );
        }
        self.click.duration_ms = v;

        let (v, c) = clamp_finite(
            self.click.amplitude,
            ranges::AMPLITUDE_MIN,
            ranges::AMPLITUDE_MAX,
        );
        if c {
            log::warn!(
                "Invalid click.amplitude {}, clamping to {}",
                self.click.amplitude,
                v
            );
        }
        self.click.amplitude = v;

        // Spring parameters
        let (v, c) = clamp_finite(
            self.spring.frequency,
            ranges::SPRING_FREQ_MIN,
            ranges::SPRING_FREQ_MAX,
        );
        if c {
            log::warn!(
                "Invalid spring.frequency {}, clamping to {}",
                self.spring.frequency,
                v
            );
        }
        self.spring.frequency = v;

        let (v, c) = clamp_finite(
            self.spring.resonance,
            ranges::RESONANCE_MIN,
            ranges::RESONANCE_MAX,
        );
        if c {
            log::warn!(
                "Invalid spring.resonance {}, clamping to {}",
                self.spring.resonance,
                v
            );
        }
        self.spring.resonance = v;

        let (v, c) = clamp_finite(self.spring.mix, ranges::MIX_MIN, ranges::MIX_MAX);
        if c {
            log::warn!("Invalid spring.mix {}, clamping to {}", self.spring.mix, v);
        }
        self.spring.mix = v;

        // Decay parameters — the critical ones for infinite-loop prevention
        let (v, c) = clamp_finite(
            self.decay.coefficient,
            ranges::DECAY_COEFF_MIN,
            ranges::DECAY_COEFF_MAX,
        );
        if c {
            log::warn!(
                "Invalid decay.coefficient {}, clamping to {} (must be < 1.0 to prevent infinite loops)",
                self.decay.coefficient,
                v
            );
        }
        self.decay.coefficient = v;

        let (v, c) = clamp_finite(
            self.decay.voice_off_threshold,
            ranges::VOICE_OFF_THRESHOLD_MIN,
            ranges::VOICE_OFF_THRESHOLD_MAX,
        );
        if c {
            log::warn!(
                "Invalid decay.voice_off_threshold {}, clamping to {}",
                self.decay.voice_off_threshold,
                v
            );
        }
        self.decay.voice_off_threshold = v;

        // Ambient parameters
        let (v, c) = clamp_finite(
            self.ambient.noise_level,
            ranges::AMBIENT_NOISE_LEVEL_MIN,
            ranges::AMBIENT_NOISE_LEVEL_MAX,
        );
        if c {
            log::warn!(
                "Invalid ambient.noise_level {}, clamping to {}",
                self.ambient.noise_level,
                v
            );
        }
        self.ambient.noise_level = v;

        let (v, c) = clamp_finite(
            self.ambient.noise_decay,
            ranges::AMBIENT_NOISE_DECAY_MIN,
            ranges::AMBIENT_NOISE_DECAY_MAX,
        );
        if c {
            log::warn!(
                "Invalid ambient.noise_decay {}, clamping to {} (must be < 1.0 to prevent infinite rattle)",
                self.ambient.noise_decay,
                v
            );
        }
        self.ambient.noise_decay = v;

        // Housing parameters (v4.2.0+)
        let (v, c) = clamp_finite(
            self.housing.frequency,
            ranges::HOUSING_FREQ_MIN,
            ranges::HOUSING_FREQ_MAX,
        );
        if c {
            log::warn!(
                "Invalid housing.frequency {}, clamping to {}",
                self.housing.frequency,
                v
            );
        }
        self.housing.frequency = v;

        let (v, c) = clamp_finite(
            self.housing.resonance,
            ranges::RESONANCE_MIN,
            ranges::RESONANCE_MAX,
        );
        if c {
            log::warn!(
                "Invalid housing.resonance {}, clamping to {}",
                self.housing.resonance,
                v
            );
        }
        self.housing.resonance = v;

        let (v, c) = clamp_finite(self.housing.mix, ranges::MIX_MIN, ranges::MIX_MAX);
        if c {
            log::warn!(
                "Invalid housing.mix {}, clamping to {}",
                self.housing.mix,
                v
            );
        }
        self.housing.mix = v;
    }
}

// ── Per-key override profile loading ────────────────────────────────

/// A single per-key override entry parsed from a `[[keys]]` TOML block.
///
/// Only the `scancode` field is required; all other fields are optional
/// and fall back to the `[default]` profile when `None`. This struct is
/// a deserialisation helper — it is collapsed into a complete
/// [`KeyProfile`] (with defaults merged in) at load time.
///
/// Each parameter sub-struct uses `Option<OverrideXxx>` (with all-Option
/// fields) rather than `Option<ClickParams>` so users can override
/// individual fields (e.g. just `frequency`) without specifying the
/// entire click table.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct KeyOverride {
    /// Hardware evdev scancode this override applies to.
    pub scancode: u32,
    /// Optional click parameter overrides.
    #[serde(default)]
    pub click: Option<OverrideClick>,
    /// Optional spring parameter overrides.
    #[serde(default)]
    pub spring: Option<OverrideSpring>,
    /// Optional decay parameter overrides.
    #[serde(default)]
    pub decay: Option<OverrideDecay>,
    /// Optional ambient parameter overrides.
    #[serde(default)]
    pub ambient: Option<OverrideAmbient>,
    /// Optional housing parameter overrides (v4.2.0+).
    #[serde(default)]
    pub housing: Option<OverrideHousing>,
}

/// Optional click parameter overrides for a `[[keys]]` block.
///
/// Every field is `Option<f32>` so users can override individual
/// parameters (e.g. just `frequency`) without specifying the entire
/// click table. `None` fields inherit from the `[default]` profile.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OverrideClick {
    #[serde(default)]
    pub frequency: Option<f32>,
    #[serde(default)]
    pub resonance: Option<f32>,
    #[serde(default)]
    pub duration_ms: Option<f32>,
    #[serde(default)]
    pub amplitude: Option<f32>,
}

/// Optional spring parameter overrides for a `[[keys]]` block.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OverrideSpring {
    #[serde(default)]
    pub frequency: Option<f32>,
    #[serde(default)]
    pub resonance: Option<f32>,
    #[serde(default)]
    pub mix: Option<f32>,
}

/// Optional decay parameter overrides for a `[[keys]]` block.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OverrideDecay {
    #[serde(default)]
    pub coefficient: Option<f32>,
    #[serde(default)]
    pub voice_off_threshold: Option<f32>,
}

/// Optional ambient parameter overrides for a `[[keys]]` block.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OverrideAmbient {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub noise_level: Option<f32>,
    #[serde(default)]
    pub noise_decay: Option<f32>,
}

/// Optional housing parameter overrides for a `[[keys]]` block (v4.2.0+).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OverrideHousing {
    #[serde(default)]
    pub frequency: Option<f32>,
    #[serde(default)]
    pub resonance: Option<f32>,
    #[serde(default)]
    pub mix: Option<f32>,
}

/// A loaded acoustic profile with optional per-key overrides.
///
/// Parsed from a TOML file with the structure:
///
/// ```toml
/// [default]
/// [default.click]
/// frequency = 4500.0
/// # ...
/// [default.spring]
/// # ...
/// [default.decay]
/// # ...
///
/// [[keys]]
/// scancode = 28  # Enter
/// [keys.click]
/// frequency = 3000.0  # deeper thump
/// ```
///
/// The `[default]` table is mandatory; `[[keys]]` blocks are optional
/// and may appear zero or more times.
#[derive(Debug, Clone)]
pub struct ProfileWithOverrides {
    /// The default profile applied to any scancode without an override.
    pub default: KeyProfile,
    /// Per-scancode overrides. The KeyProfile values are complete (defaults
    /// merged with overrides) and already validated/clamped.
    pub overrides: std::collections::HashMap<u32, KeyProfile>,
}

impl ProfileWithOverrides {
    /// Load from a TOML file on disk.
    ///
    /// Both the default profile and any per-key overrides are validated
    /// and clamped to safe DSP ranges before being returned.
    pub fn from_file(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        Self::parse(&content)
    }

    /// Parse from a TOML string.
    ///
    /// Expects a `[default]` table (mandatory) and zero or more
    /// `[[keys]]` blocks (optional). Both are validated and clamped.
    ///
    /// Named `parse` rather than `from_str` to avoid clashing with the
    /// `std::str::FromStr` trait method (which would require an
    /// associated error type, complicating the public API).
    pub fn parse(toml_str: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct File {
            default: KeyProfile,
            #[serde(default)]
            keys: Vec<KeyOverride>,
        }

        let file: File =
            toml::from_str(toml_str).map_err(|e| format!("failed to parse profile TOML: {e}"))?;

        let mut default = file.default;
        default.validate_and_clamp();

        let mut overrides = std::collections::HashMap::with_capacity(file.keys.len());
        for ko in file.keys {
            // Merge: start from default, apply any provided overrides.
            let mut merged = default;
            if let Some(click) = ko.click {
                if let Some(v) = click.frequency {
                    merged.click.frequency = v;
                }
                if let Some(v) = click.resonance {
                    merged.click.resonance = v;
                }
                if let Some(v) = click.duration_ms {
                    merged.click.duration_ms = v;
                }
                if let Some(v) = click.amplitude {
                    merged.click.amplitude = v;
                }
            }
            if let Some(spring) = ko.spring {
                if let Some(v) = spring.frequency {
                    merged.spring.frequency = v;
                }
                if let Some(v) = spring.resonance {
                    merged.spring.resonance = v;
                }
                if let Some(v) = spring.mix {
                    merged.spring.mix = v;
                }
            }
            if let Some(decay) = ko.decay {
                if let Some(v) = decay.coefficient {
                    merged.decay.coefficient = v;
                }
                if let Some(v) = decay.voice_off_threshold {
                    merged.decay.voice_off_threshold = v;
                }
            }
            if let Some(housing) = ko.housing {
                if let Some(v) = housing.frequency {
                    merged.housing.frequency = v;
                }
                if let Some(v) = housing.resonance {
                    merged.housing.resonance = v;
                }
                if let Some(v) = housing.mix {
                    merged.housing.mix = v;
                }
            }
            // Re-clamp the merged profile to catch override values that
            // are out of bounds.
            merged.validate_and_clamp();
            overrides.insert(ko.scancode, merged);
        }

        Ok(Self { default, overrides })
    }

    /// Look up the profile for a given scancode, falling back to the
    /// default if no per-key override exists.
    pub fn for_scancode(&self, scancode: u32) -> KeyProfile {
        self.overrides
            .get(&scancode)
            .copied()
            .unwrap_or(self.default)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a profile with all fields set to known-good values.
    fn good_profile() -> KeyProfile {
        KeyProfile {
            click: ClickParams {
                frequency: 4500.0,
                resonance: 2.0,
                duration_ms: 1.5,
                amplitude: 0.8,
            },
            spring: SpringParams {
                frequency: 1800.0,
                resonance: 3.5,
                mix: 0.6,
            },
            decay: DecayParams {
                coefficient: 0.9994,
                voice_off_threshold: 1e-5,
            },
            ambient: AmbientParams {
                enabled: false,
                noise_level: 0.1,
                noise_decay: 0.99,
            },
            housing: HousingParams {
                frequency: 400.0,
                resonance: 2.0,
                mix: 0.4,
            },
        }
    }

    #[test]
    fn validate_and_clamp_is_noop_on_safe_profile() {
        let mut p = good_profile();
        let before = p;
        p.validate_and_clamp();
        // Bit-for-bit equality — no clamping should have occurred.
        assert_eq!(p.click.frequency, before.click.frequency);
        assert_eq!(p.click.resonance, before.click.resonance);
        assert_eq!(p.click.duration_ms, before.click.duration_ms);
        assert_eq!(p.click.amplitude, before.click.amplitude);
        assert_eq!(p.spring.frequency, before.spring.frequency);
        assert_eq!(p.spring.resonance, before.spring.resonance);
        assert_eq!(p.spring.mix, before.spring.mix);
        assert_eq!(p.decay.coefficient, before.decay.coefficient);
        assert_eq!(
            p.decay.voice_off_threshold,
            before.decay.voice_off_threshold
        );
    }

    #[test]
    fn validate_and_clamp_clamps_out_of_bounds_decay() {
        let mut p = good_profile();
        // The killer case from the prompt: decay = 9999. Without
        // clamping this would cause an infinite render loop because
        // the envelope never decays below voice_off_threshold.
        p.decay.coefficient = 9999.0;
        p.validate_and_clamp();
        assert_eq!(p.decay.coefficient, ranges::DECAY_COEFF_MAX);
        assert!(p.decay.coefficient < 1.0, "must be strictly < 1.0");
    }

    #[test]
    fn validate_and_clamp_clamps_infinite_decay() {
        let mut p = good_profile();
        p.decay.coefficient = f32::INFINITY;
        p.validate_and_clamp();
        assert_eq!(p.decay.coefficient, ranges::DECAY_COEFF_MAX);
    }

    #[test]
    fn validate_and_clamp_clamps_nan_decay() {
        let mut p = good_profile();
        p.decay.coefficient = f32::NAN;
        p.validate_and_clamp();
        // NaN maps to MIN (the "safest" choice — instant decay).
        assert_eq!(p.decay.coefficient, ranges::DECAY_COEFF_MIN);
        assert!(!p.decay.coefficient.is_nan());
    }

    #[test]
    fn validate_and_clamp_clamps_negative_decay() {
        let mut p = good_profile();
        p.decay.coefficient = -5.0;
        p.validate_and_clamp();
        assert_eq!(p.decay.coefficient, ranges::DECAY_COEFF_MIN);
    }

    #[test]
    fn validate_and_clamp_clamps_excessive_resonance() {
        let mut p = good_profile();
        // resonance = 9999 would make k = 1/9999 ≈ 0, producing an
        // unstable TPT SVF (infinite Q).
        p.click.resonance = 9999.0;
        p.validate_and_clamp();
        assert_eq!(p.click.resonance, ranges::RESONANCE_MAX);
    }

    #[test]
    fn validate_and_clamp_clamps_zero_resonance() {
        let mut p = good_profile();
        // resonance = 0 would divide by zero in `k = 1/resonance`.
        p.spring.resonance = 0.0;
        p.validate_and_clamp();
        assert_eq!(p.spring.resonance, ranges::RESONANCE_MIN);
    }

    #[test]
    fn validate_and_clamp_clamps_frequency_to_audible_range() {
        let mut p = good_profile();
        p.click.frequency = 100_000.0; // way above Nyquist
        p.spring.frequency = 10.0; // below audible
        p.validate_and_clamp();
        assert_eq!(p.click.frequency, ranges::CLICK_FREQ_MAX);
        assert_eq!(p.spring.frequency, ranges::SPRING_FREQ_MIN);
    }

    #[test]
    fn validate_and_clamp_clamps_nan_amplitude() {
        let mut p = good_profile();
        p.click.amplitude = f32::NAN;
        p.validate_and_clamp();
        assert_eq!(p.click.amplitude, ranges::AMPLITUDE_MIN);
        assert!(!p.click.amplitude.is_nan());
    }

    #[test]
    fn profile_with_overrides_parses_default_only() {
        let toml = r#"
[default]
[default.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[default.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[default.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001
"#;
        let p = ProfileWithOverrides::parse(toml).expect("parse should succeed");
        assert!(p.overrides.is_empty());
        assert_eq!(p.default.click.frequency, 4500.0);
        assert_eq!(p.default.decay.coefficient, 0.9994);
    }

    #[test]
    fn profile_with_overrides_parses_per_key_override() {
        let toml = r#"
[default]
[default.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[default.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[default.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001

[[keys]]
scancode = 28
[keys.click]
frequency = 3000.0
"#;
        let p = ProfileWithOverrides::parse(toml).expect("parse should succeed");
        assert_eq!(p.overrides.len(), 1);

        // Scancode 28 (Enter) should get the overridden click frequency.
        let enter_profile = p.for_scancode(28);
        assert_eq!(enter_profile.click.frequency, 3000.0);
        // But other parameters should fall back to default.
        assert_eq!(enter_profile.click.resonance, 2.0);
        assert_eq!(enter_profile.spring.frequency, 1800.0);
        assert_eq!(enter_profile.decay.coefficient, 0.9994);

        // Any other scancode should get the default profile entirely.
        let other = p.for_scancode(999);
        assert_eq!(other.click.frequency, 4500.0);
    }

    #[test]
    fn profile_with_overrides_clamps_invalid_override_values() {
        let toml = r#"
[default]
[default.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[default.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[default.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001

[[keys]]
scancode = 28
[keys.decay]
coefficient = 9999.0
"#;
        let p = ProfileWithOverrides::parse(toml).expect("parse should succeed");
        let enter_profile = p.for_scancode(28);
        // The out-of-bounds decay coefficient should have been clamped.
        assert_eq!(enter_profile.decay.coefficient, ranges::DECAY_COEFF_MAX);
        assert!(enter_profile.decay.coefficient < 1.0);
    }

    #[test]
    fn profile_with_overrides_rejects_missing_default() {
        let toml = r#"
[[keys]]
scancode = 28
[keys.click]
frequency = 3000.0
"#;
        let result = ProfileWithOverrides::parse(toml);
        assert!(result.is_err(), "missing [default] must error");
    }

    #[test]
    fn load_profile_from_str_clamps_legacy_format() {
        // The legacy [profile] format (used pre-v0.3.0) should still
        // parse and be validated.
        let toml = r#"
[profile]
[profile.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[profile.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[profile.decay]
coefficient = 1.5
voice_off_threshold = 0.00001
"#;
        let p = load_profile_from_str(toml).expect("parse should succeed");
        // decay = 1.5 (>= 1.0) MUST be clamped to prevent infinite loops.
        assert!(p.decay.coefficient < 1.0);
        assert_eq!(p.decay.coefficient, ranges::DECAY_COEFF_MAX);
    }
}
