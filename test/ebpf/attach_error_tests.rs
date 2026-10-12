// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
    // An unclassified errno stays honest: Unknown, carrying the
    // symbol and the errno in its detail.
    let e = AttachError::from_kprobe_open("input_event", libc::ENOMEM);
    assert_eq!(
        e,
        AttachError::Unknown {
            detail: format!(
                "perf_event_open on kprobe:input_event failed with errno {} ({})",
                libc::ENOMEM,
                "out of memory"
            ),
        }
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
    let AttachError::Unknown { detail } = e else {
        panic!("unclassified errno must map to Unknown");
    };
    assert!(detail.contains("fentry"), "{detail}");
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
        // Codes are 1..=9, distinct, in the declared variant order.
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
    assert_eq!(AttachError::from_code(0), None);
    assert_eq!(AttachError::from_code(10), None);
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
