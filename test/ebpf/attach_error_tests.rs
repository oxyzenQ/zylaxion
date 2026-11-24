// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

// attach_error_tests.rs — the classification pins (night-improve-25):
// feed each synthetic failure into the classifier, assert the exact
// variant — the same discipline the night-badas-1 privilege gate
// added for its polarity. The marker code round-trip is pinned too:
// the code is a wire format an old pin file may still carry, so a
// renumbered code (or a from_code that answers the wrong variant)
// fails here before it can mislead a doctor.

use super::*;

#[test]
fn kprobe_registration_classification_is_the_kernel_s_own() {
    // The kernel's errno semantics for a kprobe registration
    // (kernel/trace/trace_kprobe.c + the perf core):
    assert_eq!(
        AttachError::from_kprobe_open("input_event", libc::EPERM),
        AttachError::PermissionDenied
    );
    assert_eq!(
        AttachError::from_kprobe_open("input_event", libc::EACCES),
        AttachError::PermissionDenied
    );
    assert_eq!(
        AttachError::from_kprobe_open("input_event", libc::ENOENT),
        AttachError::SymbolMissing
    );
    assert_eq!(
        AttachError::from_kprobe_open("input_event", libc::EINVAL),
        AttachError::KprobeBlacklisted
    );
    // The alternate symbol classifies through the same law.
    assert_eq!(
        AttachError::from_kprobe_open("input_handle_event", libc::ENOENT),
        AttachError::SymbolMissing
    );
    // An unclassified errno stays honest and STRUCTURED (night-
    // depth-hardening-4): the errno rides the variant, the strerror
    // text renders only when the sentence prints.
    let e = AttachError::from_kprobe_open("input_event", libc::ENOMEM);
    assert_eq!(
        e,
        AttachError::UnknownErrno {
            lane: ErrnoLane::KprobeOpen {
                symbol: "input_event".to_string(),
            },
            errno: libc::ENOMEM,
        }
    );
    // The lazy render is byte-identical to the eager form it replaced
    // (the doctor's sentence loses nothing).
    assert_eq!(
        e.to_string(),
        format!(
            "the operation failed for an unclassified reason: \
             perf_event_open on kprobe:input_event failed with errno {} ({}).",
            libc::ENOMEM,
            "out of memory"
        ),
    );
}

#[test]
fn fentry_link_classification() {
    assert_eq!(
        AttachError::from_fentry_link(libc::EPERM),
        AttachError::PermissionDenied
    );
    assert_eq!(
        AttachError::from_fentry_link(libc::EACCES),
        AttachError::PermissionDenied
    );
    assert_eq!(
        AttachError::from_fentry_link(libc::ENOENT),
        AttachError::SymbolMissing
    );
    assert_eq!(
        AttachError::from_fentry_link(libc::EINVAL),
        AttachError::FentryUnavailable {
            reason: "the kernel refused the BTF tracing link".to_string()
        }
    );
    let e = AttachError::from_fentry_link(libc::EBUSY);
    assert!(e.to_string().contains("fentry"), "{e}");
    let AttachError::UnknownErrno { lane, errno } = e else {
        panic!("unclassified errno must map to the structured UnknownErrno");
    };
    assert_eq!(errno, libc::EBUSY);
    assert_eq!(lane, ErrnoLane::FentryLink);
}

#[test]
fn link_create_classification() {
    assert_eq!(
        AttachError::from_link_create(libc::EPERM),
        AttachError::PermissionDenied
    );
    let AttachError::Unknown { detail } = AttachError::from_link_create(libc::EINVAL) else {
        panic!("EINVAL on link create must stay Unknown (honestly unclassified)");
    };
    assert!(detail.contains("BPF_LINK_CREATE"), "{detail}");
    assert!(detail.contains("bpf_perf_link"), "{detail}");
    // The fallthrough (an errno outside the two classified arms) is
    // the structured honest lane, pinned like the other two lanes.
    let e = AttachError::from_link_create(libc::EBUSY);
    assert!(e.to_string().contains("BPF_LINK_CREATE"), "{e}");
    let AttachError::UnknownErrno { lane, errno } = e else {
        panic!("the link-create fallthrough must map to the structured UnknownErrno");
    };
    assert_eq!(errno, libc::EBUSY);
    assert_eq!(lane, ErrnoLane::LinkCreate);
}

