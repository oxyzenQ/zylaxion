// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Zactrix Engine — polyphonic voice pool and DSP orchestration.
//!
//! This module manages the lifecycle of sounding voices, handles polyphony
//! with oldest-first voice stealing, and mixes active voices into a stereo
//! output buffer. It uses the `profiles` module for the actual DSP
//! (performed by the [`AcousticModel`](crate::profiles::AcousticModel) trait).
//!
//! ## Zero-Allocation Guarantee
//!
//! The render path ([`VoicePool::process_sample`]) performs no heap allocation.
//! All voices are pre-allocated in a fixed-size array.
//!
//! Frozen as of c028ae6f907055807125c9a20c156cb16a3a2038. Changes
//! require a why that passes the maintenance bar: a security fix, a
//! compatibility fix, or a measured improvement proven by the audit
//! harness.

mod denormals;
mod pool;
mod voice;

// night-frontier-Z5: the certified bounded-domain transcendental lane
// — the pure-IEEE-arithmetic replacements for every libm
// transcendental on the audio path. Its addition, and the init/render
// call-site swap onto it, are the documented why this freeze pass
// admits: a measured improvement proven by the audit harness (the
// golden corpus becomes machine-independent; the certificate and the
// vectors live in the module and its test tree).
pub mod detmath;

// night-netrunner-Z2: the gain-law core — the volume cell, the
// percent-of-default gain law, and the PRNG's signed-float mapping,
// extracted to ONE crate-path-free module so the detached proofs
// package (proofs/, the sampled tier) can include the SAME bytes the
// binary ships (the fuzz harnesses' include law). A pure move: every
// former site imports from here.
pub mod gain;

pub use pool::VoicePool;

// night-depth-hardening-2, the proven datapath: the Kani bounded-model
// harnesses compile under the verifier lane only (cfg(kani)) — invisible
// to every build, test, and lint lane. The laws they prove live in this
// module (the volume cell) and across the crate's pure surfaces.
#[cfg(kani)]
mod kani_proofs;

// night-frontier-Z6: the errno-table proof family (entry 49), split
// from kani_proofs.rs under the 600-LOC law — the same verifier lane.
#[cfg(kani)]
mod kani_laws;
pub use voice::Voice;

#[cfg(test)]
#[path = "../../test/engine/denormal_policy_tests.rs"]
mod denormal_policy_tests;

#[cfg(test)]
#[path = "../../test/engine/detmath_tests.rs"]
mod detmath_tests;

// night-improve-21: the former convenience re-exports of SAMPLE_RATE
// and MAX_POLYPHONY are retired with the unification — the profiles
// module is one path away in the same crate, and a binary-only crate
// carries no external consumers that would need the bridge.

#[cfg(test)]
#[path = "../../test/engine/freeze_tests.rs"]
mod tests;
