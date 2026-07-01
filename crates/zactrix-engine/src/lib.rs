// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zactrix Engine — polyphonic voice pool and DSP orchestration.
//!
//! This crate manages the lifecycle of sounding voices, handles polyphony
//! with oldest-first voice stealing, and mixes active voices into a stereo
//! output buffer. It depends on `zactrix-profiles` for the actual DSP
//! (performed by the [`AcousticModel`](zactrix_profiles::AcousticModel) trait).
//!
//! ## Zero-Allocation Guarantee
//!
//! The render path ([`VoicePool::process_sample`]) performs no heap allocation.
//! All voices are pre-allocated in a fixed-size array.

mod pool;
mod voice;

pub use pool::VoicePool;
pub use voice::Voice;

/// Sample rate exposed for convenience.
pub use zactrix_profiles::SAMPLE_RATE;

/// Maximum polyphony (re-exported from profiles for API consistency).
pub use zactrix_profiles::MAX_POLYPHONY;