#[test]
fn marker_codes_round_trip_the_wire_format() {
    // The code is a wire format: from_code(code()).code() must be a
    // fixed point for every variant, and the bare variants
    // reconstruct exactly. 0 is reserved (no failure recorded) and
    // must not reconstruct.
    let samples = [
        AttachError::FentryUnavailable {
            reason: "no vmlinux BTF".to_string(),
        },
        AttachError::KprobeBlacklisted,
        AttachError::SymbolMissing,
        AttachError::VerifierRejected {
            detail: "log".to_string(),
        },
        AttachError::PermissionDenied,
        AttachError::BpffsUnavailable,
        AttachError::PinFailed {
            path: "/sys/fs/bpf/zylaxion".to_string(),
        },
        AttachError::LockdownActive,
        AttachError::Unknown {
            detail: "story".to_string(),
        },
    ];
    for (i, e) in samples.iter().enumerate() {
        // The original nine keep codes 1..=9, distinct, in the
        // declared variant order (ObjectUnsealed, declared tenth,
        // takes code 10 — the appended slot, pinned separately
        // below, so 9 stays Unknown for old marker files).
        assert_eq!(e.code(), i as u32 + 1);
        let reconstructed = AttachError::from_code(e.code())
            .unwrap_or_else(|| panic!("code {} must reconstruct", e.code()));
        assert_eq!(reconstructed.code(), e.code());
    }
    for e in [
        AttachError::KprobeBlacklisted,
        AttachError::SymbolMissing,
        AttachError::PermissionDenied,
        AttachError::BpffsUnavailable,
        AttachError::LockdownActive,
    ] {
        assert_eq!(AttachError::from_code(e.code()), Some(e));
    }
    // night-netrunner-Z1: the anchor-gate refusal takes the NEXT free
    // code (10), not the positional slot its declaration order would
    // suggest — 9 stays Unknown so an old marker file keeps reading
    // as the failure it recorded (the wire-format law above).
    let unsealed = AttachError::ObjectUnsealed {
        detail: "the tap was patched after the build".to_string(),
    };
    assert_eq!(unsealed.code(), 10);
    assert_eq!(AttachError::from_code(10).map(|e| e.code()), Some(10));
    // night-depth-hardening-4: the structured errno fallthrough shares
    // Unknown's wire code — the marker band stays the pinned 1..=10
    // space, and an old marker file keeps reading as the same honest
    // unclassified failure.
    let unknown_errno = AttachError::UnknownErrno {
        lane: ErrnoLane::LinkCreate,
        errno: libc::EBUSY,
    };
    assert_eq!(unknown_errno.code(), 9);
    assert_eq!(AttachError::from_code(9).map(|e| e.code()), Some(9));
    assert_eq!(AttachError::from_code(0), None);
    assert_eq!(AttachError::from_code(11), None);
    assert_eq!(AttachError::from_code(u32::MAX), None);
}

#[test]
fn every_variant_renders_a_complete_sentence() {
    // The doctor prints Display verbatim — no variant may render as
    // a bare enum-ish fragment. Each sentence names its cause and
    // (where a fix exists) the fix.
    let samples = [
        AttachError::FentryUnavailable {
            reason: "no vmlinux BTF (CONFIG_DEBUG_INFO_BTF=n)".to_string(),
        },
        AttachError::KprobeBlacklisted,
        AttachError::SymbolMissing,
        AttachError::VerifierRejected {
            detail: "R0 invalid mem access".to_string(),
        },
        AttachError::PermissionDenied,
        AttachError::BpffsUnavailable,
        AttachError::PinFailed {
            path: "/sys/fs/bpf/zylaxion/tap_link".to_string(),
        },
        AttachError::LockdownActive,
        AttachError::Unknown {
            detail: "a mystery the chain could not classify from the kernel's answer".to_string(),
        },
        AttachError::UnknownErrno {
            lane: ErrnoLane::KprobeOpen {
                symbol: "input_event".to_string(),
            },
            errno: libc::ENOMEM,
        },
    ];
    for e in &samples {
        let text = e.to_string();
        assert!(text.ends_with('.') || text.ends_with(')'), "{e:?}: {text}");
        let first = text.chars().next().unwrap();
        assert!(first.is_lowercase(), "{e:?} renders mid-sentence: {text}");
        assert!(text.len() > 60, "{e:?} renders too thin: {text}");
    }
    // The detail-carrying variants name their detail in the render.
    assert!(samples[0].to_string().contains("CONFIG_DEBUG_INFO_BTF"));
    assert!(samples[3].to_string().contains("R0 invalid mem access"));
    assert!(
        samples[6]
            .to_string()
            .contains("/sys/fs/bpf/zylaxion/tap_link")
    );
    assert!(
        samples[8]
            .to_string()
            .contains("could not classify from the kernel's answer")
    );
}

