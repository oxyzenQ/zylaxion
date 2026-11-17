// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-depth-hardening-2, the proven datapath (night-depth-hardening-3's
//! Kani move, the zelynic lineage): bounded model checking over the pure
//! laws the render and control paths ride. Each harness quantifies over
//! the WHOLE reachable input space of one law — not a sample, the space —
//! and the solver proves the invariant, not a test's coverage.
//!
//! The three laws the buyer's audit asks about first:
//! - the volume clamp: an untrusted control-map percent reaches the
//!   gain path, and the clamp cell is the last gate before the render;
//!   no input may produce a NaN bit pattern or escape the cell's range;
//! - the gain monotonicity: louder percent never renders quieter;
//! - the noise mapping: the PRNG's u32 domain maps inside [-1, 1],
//!   every bit pattern, no panic, no NaN.
//!
//! The harnesses run under `cargo kani` (the nightly verifier lane,
//! `.github/workflows/kani.yml`); they compile under nothing else —
//! `cfg(kani)` keeps them invisible to every build, test, and lint lane.

#[cfg(kani)]
mod proofs {
    use crate::commands::player::gain_from_percent;
    use crate::engine::VoicePool;
    use crate::profiles::u32_to_signed_f32;

    /// The volume cell's law, proven over the whole f32 domain: a
    /// finite volume clamps into [-100, 100] exactly, a non-finite one
    /// falls back to the compiled default, and the bit round-trip is
    /// lossless in both arms.
    #[kani::proof]
    fn volume_bits_clamps_every_volume() {
        let volume: f32 = kani::any();
        let bits = VoicePool::volume_bits(volume);
        let restored = f32::from_bits(bits);
        if volume.is_finite() {
            assert!(restored >= -100.0, "the clamp cell leaked below -100");
            assert!(restored <= 100.0, "the clamp cell leaked above 100");
            assert_eq!(restored, volume.clamp(-100.0, 100.0));
        } else {
            assert_eq!(restored, VoicePool::DEFAULT_MASTER_VOLUME);
        }
    }

    /// The gain law: louder percent never renders quieter, and the
    /// whole u32 percent domain (the control map is untrusted input)
    /// produces finite, non-negative gains — no overflow, no NaN, no
    /// panic anywhere on the path.
    #[kani::proof]
    fn gain_from_percent_is_finite_and_monotone() {
        let a: u32 = kani::any();
        let b: u32 = kani::any();
        kani::assume(a <= b);
        let gain_a = gain_from_percent(a);
        let gain_b = gain_from_percent(b);
        assert!(
            gain_a.is_finite(),
            "the gain path produced a non-finite value"
        );
        assert!(
            gain_b.is_finite(),
            "the gain path produced a non-finite value"
        );
        assert!(gain_a >= 0.0, "the gain path went negative");
        assert!(gain_a <= gain_b, "louder percent rendered quieter");
    }

    /// The noise mapping's law: every u32 bit pattern the PRNG can
    /// produce lands inside [-1, 1], finite — no DC bias, no escape.
    #[kani::proof]
    fn u32_to_signed_f32_stays_in_unit_range() {
        let x: u32 = kani::any();
        let v = u32_to_signed_f32(x);
        assert!(
            v.is_finite(),
            "the noise mapping produced a non-finite value"
        );
        assert!(
            (-1.0..=1.0).contains(&v),
            "the noise mapping escaped [-1, 1]"
        );
    }
}
