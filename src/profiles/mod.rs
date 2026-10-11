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
//! - [`KeyProfile`] and sub-parameter structs — the DSP configuration tables.
//! - [`registry`] — the compiled preset catalog (night-improve-22, the
//!   no-config law: every shipped preset is compile-time data here).
//! - [`SynthState`] — zero-alloc per-voice state passed through the render path.
//! - [`MechanicalClick`] — the reference implementation for mechanical switches.
//! - [`TptSvf`] — topology-preserving SVF for numerically stable filtering.
mod mechanical;
mod model;
mod overrides;
mod params;
mod profile;
mod registry;
mod tpt;

// night-improve-22: the registry re-exports are consumed by the
// session preset resolver and the commands tree wired in this same
// night; the non-test compilation flags them until those consumers
// land, so the bridge carries the family-standard scoped allow.
#[allow(unused_imports)]
pub use registry::{
    DEFAULT_PRESET, Preset, all as registry_all, lookup as registry_lookup,
    names_sorted as registry_names,
};

pub use mechanical::MechanicalClick;
pub use model::{AcousticModel, SynthState};
// night-improve-21: the Override* re-exports are exercised by the
// session config test tree through this root path; the unified
// binary's non-test compilation never touches them, hence the allow.
// `ranges` stays on the root too: the wired profiles test tree reads
// it through this bare path.
#[allow(unused_imports)]
pub use overrides::{
    KeyOverride, OverrideAmbient, OverrideClick, OverrideDecay, OverrideHousing, OverrideSpring,
    ProfileWithOverrides,
};
pub use params::{
    AmbientParams, ClickParams, DecayParams, HousingParams, KeyTrigger, MAX_POLYPHONY, SAMPLE_RATE,
    SpringParams,
};
// night-improve-21: the `tpt::TptSvf` root re-export is retired —
// zero consumers in the unified tree; the item stays available
// through its home module. The bare path is read by the wired test
// tree only, so the non-test compilation flags it unused.
#[allow(unused_imports)]
pub use profile::{KeyProfile, ranges};

#[cfg(test)]
#[path = "../../test/profiles/profiles_tests.rs"]
mod tests;