#[test]
fn reconstructed_variants_render_the_canonical_pointer() {
    // from_code cannot know the per-run detail; its render points the
    // operator at the live command instead of inventing one.
    let e = AttachError::from_code(4).expect("code 4 is VerifierRejected");
    assert!(e.to_string().contains("see the attach run's output"), "{e}");
}

/// night-total-lts-2: the Unknown verb is command-neutral — the detach
/// lane's classified races render through this variant too, and the old
/// "the attach failed" wording printed under a `zylaxion detach` run was
/// its own dirty bug (the user ran detach, the sentence said attach).
#[test]
fn unknown_renders_the_neutral_operation_verb() {
    let e = AttachError::Unknown {
        detail: "tap detach incomplete: [] removed".to_string(),
    };
    let rendered = e.to_string();
    assert!(
        rendered.starts_with("the operation failed for an unclassified reason:"),
        "the verb must be command-neutral, got: {rendered}"
    );
    assert!(
        !rendered.contains("the attach failed"),
        "the attach verb must stay out of the shared variant, got: {rendered}"
    );
    assert!(
        rendered.ends_with('.'),
        "the render owns the sentence: {rendered}"
    );
}

/// night-depth-hardening-1: the verifier tail keeps the last `keep`
/// non-empty lines — the rejection verdict — and drops the kernel-
/// version-specific state-dump head, so the same rejection renders the
/// same bounded shape on 5.15 and 6.x alike.
#[test]
fn verifier_tail_keeps_the_last_non_empty_lines_only() {
    let log = "\nstate 1: R0=inv1\n\nstate 42: R0_w=1\nbackedge from insn 7 to 1\n";
    assert_eq!(
        verifier_tail(log, 2),
        "state 42: R0_w=1 | backedge from insn 7 to 1"
    );
}

#[test]
fn verifier_tail_caps_line_length_and_handles_short_logs() {
    // A 200-character line is capped at 120 characters plus the
    // ellipsis marker; a log shorter than the keep bound is returned
    // whole; an empty log renders as the empty string.
    let long = format!("x{}", "y".repeat(199));
    let tail = verifier_tail(&long, 3);
    assert_eq!(tail.chars().count(), 123, "120 cap + the ... marker");
    assert!(tail.ends_with("..."));
    assert_eq!(verifier_tail("only one line", 3), "only one line");
    assert_eq!(verifier_tail("", 3), "");
}

/// night-depth-hardening-4's manual line-scan equivalence pins: the
/// `\n`-cut plus `trim()` body must yield the SAME kept lines
/// `str::lines()` yields on the shapes where the two could drift —
/// CRLF endings (the `\r` rides inside the cut segment and the trim
/// eats it), a trailing segment with no final newline, a log that
/// ends with the newline, and whitespace-only padding lines.
#[test]
fn verifier_tail_cuts_crlf_and_trailing_segments_like_lines() {
    // CRLF endings: the same two kept lines as the LF form.
    assert_eq!(
        verifier_tail("head\r\nstate 1: R0=inv1\r\nstate 42: R0_w=1\r\n", 2),
        "state 1: R0=inv1 | state 42: R0_w=1"
    );
    // A trailing segment without a final newline is still a line.
    assert_eq!(
        verifier_tail("state 1: R0=inv1\nstate 42: R0_w=1", 2),
        "state 1: R0=inv1 | state 42: R0_w=1"
    );
    // A log that ends with the newline adds no phantom trailing line.
    assert_eq!(verifier_tail("one line\n", 3), "one line");
    // Whitespace-only lines are the empty-line filter's, not the
    // keep bound's.
    assert_eq!(verifier_tail(" \t \none line\n \t \n", 3), "one line");
}

