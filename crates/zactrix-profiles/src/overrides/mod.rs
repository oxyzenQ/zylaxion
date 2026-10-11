// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Per-key overrides: patch single fields of a base profile from
//! config.toml's [[override]] tables.
//!
//! Split from the crate root in night-improve-18 (the no-mercy LOC
//! law). Every path is re-exported at the crate root, so external
//! users see no change.

use serde::Deserialize;

use crate::KeyProfile;

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
    /// v10.2.0+ (N7): fast-stage coefficient override.
    #[serde(default)]
    pub coefficient_fast: Option<f32>,
    /// v10.2.0+ (N7): fast-stage duration override (ms).
    #[serde(default)]
    pub fast_samples_ms: Option<f32>,
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
                // v10.2.0+ (N7): two-stage decay per-key overrides.
                if let Some(v) = decay.coefficient_fast {
                    merged.decay.coefficient_fast = v;
                }
                if let Some(v) = decay.fast_samples_ms {
                    merged.decay.fast_samples_ms = v;
                }
            }
            if let Some(ambient) = ko.ambient {
                if let Some(v) = ambient.enabled {
                    merged.ambient.enabled = v;
                }
                if let Some(v) = ambient.noise_level {
                    merged.ambient.noise_level = v;
                }
                if let Some(v) = ambient.noise_decay {
                    merged.ambient.noise_decay = v;
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
