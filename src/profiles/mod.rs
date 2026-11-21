// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Acoustic profile definitions and DSP models for the Zactrix engine.
//!
//! This module provides the foundational types and traits that define how
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
mod mechanical;
mod model;
mod params;
mod profile;
mod registry;

// night-improve-22: the registry re-exports are consumed by the
// session preset resolver and the commands tree (night-dragon-lts:
// every name verified live in the non-test compilation — the scoped
// allow that hid future rot is gone).
// night-strict-warnings-1: the audit harness #[path]-includes this
// file WITHOUT those consumers (they sit outside the audit surface),
// so under the strict gate the re-export answers unused there — the
// same scoped-allow idiom the identity re-export below carries. Root
// builds verify every name live; the marker names which includer
// borrows the exemption.
#[cfg_attr(zylaxion_strict_warnings, allow(unused_imports))]
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
// night-depth-hardening-3: the kani verifier module
// (src/engine/kani_proofs.rs, the cfg(kani) bounded proofs) imports the
// noise-mapping law by its crate-visible name — the harness compiles as
// part of THIS crate, so the crate-scoped re-export is the narrow door
// it needs; the module itself stays private. The allow rides the same
// law as the identity re-export above: the harness compiles under
// nothing else, so every normal build sees an unused import.
#[allow(unused_imports)]
pub(crate) use mechanical::u32_to_signed_f32;
pub use model::{AcousticModel, SynthState};
pub use params::{
    AmbientParams, ClickParams, DecayParams, HousingParams, KeyTrigger, MAX_POLYPHONY, SAMPLE_RATE,
    SpringParams,
};
// night-improve-21: the tpt root re-export is retired (night-dragon-lts
// went further: the whole zombie module is gone — the render path
// inlines the identical TPT math). This bridge stays: KeyProfile is
// live in the non-test compilation, `ranges` is read by the wired test
// tree only, so the scoped allow remains honest.
#[allow(unused_imports)]
pub use profile::{KeyProfile, ranges};

#[cfg(test)]
#[path = "../../test/profiles/profiles_tests.rs"]
mod tests;
