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
//! - [`ZactrixEngine`] — the reference implementation for mechanical
//!   switches (the Zactrix Engine; [`MechanicalClick`] is its
//!   historical-name alias).
//! - `TptSvf` — topology-preserving SVF for numerically stable filtering
//!   (the `tpt` module is internal; its filter rides the registry's
//!   compiled presets).
mod mechanical;
mod model;
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

// The ZactrixEngine re-export is the engine's public identity surface
// (docs/ZACTRIX_AUDIT.md, the CHANGELOG record); the tree's internal
// consumers read the MechanicalClick alias, so the non-test
// compilation flags the identity name until an external consumer
// lands — the same scoped allow the registry bridge above carries.
#[allow(unused_imports)]
pub use mechanical::{MechanicalClick, ZactrixEngine};
pub use model::{AcousticModel, SynthState};
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
