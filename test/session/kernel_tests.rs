// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// kernel_tests.rs — the session/kernel.rs pure-core pins (night-improve-23,
// the eBPF-only floor). Wired via #[path] (the zelynic Pattern C test
// tree). The doctor and the attach path share these verdicts — a drift
// here would surface as a doctor that promises what attach refuses
// (or the opposite). The release shapes are the real-world set
// (generic Ubuntu, Debian, WSL2, bare semver).

use super::*;

#[test]
fn parse_real_release_shapes() {
    assert_eq!(parse_kernel_release("6.8.0-45-generic"), Some((6, 8)));
    assert_eq!(
        parse_kernel_release("5.15.167.4-microsoft-standard-WSL2"),
        Some((5, 15))
    );
    assert_eq!(parse_kernel_release("4.19.0-27-amd64"), Some((4, 19)));
    assert_eq!(parse_kernel_release("6.12.1"), Some((6, 12)));
}

#[test]
fn parse_edges() {
    assert_eq!(parse_kernel_release("5"), Some((5, 0)));
    assert_eq!(parse_kernel_release(""), None);
    assert_eq!(parse_kernel_release("abc"), None);
    assert_eq!(parse_kernel_release("5.x"), None);
    assert_eq!(parse_kernel_release(".15"), None);
}

#[test]
fn floor_law_is_the_owner_minimum() {
    // Exactly at the floor: supported.
    assert!(is_supported(Some(MIN_KERNEL)));
    assert_eq!(MIN_KERNEL, (5, 15));
    assert!(is_supported(Some((6, 0))));
    assert!(is_supported(Some((5, 16))));
    // Below it: unsupported — including the adjacent minor.
    assert!(!is_supported(Some((5, 14))));
    assert!(!is_supported(Some((4, 19))));
    // Unparseable is unsupported (never guess).
    assert!(!is_supported(None));
}

#[test]
fn verdict_strings_are_canonical() {
    let mut report = KernelReport {
        release: String::new(),
        parsed: Some((6, 8)),
        btf: true,
        bpffs: true,
        tracepoint_visible: true,
        lockdown_readable: false,
        lockdown_blocks_bpf: false,
    };
    assert_eq!(report.verdict(), "verified supported");
    report.parsed = Some((5, 14));
    assert_eq!(report.verdict(), "verified unsupported");
    report.parsed = Some(MIN_KERNEL);
    assert_eq!(report.verdict(), "verified supported");
}

#[test]
fn lockdown_modes() {
    // The bracketed mode is the active one; only confidentiality blocks BPF.
    assert!(!lockdown_blocks("[none] integrity confidentiality"));
    assert!(!lockdown_blocks("none [integrity] confidentiality"));
    assert!(lockdown_blocks("none integrity [confidentiality]"));
    // No brackets at all (lockdown LSM compiled out renders nothing):
    // not blocking.
    assert!(!lockdown_blocks(""));
    assert!(!lockdown_blocks("garbage"));
}
