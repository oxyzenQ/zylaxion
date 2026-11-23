// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! night-frontier-Z6: the errno-table harness family (entry 49's
//! control-plane proofs, split from `kani_proofs.rs` under the 600-LOC
//! no-mercy law — a pure move, the same verifier lane owns it).

#[cfg(kani)]
mod errno_proofs {
    use crate::ebpf::attach_error::AttachError;

    /// The classification a marker code reconstructs to, as one byte —
    /// the harness compares KINDS, not details (the detail-carrying
    /// variants return canonical placeholder texts from `from_code`).
    /// ObjectUnsealed carries 10 (night-netrunner-Z1): the band's
    /// newest member, appended after 9 so Unknown keeps its code for
    /// every old marker file (the wire-format law).
    fn kind_of(e: &AttachError) -> u32 {
        match e {
            AttachError::FentryUnavailable { .. } => 1,
            AttachError::KprobeBlacklisted => 2,
            AttachError::SymbolMissing => 3,
            AttachError::VerifierRejected { .. } => 4,
            AttachError::PermissionDenied => 5,
            AttachError::BpffsUnavailable => 6,
            AttachError::PinFailed { .. } => 7,
            AttachError::LockdownActive => 8,
            AttachError::Unknown { .. } => 9,
            // night-depth-hardening-4: the structured errno fallthrough
            // shares Unknown's wire code — the same honest-unclassified
            // kind, so the pinned 1..=10 band and every old marker file
            // keep their meaning.
            AttachError::UnknownErrno { .. } => 9,
            AttachError::ObjectUnsealed { .. } => 10,
        }
    }

    /// The marker wire format's law (entry 28): every code in the
    /// pinned 1..=10 band reconstructs to a variant of the SAME kind
    /// that round-trips to the same code, and every code outside the
    /// band reconstructs to nothing — the doctor can never read a
    /// failure the taxonomy does not name.
    #[kani::proof]
    fn attach_error_codes_round_trip_inside_the_pinned_band() {
        let code: u32 = kani::any();
        kani::assume((1..=10).contains(&code));
        let restored =
            AttachError::from_code(code).expect("every code in the pinned band must reconstruct");
        assert_eq!(restored.code(), code, "the wire code drifted on round-trip");
        assert_eq!(
            kind_of(&restored),
            code,
            "the reconstruction changed the failure's kind"
        );
    }

    #[kani::proof]
    fn attach_error_codes_refuse_outside_the_band() {
        let code: u32 = kani::any();
        kani::assume(!(1..=10).contains(&code));
        assert!(
            AttachError::from_code(code).is_none(),
            "code {code} reconstructed outside the pinned band"
        );
    }

    /// The errno classification table (entry 28) over the WHOLE i32
    /// errno domain: the kernel's own classifications are the law —
    /// EPERM/EACCES are privilege, ENOENT is the missing symbol,
    /// EINVAL is the kprobe refusal, and every other errno lands in
    /// the honest structured fallthrough (`UnknownErrno`), never a
    /// pretend cause. night-depth-hardening-4 is what keeps this
    /// proof inside the wall: the eager `format!` + strerror render
    /// left the constructor for `Display` — the constructor stays
    /// string-machinery-free, and the whole-i32 domain verifies in
    /// seconds where the eager form ran past the twelve-minute cap
    /// alone (measured, not guessed).
    #[kani::proof]
    fn kprobe_open_classifies_the_whole_errno_domain() {
        let errno: i32 = kani::any();
        let verdict = AttachError::from_kprobe_open("input_event", errno);
        let want: u32 = match errno {
            libc::EPERM | libc::EACCES => 5,
            libc::ENOENT => 3,
            libc::EINVAL => 2,
            _ => 9,
        };
        assert_eq!(
            verdict.code(),
            want,
            "kprobe-open errno {errno} misclassified"
        );
    }

    #[kani::proof]
    fn fentry_link_classifies_the_whole_errno_domain() {
        let errno: i32 = kani::any();
        let verdict = AttachError::from_fentry_link(errno);
        let want: u32 = match errno {
            libc::EPERM | libc::EACCES => 5,
            libc::ENOENT => 3,
            libc::EINVAL => 1,
            _ => 9,
        };
        assert_eq!(
            verdict.code(),
            want,
            "fentry-link errno {errno} misclassified"
        );
    }