// ── night-hardening-2: the full-domain sweeps ─────────────────────────

/// The kernel's errno space, swept end to end across all three raw
/// syscall lanes (plus 0 and the negatives a mis-wired caller could
/// pass). The law: EPERM and EACCES are ALWAYS the privilege verdict;
/// ENOENT is ALWAYS the missing-symbol verdict on the kprobe and
/// fentry lanes and honest-unclassified on the link-create lane;
/// EINVAL is the kernel's own per-lane refusal; EVERY other value
/// lands in the structured fallthrough (`UnknownErrno`, night-
/// depth-hardening-4) carrying the raw errno — never folded into a
/// classified cause — and the render still names the errno number.
/// The example pins above name the wordings; this sweep pins the
/// POLARITY over the whole input domain.
#[test]
fn every_errno_classifies_by_the_shared_law() {
    for errno in -2i32..=133 {
        let kprobe = AttachError::from_kprobe_open("input_event", errno);
        let fentry = AttachError::from_fentry_link(errno);
        let link = AttachError::from_link_create(errno);
        match errno {
            libc::EPERM | libc::EACCES => {
                assert_eq!(kprobe, AttachError::PermissionDenied);
                assert_eq!(fentry, AttachError::PermissionDenied);
                assert_eq!(link, AttachError::PermissionDenied);
            }
            libc::ENOENT => {
                assert_eq!(kprobe, AttachError::SymbolMissing);
                assert_eq!(fentry, AttachError::SymbolMissing);
                assert!(
                    matches!(link, AttachError::UnknownErrno { errno: e, .. } if e == errno),
                    "link create errno {errno} must stay honestly unclassified"
                );
            }
            libc::EINVAL => {
                assert_eq!(kprobe, AttachError::KprobeBlacklisted);
                assert!(
                    matches!(fentry, AttachError::FentryUnavailable { .. }),
                    "fentry errno {errno} must be the BTF-tracing refusal"
                );
                assert!(
                    matches!(link, AttachError::Unknown { .. }),
                    "link create errno {errno} must stay Unknown"
                );
            }
            other => {
                for (lane, e) in [("kprobe", &kprobe), ("fentry", &fentry), ("link", &link)] {
                    let AttachError::UnknownErrno { errno: carried, .. } = e else {
                        panic!(
                            "{lane}: errno {other} must stay in the structured fallthrough, \
                             got {e:?}"
                        );
                    };
                    assert_eq!(
                        *carried, other,
                        "{lane}: the fallthrough must carry the raw errno"
                    );
                    let rendered = e.to_string();
                    assert!(
                        rendered.contains(&other.to_string()),
                        "{lane}: errno {other} must appear in the render: {rendered}"
                    );
                }
            }
        }
    }
}

/// The marker wire format over its domain edges: codes 1..=10
/// reconstruct (pinned per-variant above — 10 joined the space with
/// night-netrunner-Z1's ObjectUnsealed, appended so 9 stays Unknown
/// for every old marker file); EVERY other value — 0, the
/// 11..=1000 band, the powers-of-two neighborhood, and both u32
/// extremes — answers None. A renumbered or widened code space would
/// make an old marker read as a different failure; this sweep holds
/// the door.
#[test]
fn the_marker_code_domain_answers_none_outside_one_to_ten() {
    assert_eq!(AttachError::from_code(0), None);
    for code in 11u32..=1000 {
        assert!(
            AttachError::from_code(code).is_none(),
            "code {code} must not reconstruct"
        );
    }
    for code in [
        1u32 << 16,
        1u32 << 31,
        (1 << 31) + 1,
        u32::MAX - 1,
        u32::MAX,
    ] {
        assert!(
            AttachError::from_code(code).is_none(),
            "code {code} must not reconstruct"
        );
    }
}
