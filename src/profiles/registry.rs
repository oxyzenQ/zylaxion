// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The compiled preset registry — the single source of truth for every
//! acoustic preset zylaxion ships (night-improve-22, the no-config law).
//!
//! night-improve-22 deletes the 1672-line `config.toml` and its whole
//! support stack (search paths, TOML parsing, the mtime watcher thread,
//! the `testconf` subcommand). The owner's verdict: a file format, a
//! parser, a watcher, and a validation command are absurd maintenance
//! weight for a keyboard-sound switcher. The masterclass replacement is
//! this module: every preset is compile-time Rust data — type-checked,
//! range-checked by the test tree, zero parse errors possible, zero
//! I/O, zero cold-start file lookups.
//!
//! What the user lost: nothing that survived an honest cost/benefit
//! pass. Custom DSP tuning per keystroke was the low-gains tail of the
//! investment curve; the 24 hand-tuned presets carry the actual value.
//! Preset selection is now `--preset <name>` on the CLI or the
//! `ZYLAXION_PRESET` environment variable (the Unix configuration
//! surface — no parser, no validator, no watcher). See
//! `crate::session::preset` for the resolution order.
//!
//! This module is pure data: no I/O, no environment access, no logging.
//! The session layer resolves names; the registry only answers lookups.

use crate::profiles::{
    AmbientParams, ClickParams, DecayParams, HousingParams, KeyProfile, SpringParams,
};

/// The compiled-in default preset when neither `--preset` nor
/// `ZYLAXION_PRESET` names one. "technical" — the classic crisp click —
/// is the known-good universally-available choice, unchanged from the
/// pre-registry fallback contract.
pub const DEFAULT_PRESET: &str = "technical";

/// One compiled acoustic preset: a name, a one-line description for
/// `list-presets`, and the full DSP table.
#[derive(Debug, Clone, Copy)]
pub struct Preset {
    /// The preset name as typed on the CLI (`--preset <name>`).
    pub name: &'static str,
    /// One-line human description rendered by `zylaxion list-presets`.
    pub description: &'static str,
    /// The validated DSP table. Every value below is hand-tuned and
    /// pinned in-range by the registry test tree.
    pub profile: KeyProfile,
}

// ── Table shorthand constructors ──────────────────────────────────────
// Pure `const fn` field packers: they keep the 24-entry table a flat,
// greppable, diffable list instead of nested struct literals.

const fn click(frequency: f32, resonance: f32, duration_ms: f32, amplitude: f32) -> ClickParams {
    ClickParams {
        frequency,
        resonance,
        duration_ms,
        amplitude,
    }
}

const fn spring(frequency: f32, resonance: f32, mix: f32) -> SpringParams {
    SpringParams {
        frequency,
        resonance,
        mix,
    }
}

const fn decay(coefficient: f32, voice_off_threshold: f32) -> DecayParams {
    DecayParams {
        coefficient,
        voice_off_threshold,
        coefficient_fast: 0.0,
        fast_samples_ms: 0.0,
    }
}

const fn ambient(enabled: bool, noise_level: f32, noise_decay: f32) -> AmbientParams {
    AmbientParams {
        enabled,
        noise_level,
        noise_decay,
    }
}

const fn housing(frequency: f32, resonance: f32, mix: f32) -> HousingParams {
    HousingParams {
        frequency,
        resonance,
        mix,
    }
}

const fn preset(
    name: &'static str,
    description: &'static str,
    click: ClickParams,
    spring: SpringParams,
    decay: DecayParams,
    ambient: AmbientParams,
    housing: HousingParams,
) -> Preset {
    Preset {
        name,
        description,
        profile: KeyProfile {
            click,
            spring,
            decay,
            ambient,
            housing,
        },
    }
}