    #[kani::proof]
    fn link_create_classifies_the_whole_errno_domain() {
        let errno: i32 = kani::any();
        let verdict = AttachError::from_link_create(errno);
        let want: u32 = match errno {
            libc::EPERM | libc::EACCES => 5,
            libc::EINVAL => 9,
            _ => 9,
        };
        assert_eq!(
            verdict.code(),
            want,
            "link-create errno {errno} misclassified"
        );
    }

    /// The errno text table's totality: every i32 renders a non-empty
    /// sentence fragment — no bare number ever reaches the user alone.
    #[kani::proof]
    fn errno_text_is_total_and_nonempty() {
        let errno: i32 = kani::any();
        assert!(
            !crate::ebpf::attach_error::errno_text(errno).is_empty(),
            "errno {errno} rendered an empty fragment"
        );
    }

    /// The verifier-log tail's KEEP law (night-depth-hardening-4
    /// splits the former combined harness into its two laws, each on
    /// its own measured wall): at most `keep` lines survive, whatever
    /// the log's line count, the empty-line filter included, and the
    /// empty log keeps nothing. Asserted against the keep stage
    /// ([`crate::ebpf::attach_error::keep_last_lines`]) — the
    /// assume-guarantee decomposition the deep lane measured: one SAT
    /// instance carrying a heap-Vec loop AND heap-String growth is a
    /// memory bomb, the same bodies split into per-stage instances
    /// verify in seconds, and the composition is pinned byte-exact by
    /// the unit tree.
    ///
    /// The quantifier shape: the count axis rides a symbolic selector
    /// over seven CONCRETE static logs — every distinct behavior of
    /// the keep bound is a branch (the empty log, the below-bound
    /// ramp, the exact-bound case, and the clamp plateau), every byte
    /// the stage machinery touches is concrete, and only the selection
    /// is symbolic.
    #[kani::proof]
    #[kani::unwind(64)]
    fn verifier_tail_keeps_at_most_the_keep_bound() {
        const LOGS: [&str; 7] = [
            "\n",
            "\nxxxxxxxx\n",
            "\nxxxxxxxx\nxxxxxxxx\n",
            "\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\n",
            "\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\n",
            "\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\n",
            "\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\nxxxxxxxx\n",
        ];
        let count: usize = kani::any();
        kani::assume(count < LOGS.len());
        let kept = crate::ebpf::attach_error::keep_last_lines(LOGS[count], 3);
        if count == 0 {
            assert!(kept.is_empty(), "the empty log must keep nothing");
        } else {
            // The count law only, asserted machinery-free: the kept
            // length against the bound. The per-line content (lines
            // ride through trimmed-whole, in order) is the byte-exact
            // composition the unit tree pins — a heap-Vec element loop
            // here would re-pay the memory bomb the decomposition
            // shed (symbolic trip count, unrolled to the bound).
            assert_eq!(
                kept.len(),
                count.min(3),
                "the tail kept the wrong line count against the bound 3"
            );
        }
    }

    /// The verifier-log tail's CAP law: a line over the 120-character
    /// bound is cut to 120 characters plus the ellipsis, shorter lines
    /// ride through whole, and adjacent kept lines join with the
    /// separator. Asserted against the join stage
    /// ([`crate::ebpf::attach_error::cap_and_join`]) over CONCRETE
    /// array probes at the exact boundary — the decomposition shape
    /// the deep lane measured (concrete trip counts verify in seconds;
    /// a symbolic trip count over the same body is the memory bomb).
    #[kani::proof]
    #[kani::unwind(128)]
    fn verifier_tail_caps_every_line_at_the_bound() {
        // One static line just over the bound: 121 x's. A literal, not
        // a runtime push loop — the harness adds zero machinery of its
        // own. The cut is byte-exact: 120 characters plus the ellipsis.
        const LONG: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
xxxxxxxxxxxxxxxxxxxxxxx";
        const CAPPED: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
xxxxxxxxxxxxxxxxxxxx...";
        assert_eq!(LONG.chars().count(), 121, "the probe line is over the cap");
        let capped = crate::ebpf::attach_error::cap_and_join(&[LONG]);
        assert_eq!(
            capped, CAPPED,
            "the capped render drifted off the 120-character bound"
        );
        // The pass-through and separator arms over the same concrete
        // machinery: shorter lines ride through whole, joined once.
        let joined = crate::ebpf::attach_error::cap_and_join(&["xxxxxxxx", "second line"]);
        assert_eq!(
            joined, "xxxxxxxx | second line",
            "the pass-through join drifted off the separator law"
        );
    }
}
