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
    /// carry their own walls, and the composition is pinned byte-exact
    /// by the unit tree.
    ///
    /// The quantifier shape: the count axis rides a symbolic selector
    /// over CONCRETE static logs — every distinct behavior of the
    /// keep bound is a branch (the empty log, the below-bound ramp,
    /// the exact-bound case, and the clamp plateau), only the
    /// selection is symbolic. The cost history, measured: the FIRST
    /// bomb was the stage's own `str::lines()` machinery — a fully
    /// concrete seven-byte probe ran CBMC out of memory at the 90 s
    /// mark on a four-gigabyte host — and that fix lives in the body
    /// (the manual `\n` scan, its note carries the record). The
    /// selector then needed its own shape: an indexed array read
    /// (`LOGS[count]`) still OOMed at 91 s over five one-char logs
    /// — the symbolic index reads an array of (pointer, length)
    /// pairs, and every byte the stage walks resolves through a
    /// symbolic pointer target — so the match below carries the SAME
    /// selector with the calls INSIDE the arms: a concrete reference
    /// per arm, no symbolic pointer arithmetic in the instance at
    /// all. Measured wall after both fixes: 12 s whole. The five
    /// probes are the law's whole outcome space (`count.min(3)`:
    /// 0, 1, 2, 3, and the plateau at 3; one char per line, because
    /// the line content is irrelevant to the count law).
    #[kani::proof]
    #[kani::unwind(64)]
    fn verifier_tail_keeps_at_most_the_keep_bound() {
        let count: usize = kani::any();
        kani::assume(count < 5);
        let kept = match count {
            0 => crate::ebpf::attach_error::keep_last_lines("\n", 3),
            1 => crate::ebpf::attach_error::keep_last_lines("\nx\n", 3),
            2 => crate::ebpf::attach_error::keep_last_lines("\nx\nx\n", 3),
            3 => crate::ebpf::attach_error::keep_last_lines("\nx\nx\nx\n", 3),
            _ => crate::ebpf::attach_error::keep_last_lines("\nx\nx\nx\nx\n", 3),
        };
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

    /// The verifier-log tail's CAP law: a line over the cut bound is
    /// cut to `cap` characters plus the ellipsis, a line at or under
    /// the bound rides through whole, and adjacent kept lines join
    /// with the separator. Asserted against the join stage's
    /// parameterized mechanism
    /// ([`crate::ebpf::attach_error::cap_and_join_at`]) — production
    /// binds the cap at 120, a binding the unit tree pins with its
    /// 200-character probe (123 in, 123 out), because the 121-
    /// character probe itself is a measured memory bomb: the
    /// verifier's pointer model walks every probe byte through the
    /// heap objects however concrete the input, and CBMC ground past
    /// the six-minute mark and out of memory on it twice. The
    /// mechanism is cap-generic, so the harness quantifies it at
    /// cap=3 over probes that carry the SAME branch structure —
    /// under-bound, exact-bound, over-bound — at a byte count the
    /// instance fits comfortably (the tree's own laws-not-details
    /// idiom: the classifier table proves kinds, not sentences).
    /// The selector's calls ride INSIDE the match arms: a symbolic
    /// slice selected before the call would phi-merge the probe
    /// pointers into the machinery (the indexed-selector shape the
    /// KEEP harness above measured as a bomb); per-arm concrete
    /// calls keep the pointer model out of the instance entirely.
    #[kani::proof]
    #[kani::unwind(32)]
    fn verifier_tail_caps_every_line_at_the_bound() {
        let which: usize = kani::any();
        kani::assume(which < 3);
        let capped = match which {
            0 => crate::ebpf::attach_error::cap_and_join_at(&["xx"], 3),
            1 => crate::ebpf::attach_error::cap_and_join_at(&["xxx"], 3),
            _ => crate::ebpf::attach_error::cap_and_join_at(&["xxxx"], 3),
        };
        let want = match which {
            0 => "xx",
            1 => "xxx",
            _ => "xxx...",
        };
        assert_eq!(
            capped, want,
            "the capped render drifted off the cut law at the boundary"
        );
        // The separator arm over the same concrete machinery: two
        // over-bound lines each cut at the cap, joined once — the cut
        // and the separator in one probe.
        let joined = crate::ebpf::attach_error::cap_and_join_at(&["xxxxxxxx", "second line"], 3);
        assert_eq!(
            joined, "xxx... | sec...",
            "the joined render drifted off the separator law"
        );
    }
}
