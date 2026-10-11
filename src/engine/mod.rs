// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zactrix Engine — polyphonic voice pool and DSP orchestration.
//!
//! This crate manages the lifecycle of sounding voices, handles polyphony
//! with oldest-first voice stealing, and mixes active voices into a stereo
//! output buffer. It depends on `zactrix-profiles` for the actual DSP
//! (performed by the [`AcousticModel`](crate::profiles::AcousticModel) trait).
//!
//! ## Zero-Allocation Guarantee
//!
//! The render path ([`VoicePool::process_sample`]) performs no heap allocation.
//! All voices are pre-allocated in a fixed-size array.

mod pool;
mod voice;

pub use pool::VoicePool;
pub use voice::Voice;

// night-improve-21: the former convenience re-exports of SAMPLE_RATE
// and MAX_POLYPHONY are retired with the unification — the profiles
// module is one path away in the same crate, and a binary-only crate
// carries no external consumers that would need the bridge.
