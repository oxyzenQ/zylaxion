// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-frontier-Z6: the errno-table harness family (entry 49's
//! control-plane proofs, split from `kani_proofs.rs` under the 600-LOC
//! no-mercy law — a pure move, the same verifier lane owns it).

#[cfg(kani)]
mod errno_proofs {
    use crate::ebpf::attach_error::AttachError;

    /// The classification a marker code reconstructs to, as one byte —
    /// the harness compares KINDS, not details (the detail-carrying
    /// variants return canonical placeholder texts from `from_code`).
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
        }
    }

    /// The marker wire format's law (entry 28): every code in the
    /// pinned 1..=9 band reconstructs to a variant of the SAME kind
    /// that round-trips to the same code, and every code outside the
    /// band reconstructs to nothing — the doctor can never read a
    /// failure the taxonomy does not name.
    #[kani::proof]
    fn attach_error_codes_round_trip_inside_the_pinned_band() {
        let code: u32 = kani::any();
        kani::assume((1..=9).contains(&code));
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
        kani::assume(!(1..=9).contains(&code));
        assert!(
            AttachError::from_code(code).is_none(),
            "code {code} reconstructed outside the pinned band"
        );
    }

    /// The errno classification table (entry 28) over the WHOLE i32
    /// errno domain: the kernel's own classifications are the law —
    /// EPERM/EACCES are privilege, ENOENT is the missing symbol,
    /// EINVAL is the kprobe refusal, and every other errno lands in
    /// the honest Unknown, never a pretend cause.
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

    /// The verifier-log tail's bound law: at most `keep` lines survive,
    /// each capped at 120 characters plus the ellipsis, whatever the
    /// log's shape — the same input, the same output shape, on every
    /// kernel's log style.
    #[kani::proof]
    #[kani::unwind(64)]
    fn verifier_tail_stays_bounded() {
        // The string machinery is the SAT budget's enemy (a 400-unwind
        // encode of byte loops crashed CBMC on this container — meas-
        // ured, not guessed), so the harness carries the bound law in
        // two cheap halves: the CAP law on a CONCRETE over-long log
        // (the 120-character cap and the keep=3 line bound, exercised
        // for real), and the SHAPE law over short symbolic lengths.
        const LONG: &str = "x130xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
xxxxxxxxxxxxxxx";
        let long_log = format!("{LONG}\n{LONG}\n{LONG}\n{LONG}");
        let long_tail = crate::ebpf::attach_error::verifier_tail(long_log.as_str(), 3);
        let long_parts: Vec<&str> = long_tail.split(" | ").collect();
        assert_eq!(
            long_parts.len(),
            3,
            "the concrete cap kept the wrong line count"
        );
        for part in &long_parts {
            assert!(
                part.chars().count() <= 123,
                "a tail line escaped the 120-character cap"
            );
        }

        // The symbolic axis is the LINE COUNT only (three bits): every
        // count from 0 to 6 rides one concrete eight-character line
        // shape plus an empty line for the filter, so the keep-bound
        // and the empty-line filter verify over their whole space
        // without one symbolic byte of string machinery.
        const SHORT: &str = "xxxxxxxx";
        let count: u32 = kani::any();
        kani::assume(count <= 6);
        let mut log = String::with_capacity(72);
        log.push('\n');
        for _ in 0..count {
            log.push_str(SHORT);
            log.push('\n');
        }
        let tail = crate::ebpf::attach_error::verifier_tail(&log, 3);
        let parts: Vec<&str> = tail.split(" | ").collect();
        let want: usize = if count == 0 {
            0
        } else {
            (count as usize).min(3)
        };
        assert_eq!(
            parts.len(),
            want,
            "the tail kept {} lines against the bound 3 for count {count}",
            parts.len()
        );
        for part in &parts {
            assert!(
                part.chars().count() <= 123,
                "a tail line escaped the 120-character cap"
            );
        }
    }
}
