// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-depth-hardening-2, the proven datapath (night-depth-hardening-3's
//! Kani move, the zelynic lineage): bounded model checking over the pure
//! laws the render and control paths ride. Each harness quantifies over
//! the WHOLE reachable input space of one law — not a sample, the space —
//! and the solver proves the invariant, not a test's coverage.
//!
//! The laws the buyer's audit asks about first:
//! - the volume clamp: an untrusted control-map percent reaches the
//!   gain path, and the clamp cell is the last gate before the render;
//!   no input may produce a NaN bit pattern or escape the cell's range;
//! - the gain monotonicity: louder percent never renders quieter;
//! - the noise mapping: the PRNG's u32 domain maps inside [-1, 1],
//!   every bit pattern, no panic, no NaN.
//!
//! night-frontier-Z6 widened the proven surface to the remaining
//! integer laws the ledger had only measured (entry 44's deferred
//! half): the errno classification table (entry 28), the orphan
//! decision ladder (entry 25), the privilege polarity table (entry
//! 26), the no-config preset resolution (entry 27), the pool's
//! NaN-silence mix net end to end, and the envelope decay chain's
//! monotonicity with the absorbing voice-off law — provable in SMT
//! now precisely because night-frontier-Z5 took the transcendentals
//! out of the reachable state space (the decay chain is pure
//! mul/add).
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

    // ── night-frontier-Z6: the measured integer laws become proved ────
    //
    // Entry 44 proved the datapath's three laws and left the control
    // plane's integer laws "measured, pinned by tests". The tests stay
    // (they pin exact wordings and wire codes); the harnesses below
    // add what a test cannot: the WHOLE input space of each law.

    use crate::commands::player::preset_by_index;
    use crate::ebpf::attach_error::AttachError;
    use crate::profiles::{AcousticModel, KeyProfile, KeyTrigger, SynthState};
    use crate::session::orphan::{
        LadderStep, PLAYER_ARG, cmdline_matches_player, ladder_next, pid_is_our_player,
    };
    use crate::session::privilege::{
        attach_requirement, detach_requirement, network_refusal, session_refusal,
    };
    use std::os::unix::ffi::OsStrExt as _;
    use std::sync::atomic::Ordering;

    /// The orphan ladder's totality + escalation shape (entry 25): for
    /// EVERY (stage, alive, elapsed) triple the next step is one of
    /// the table's five, the stage never de-escalates, and a dead pid
    /// concludes immediately — no input space, no panic, no drift.
    #[kani::proof]
    fn ladder_next_is_total_and_shape_true() {
        let stage: u8 = kani::any();
        let alive: bool = kani::any();
        let elapsed: bool = kani::any();
        let step = ladder_next(stage, alive, elapsed);
        match (stage, alive, elapsed) {
            (_, false, _) => {
                assert_eq!(step, LadderStep::ConcludeGone);
            }
            (0, true, false) => assert_eq!(step, LadderStep::Observe),
            (0, true, true) => assert_eq!(step, LadderStep::SendTerm),
            (1, true, false) => assert_eq!(step, LadderStep::Observe),
            (1, true, true) => assert_eq!(step, LadderStep::SendKill),
            (_, true, false) => assert_eq!(step, LadderStep::Observe),
            (_, true, true) => assert_eq!(step, LadderStep::ConcludeSurvived),
        }
    }

    /// The NO-ORPHAN law (entry 25): every arm of the ladder is
    /// reachable — a concrete witness input for each, so no step is a
    /// dead decision the runtime can never walk into.
    #[kani::proof]
    fn ladder_has_no_orphan_arm() {
        assert_eq!(ladder_next(0, false, false), LadderStep::ConcludeGone);
        assert_eq!(ladder_next(0, true, false), LadderStep::Observe);
        assert_eq!(ladder_next(0, true, true), LadderStep::SendTerm);
        assert_eq!(ladder_next(1, true, true), LadderStep::SendKill);
        assert_eq!(ladder_next(2, true, true), LadderStep::ConcludeSurvived);
    }

    /// The input-space collapse: every stage at or above 2 behaves as
    /// stage 2 — the ladder's state space is exactly three stages, so
    /// a corrupted stage byte cannot invent a fourth behavior.
    #[kani::proof]
    fn ladder_collapses_above_stage_two() {
        let stage: u8 = kani::any();
        let alive: bool = kani::any();
        let elapsed: bool = kani::any();
        kani::assume(stage >= 2);
        assert_eq!(
            ladder_next(stage, alive, elapsed),
            ladder_next(2, alive, elapsed),
            "stage {stage} invented a behavior stage 2 does not have"
        );
    }

    /// The identity gate's tamper law (entry 25, night-total-lts-1):
    /// a cmdline that matches the exact player shape STOPS matching
    /// when ANY single byte flips — the root reaper's target cannot be
    /// retargeted by one mutated byte of map-driven /proc data. The
    /// quantifier shape is honest about the verifier's reach: string
    /// machinery over symbolic bytes is SAT poison (the solver burns
    /// minutes the lane's cap does not have — measured, not guessed),
    /// so the harness pins the REAL player shape with the five
    /// semantically distinct byte slots — path start, path end, the
    /// separator NUL, the argument's first byte, the trailing NUL —
    /// each flipped to an arbitrary other byte, and the every-shape
    /// generality stays with the unit tests, which enumerate the
    /// refusal shapes (near-misses, extra tokens, empty cmdlines)
    /// concretely in test/session/orphan_tests.rs.
    #[kani::proof]
    #[kani::unwind(64)]
    fn cmdline_tamper_breaks_the_real_player_shape() {
        // The concrete matching cmdline: this binary's path shape
        // followed by the player verb — the exact bytes the sweep
        // would match in production.
        const BASE: &[u8] = b"/usr/bin/zylaxion\0_play\0";
        let exe = std::ffi::OsStr::new("/usr/bin/zylaxion");
        assert!(
            cmdline_matches_player(BASE, exe),
            "the representative shape must match"
        );
        // The SEMANTIC slots only: path start, path end, the separator
        // NUL, the argument's first byte, and the trailing NUL — the
        // five byte roles the exact-match law distinguishes, each
        // flipped to an arbitrary other byte in its own unrolled
        // assertion. One matcher call per assertion keeps the formula
        // linear (a 24-position loop multiplies the iterator machinery
        // into an hour-long SAT encode; the probe proved the matcher
        // itself verifies in one second).
        let mut tampered = BASE.to_vec();
        for slot in [0_usize, 15, 16, 17, 23] {
            let value: u8 = kani::any();
            kani::assume(value != BASE[slot]);
            tampered[slot] = value;
            let ok = cmdline_matches_player(&tampered, exe);
            tampered[slot] = BASE[slot];
            assert!(!ok, "slot {slot} mutated and the matcher still accepted");
        }
    }

    /// The gate's refusal-by-construction: an unreadable cmdline or a
    /// missing exe path is a refusal, never a maybe — the privileged
    /// kill only ever fires on a provably-ours player.
    #[kani::proof]
    fn pid_identity_gate_refuses_unreadable_shapes() {
        let raw: [u8; 4] = kani::any();
        let exe = std::ffi::OsStr::new("zylaxion");
        assert!(!pid_is_our_player(None, Some(exe)));
        assert!(!pid_is_our_player(Some(&raw), None));
        assert!(!pid_is_our_player(None, None));
    }

    /// The privilege polarity table (entry 26) over the WHOLE fact
    /// space: root is refused at the network and session surfaces,
    /// REQUIRED at attach, and detach refuses exactly when a non-root
    /// caller cannot complete either removal half — the audit table,
    /// proven, not documented.
    #[kani::proof]
    fn privilege_table_matches_the_audit() {
        let root: bool = kani::any();
        let pins: bool = kani::any();
        let dir: bool = kani::any();

        // The verdicts compare as plain bools: an assert_eq! here would
        // Debug-format the Option<&'static str> into the panic payload —
        // the whole refusal sentence flowing through the formatter under
        // symbolic selection, which is the SAT encoding's death valley.
        // The law is the POLARITY, and bools carry it without the bytes.
        assert!(
            network_refusal(root).is_some() == root,
            "the network verdict drifted"
        );
        assert!(
            session_refusal(root).is_some() == root,
            "the session verdict drifted"
        );
        assert!(
            attach_requirement(root).is_some() == !root,
            "attach's requirement is not the mirror of the refusals"
        );
        assert!(
            detach_requirement(root, pins, dir).is_some() == (!root && (!pins || !dir)),
            "the detach verdict drifted off the two-removal contract"
        );

        // (The full wording laws — the exact sentences and the sudo
        // tip — are CONCRETE properties of four fixed strings, and any
        // string op over solver-selected static strings blows the SAT
        // encoding up without adding a law the test tree does not
        // already pin: test/session/privilege_tests.rs owns the
        // wordings verbatim.)
    }

    /// The no-config preset resolution (entry 27) over the WHOLE u32
    /// index domain: an untrusted control-map index always resolves to
    /// a finite, in-band profile — in range it IS that registry entry,
    /// out of range it collapses to the one compiled default, and the
    /// path never panics (the registry is compiled-in, never empty).
    #[kani::proof]
    fn preset_by_index_is_total_and_valid() {
        let index: u32 = kani::any();
        let profile = preset_by_index(index);
        assert!(
            profile.click.frequency.is_finite() && profile.click.frequency > 0.0,
            "index {index} resolved a dead click frequency"
        );
        assert!(
            profile.click.resonance.is_finite() && profile.click.resonance > 0.0,
            "index {index} resolved a dead click resonance"
        );
        assert!(
            profile.decay.coefficient.is_finite()
                && (0.0..=0.9999).contains(&profile.decay.coefficient),
            "index {index} resolved a decay coefficient outside the clamp band"
        );
        let all = crate::profiles::registry_all();
        if (index as usize) < all.len() {
            assert_eq!(
                profile.click.frequency, all[index as usize].profile.click.frequency,
                "an in-range index resolved a different preset"
            );
        } else {
            // The fallback collapse: every out-of-range index lands on
            // the SAME default profile (projections of the asserted
            // fields — KeyProfile carries no PartialEq by design).
            let fallback = preset_by_index(all.len() as u32);
            assert_eq!(
                profile.click.frequency, fallback.click.frequency,
                "out-of-range indexes disagreed on the fallback"
            );
            assert_eq!(
                profile.decay.coefficient, fallback.decay.coefficient,
                "out-of-range indexes disagreed on the fallback decay"
            );
        }
    }

    /// The stub model of the mix-net harness: init_state marks the
    /// voice live, render_sample hands the pool a stereo pair from the
    /// render-reachable band — the domain the clamp laws bound, orders
    /// of magnitude inside 1e7.
    struct ProofModel;

    impl AcousticModel for ProofModel {
        fn get_profile(&self, _event: &KeyTrigger) -> KeyProfile {
            KeyProfile::default()
        }

        fn init_state(
            &self,
            _profile: &KeyProfile,
            state: &mut SynthState,
            _stereo_position: f32,
            _velocity: Option<f32>,
        ) {
            state.active = true;
        }

        fn render_sample(&self, _state: &mut SynthState) -> [f32; 2] {
            // The render-reachable band: every state the clamp laws
            // bound stays orders of magnitude inside 1e7 (the TPT
            // states and envelopes are amplitude-scale; the corpus
            // renders the extremes), so the mix-adds and the gain
            // multiply cannot overflow the f32 range mid-path.
            let left: f32 = kani::any();
            let right: f32 = kani::any();
            kani::assume(left.is_finite() && left.abs() <= 1.0e7);
            kani::assume(right.is_finite() && right.abs() <= 1.0e7);
            [left, right]
        }
    }

    /// The NaN-silence net, end to end over the REAL pool code (entry
    /// 21's law, now proved): two live voices render stereo pairs from
    /// the render-reachable band into a gain from the volume cell's
    /// PROVEN band (the volume harness bounds it at ±550: 100 percent
    /// of the 5.5x default), and the sample the user hears is ALWAYS
    /// finite and ALWAYS inside [-1, 1]. The reachable space is the
    /// honest SMT domain here — the verifier's own arithmetic checks
    /// refuse to reason THROUGH a NaN intermediate, and the net's
    /// non-finite branch (unreachable under the clamp laws, kept as
    /// the last-resort net) stays pinned by the pool test tree.
    #[kani::proof]
    #[kani::unwind(32)]
    fn pool_mix_net_contains_every_voice_and_gain() {
        let gain: f32 = kani::any();
        kani::assume(gain.is_finite() && gain.abs() <= 550.0);
        let mut pool = VoicePool::new();
        let event = KeyTrigger {
            scancode: 30,
            pressed: true,
            stereo_position: 0.0,
            velocity: None,
        };
        pool.trigger(&ProofModel, &event);
        pool.trigger(&ProofModel, &event);
        pool.volume_factor()
            .store(VoicePool::volume_bits(gain), Ordering::Relaxed);
        let [left, right] = pool.process_sample(&ProofModel);
        for (name, value) in [("left", left), ("right", right)] {
            assert!(
                value.is_finite(),
                "{name} escaped the net with a non-finite sample"
            );
            assert!(
                (-1.0..=1.0).contains(&value),
                "{name} escaped the clamp band: {value}"
            );
        }
    }

    /// The envelope decay chain's laws (the voice-off guarantee, entry
    /// 12's measured shape now proved): over the WHOLE (envelope,
    /// step, threshold) space —
    ///   1. one decay step never increases the envelope and never
    ///      leaves the non-negative band, for every step below 1.0;
    ///   2. under the certified device domain the step is STRICTLY
    ///      decreasing above the denormal-glue floor (S_MAX is the
    ///      largest reachable renormalized coefficient: the clamp
    ///      ceiling 0.9999 raised to the smallest rate ratio the
    ///      documented device span allows, 44100/384000);
    ///   3. the voice-off band is ABSORBING: an envelope below the
    ///      threshold stays below — a silenced voice can never
    ///      resurrect through decay arithmetic.
    #[kani::proof]
    fn envelope_decay_is_monotone_and_off_is_absorbing() {
        const S_MAX: f32 = 0.9999884963035583_f32; // 0.9999^(44100/384000), f32
        let envelope: f32 = kani::any();
        let step: f32 = kani::any();
        let threshold: f32 = kani::any();
        kani::assume(envelope.is_finite() && envelope > 0.0 && envelope <= 1.0e7);
        kani::assume(step.is_finite() && step >= 0.0 && step < 1.0);
        kani::assume(threshold.is_finite() && (1.0e-7..=1.0e-2).contains(&threshold));

        let next = envelope * step;
        assert!(
            next.is_finite(),
            "one decay step produced a non-finite value"
        );
        assert!(next >= 0.0, "the decay chain went negative");
        assert!(
            next <= envelope,
            "one decay step increased the envelope ({envelope} -> {next})"
        );

        // The strict step, inside the documented device domain and well
        // above the denormal-glue floor.
        kani::assume(step <= S_MAX);
        kani::assume(envelope >= 1.0e-6);
        assert!(
            next < envelope,
            "the certified step failed to strictly decay ({envelope} -> {next})"
        );

        // Absorbing off: below the threshold, decay arithmetic can
        // never carry the envelope back above it.
        kani::assume(envelope < threshold);
        assert!(
            next < threshold,
            "a silenced envelope resurrected through decay ({envelope} -> {next})"
        );

        // The bounded induction: four unrolled steps keep the chain
        // monotone and non-negative.
        let s2 = envelope * step;
        let s3 = s2 * step;
        let s4 = s3 * step;
        assert!(s2 <= envelope && s3 <= s2 && s4 <= s3, "the chain wobbled");
        assert!(s4 >= 0.0, "the chain went negative");
    }
}