/// Every preset zylaxion ships, in catalog order: the classic series,
/// the v4.0.0 legendary keyboard series, then the v4.1.0 sci-fi series.
/// Values are transcribed byte-for-byte from the retired `config.toml`
/// (night-improve-22) — the test tree pins each value in-range.
///
/// night-private-research-2xblade: the Zactrix re-tune. The Phase 0
/// audit (docs/ZACTRIX_AUDIT.md) measured 21 of 24 presets carrying
/// 99+ percent of onset energy in the first 5 ms — the housing ring
/// starved by low Q (wide bandwidths kill the ring in milliseconds).
/// The re-tune raises the housing resonance — the one ring lever the
/// range contract allows — while preserving each preset's housing
/// loudness exactly (the render multiplies the housing bandpass by
/// Q, so `mix x Q` is the level; the product is held within a few
/// percent). Click, spring, decay, and ambient rows are untouched:
/// the attack and the spring are the preset's identity, the body is
/// what was starved. topre is untouched — the audit's single A-tier
/// reference. Four presets refuse the fit on identity grounds and
/// stay as authored: whisper ("extremely quiet, short decay"),
/// rotary_encoder ("subtle, metallic" detent tick), cyber_deck
/// ("short bright pings, no body"), glass_tactile (a 5 kHz ring
/// needs Q far beyond the 10.0 range cap; a low-band body would
/// break the sharp-glass character). Every refusal is recorded in
/// docs/ZACTRIX_AUDIT.md.
pub static PRESETS: [Preset; 24] = [
    // ── Classic series ────────────────────────────────────────────────
    preset(
        "technical",
        "Crisp, loud, punchy - best for Cherry MX Blue / clicky switches",
        click(4500.0, 2.0, 1.5, 0.8),
        spring(1800.0, 3.5, 0.6),
        decay(0.9994, 1e-5),
        ambient(true, 0.15, 0.985),
        housing(500.0, 6.0, 0.10),
    ),
    preset(
        "cherryMX",
        "Balanced reference Cherry MX sound - clicky but not harsh",
        click(4200.0, 2.2, 1.8, 0.75),
        spring(1900.0, 3.0, 0.55),
        decay(0.9995, 1e-5),
        ambient(true, 0.12, 0.988),
        housing(450.0, 6.0, 0.13),
    ),
    preset(
        "classic",
        "Deeper, more resonant - old bucklespring feel, MX Brown / tactile",
        click(3200.0, 2.5, 2.5, 0.7),
        spring(1200.0, 4.0, 0.75),
        decay(0.9992, 1e-5),
        ambient(true, 0.18, 0.982),
        housing(350.0, 8.0, 0.175),
    ),
    preset(
        "studio",
        "Softer attack, longer decay - studio-monitor feel for quiet offices",
        click(4000.0, 1.2, 2.0, 0.5),
        spring(2200.0, 2.0, 0.5),
        decay(0.9997, 1e-5),
        ambient(false, 0.05, 0.99),
        housing(400.0, 7.0, 0.115),
    ),
    preset(
        "elegant",
        "Very soft, muffled, polite - low-profile and scissor switches",
        click(3000.0, 1.5, 3.0, 0.35),
        spring(1500.0, 2.5, 0.4),
        decay(0.9991, 1e-4),
        ambient(false, 0.03, 0.99),
        housing(380.0, 6.0, 0.10),
    ),
    preset(
        "whisper",
        "Extremely quiet, short decay - libraries, meetings, shared spaces",
        click(5500.0, 0.8, 0.8, 0.15),
        spring(2500.0, 0.5, 0.2),
        decay(0.9985, 1e-3),
        ambient(false, 0.02, 0.99),
        housing(500.0, 1.0, 0.2),
    ),
    preset(
        "linear",
        "Smooth attack, no tactile bump - Cherry MX Red / linear switches",
        click(3800.0, 1.0, 2.5, 0.55),
        spring(1600.0, 1.8, 0.45),
        decay(0.999, 1e-5),
        ambient(true, 0.10, 0.987),
        housing(350.0, 7.0, 0.18),
    ),
    preset(
        "tactile",
        "Sharper attack with a brief mid-frequency bump - MX Brown / tactile",
        click(4100.0, 1.8, 1.8, 0.65),
        spring(2000.0, 2.5, 0.5),
        decay(0.9993, 1e-5),
        ambient(true, 0.12, 0.985),
        housing(420.0, 5.0, 0.20),
    ),
    preset(
        "clicky",
        "Very sharp attack, high resonance, long ring - MX Blue / clicky",
        click(5200.0, 3.5, 1.2, 0.85),
        spring(2400.0, 4.5, 0.7),
        decay(0.9996, 1e-5),
        ambient(true, 0.16, 0.983),
        housing(600.0, 8.0, 0.065),
    ),
    // ── v4.0.0 legendary keyboard series ──────────────────────────────
    preset(
        "ibm_model_m",
        "The iconic buckling spring - deep, very resonant, long ring",
        click(2200.0, 5.5, 1.2, 0.85),
        spring(850.0, 7.5, 0.85),
        decay(0.99975, 1e-6),
        ambient(true, 0.20, 0.983),
        housing(400.0, 9.0, 0.135),
    ),
    preset(
        "topre",
        "Electrocapacitive thock - smooth, deep, almost no click (Realforce / HHKB)",
        click(650.0, 0.6, 5.5, 0.50),
        spring(1100.0, 1.5, 0.45),
        decay(0.9994, 1e-5),
        ambient(true, 0.08, 0.99),
        housing(200.0, 2.5, 0.8),
    ),
    preset(
        "cherry_mx_black",
        "Heavy linear (~80g) - bottom-out thud with a faint spring ping",
        click(1800.0, 0.4, 3.0, 0.55),
        spring(1300.0, 1.2, 0.35),
        decay(0.9988, 5e-5),
        ambient(true, 0.06, 0.99),
        housing(350.0, 8.0, 0.14),
    ),
    preset(
        "cherry_mx_clear",
        "Tactile, heavier than Brown - pronounced bump, louder housing",
        click(3500.0, 1.6, 2.2, 0.65),
        spring(1700.0, 2.2, 0.5),
        decay(0.9992, 1e-5),
        ambient(true, 0.12, 0.985),
        housing(400.0, 7.0, 0.155),
    ),
    preset(
        "cherry_mx_silver",
        "Fast low-profile linear (40g) - snappy bottom-out for gaming",
        click(2800.0, 0.7, 1.8, 0.6),
        spring(1900.0, 1.5, 0.35),
        decay(0.999, 2e-5),
        ambient(true, 0.05, 0.99),
        housing(550.0, 5.0, 0.235),
    ),
    preset(
        "buckling_spring",
        "Sharper, brighter buckling spring - Model F style, more ping",
        click(3000.0, 6.0, 0.8, 0.85),
        spring(1400.0, 5.5, 0.75),
        decay(0.9996, 5e-6),
        ambient(true, 0.15, 0.983),
        housing(380.0, 6.0, 0.175),
    ),
    preset(
        "alps_skcm",
        "Legendary tactile ALPS - loud, hollow housing, pronounced bump",
        click(3500.0, 3.5, 1.2, 0.9),
        spring(2000.0, 3.8, 0.65),
        decay(0.9995, 1e-5),
        ambient(true, 0.22, 0.982),
        housing(300.0, 8.0, 0.21),
    ),
    preset(
        "gateron_yellow",
        "Smooth lubed-linear thock - PBT keycaps, deep rounded bottom-out",
        click(2200.0, 0.8, 3.5, 0.6),
        spring(1500.0, 1.5, 0.4),
        decay(0.9992, 1e-5),
        ambient(true, 0.10, 0.985),
        housing(250.0, 8.0, 0.20),
    ),
    preset(
        "zealios_v2",
        "Sharp enthusiast tactile - almost clicky-feeling bump",
        click(4200.0, 2.8, 1.3, 0.75),
        spring(2000.0, 2.8, 0.55),
        decay(0.9993, 1e-5),
        ambient(true, 0.13, 0.985),
        housing(380.0, 6.0, 0.21),
    ),
    preset(
        "rotary_encoder",
        "Rotary detent click for knobs and scroll wheels - subtle, metallic",
        click(5500.0, 1.2, 0.5, 0.4),
        spring(3000.0, 2.5, 0.35),
        decay(0.9985, 5e-4),
        ambient(false, 0.0, 0.99),
        housing(700.0, 1.0, 0.1),
    ),
    // ── v4.1.0 sci-fi / futuristic series ─────────────────────────────
    preset(
        "cyber_deck",
        "Cyberpunk terminal - short bright pings, no body",
        click(6500.0, 5.0, 0.6, 0.7),
        spring(3500.0, 4.0, 0.55),
        decay(0.9987, 1e-4),
        ambient(false, 0.0, 0.99),
        housing(650.0, 1.5, 0.15),
    ),
    preset(
        "oceanic",
        "Typing underwater - muffled, weighty, long bass sustain",
        click(500.0, 0.3, 8.0, 0.65),
        spring(600.0, 2.0, 0.8),
        decay(0.9998, 1e-6),
        ambient(true, 0.18, 0.99),
        housing(100.0, 8.0, 0.105),
    ),
    preset(
        "glass_tactile",
        "Crystal glass ring - sharp, high resonance, almost musical",
        click(5200.0, 9.0, 0.5, 0.6),
        spring(4200.0, 8.0, 0.7),
        decay(0.99985, 5e-7),
        ambient(false, 0.0, 0.99),
        housing(800.0, 1.2, 0.05),
    ),
    preset(
        "retro_typewriter",
        "Typebar strike on a cast-iron frame - loud, long mid ring",
        click(2500.0, 4.0, 1.0, 0.85),
        spring(1500.0, 5.5, 0.7),
        decay(0.9996, 5e-6),
        ambient(true, 0.20, 0.985),
        housing(280.0, 8.0, 0.25),
    ),
    preset(
        "neon_pulse",
        "Synthesized holographic UI - heavily textured granular feedback",
        click(3300.0, 3.5, 1.4, 0.7),
        spring(2700.0, 3.0, 0.55),
        decay(0.9994, 1e-5),
        ambient(true, 0.35, 0.987),
        housing(450.0, 6.0, 0.17),
    ),
];

/// The full preset catalog, in catalog order.
// night-improve-22: exercised by the registry test tree and the
// session resolver wired in the same night; the unified binary's
// non-test compilation reaches it through the session resolver only.
#[allow(dead_code)]
pub fn all() -> &'static [Preset] {
    &PRESETS
}

/// Look up a preset by exact name (case-sensitive, matching the
/// previous TOML behavior — `--preset cherrymx` was never `cherryMX`).
/// The did-you-mean rescue for near-misses lives in the session
/// resolver, which owns the error UX.
// night-improve-22: exercised by the registry test tree; the session
// resolver (same night) is the binary's only consumer.
#[allow(dead_code)]
pub fn lookup(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

/// All preset names, sorted lexicographically (the list the error
/// messages and `list-presets` render — same order the old TOML
/// listing produced).
// night-improve-22: exercised by the registry test tree; the session
// resolver and list-presets (same night) are the binary's consumers.
#[allow(dead_code)]
pub fn names_sorted() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = PRESETS.iter().map(|p| p.name).collect();
    names.sort_unstable();
    names
}

#[cfg(test)]
#[path = "../../test/profiles/registry_tests.rs"]
mod tests;
