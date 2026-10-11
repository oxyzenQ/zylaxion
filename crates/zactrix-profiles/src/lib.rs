// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
mod mechanical;
mod model;
mod overrides;
mod params;
mod profile;
mod tpt;

pub use mechanical::MechanicalClick;
pub use model::{AcousticModel, SynthState};
pub use overrides::{
    KeyOverride, OverrideAmbient, OverrideClick, OverrideDecay, OverrideHousing, OverrideSpring,
    ProfileWithOverrides,
};
pub use params::{
    AmbientParams, ClickParams, DecayParams, HousingParams, KeyTrigger, MAX_POLYPHONY,
    MasterParams, SAMPLE_RATE, SpringParams,
};
pub use profile::{KeyProfile, ranges};
pub use tpt::TptSvf;

#[cfg(test)]
#[path = "../test/profiles_tests.rs"]
mod tests;
