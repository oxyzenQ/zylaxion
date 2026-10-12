// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// kernel_tests.rs — the session/kernel.rs pure-core pins (night-improve-23,
// the eBPF-only floor; night-improve-25 adds the input-tap verdict
// matrix). Wired via #[path] (the zelynic Pattern C test
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
    // Unparsable is unsupported (never guess).
    assert!(!is_supported(None));
}

#[test]
fn verdict_strings_are_canonical() {
    let mut report = KernelReport {
        release: String::new(),
        parsed: Some((6, 8)),
        btf: true,
        bpffs: true,
        input_tap: input_tap_verdict(
            BtfStatus::Available,
            BlacklistStatus::Unreadable,
            BlacklistStatus::Unreadable,
        ),
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

// ── The input-tap verdict matrix (night-improve-25) ──────────────────────

#[test]
fn verdict_1_btf_available_names_the_fentry_step() {
    // BTF carries input_event: the chain's step 1 preflights clean —
    // the exact canonical string, verbatim (the doctor renders it as
    // returned; a drift here would change the doctor's output).
    assert_eq!(
        input_tap_verdict(
            BtfStatus::Available,
            BlacklistStatus::Unreadable,
            BlacklistStatus::Unreadable
        ),
        "input tap: fentry:input_event (BTF-available)"
    );
    // The blacklist state is irrelevant once fentry is available (the
    // chain never walks past step 1) — even Listed answers the same.
    assert_eq!(
        input_tap_verdict(
            BtfStatus::Available,
            BlacklistStatus::Listed,
            BlacklistStatus::Listed
        ),
        "input tap: fentry:input_event (BTF-available)"
    );
}

#[test]
fn verdict_2_clean_kprobe_falls_back_with_the_fentry_reason() {
    // No usable BTF, input_event not blacklisted: step 2, with every
    // fentry reason phrased in the verdict.
    for (btf, reason) in [
        (
            BtfStatus::Absent,
            "no vmlinux BTF (CONFIG_DEBUG_INFO_BTF=n)",
        ),
        (BtfStatus::Unreadable, "vmlinux BTF unreadable"),
        (
            BtfStatus::MissingSymbol,
            "input_event not present in vmlinux BTF",
        ),
    ] {
        assert_eq!(
            input_tap_verdict(btf, BlacklistStatus::NotListed, BlacklistStatus::Unreadable),
            format!("input tap: kprobe:input_event (fentry unavailable: {reason})")
        );
    }
}

#[test]
fn verdict_3_the_alternate_names_why_input_event_missed() {
    // input_event blacklisted (or unverifiable), input_handle_event
    // clean: step 3 — the parenthetical names the PRIMARY step's
    // blocker, per the canonical shape.
    assert_eq!(
        input_tap_verdict(
            BtfStatus::Absent,
            BlacklistStatus::Listed,
            BlacklistStatus::NotListed
        ),
        "input tap: kprobe:input_handle_event (input_event unavailable: \
         input_event is on the kprobe blacklist)"
    );
    assert_eq!(
        input_tap_verdict(
            BtfStatus::Absent,
            BlacklistStatus::Unreadable,
            BlacklistStatus::NotListed
        ),
        "input tap: kprobe:input_handle_event (input_event unavailable: \
         kprobe blacklist unreadable)"
    );
}

#[test]
fn verdict_4_unavailable_carries_the_compound_reason() {
    // Everything exhausted or unverifiable: the compound reason names
    // every step's standing — honest about what is blocked versus
    // what this session simply could not read.
    let v = input_tap_verdict(
        BtfStatus::Absent,
        BlacklistStatus::Listed,
        BlacklistStatus::Listed,
    );
    assert!(v.starts_with("input tap: UNAVAILABLE ("), "{v}");
    assert!(v.contains("fentry unavailable: no vmlinux BTF"), "{v}");
    assert!(v.contains("input_event is kprobe-blacklisted"), "{v}");
    assert!(
        v.contains("input_handle_event is kprobe-blacklisted"),
        "{v}"
    );

    // Unreadable is NOT available: an unverifiable blacklist reads as
    // UNAVAILABLE with the honest caveat, never as a kprobe verdict.
    let v = input_tap_verdict(
        BtfStatus::Absent,
        BlacklistStatus::Unreadable,
        BlacklistStatus::Unreadable,
    );
    assert!(v.starts_with("input tap: UNAVAILABLE ("), "{v}");
    assert!(
        v.contains("input_event unverified (kprobe blacklist unreadable)"),
        "{v}"
    );
    assert!(
        v.contains("input_handle_event unverified (kprobe blacklist unreadable)"),
        "{v}"
    );
}

// ── The BTF token search ─────────────────────────────────────────────────

#[test]
fn btf_search_is_a_full_token_match() {
    // A slice shaped like the BTF string section: leading NUL,
    // NUL-separated names.
    let btf = b"\0input_dev\0input_event\0input_handle_event\0\0";
    assert!(btf_contains_symbol(btf, "input_event"));
    assert!(btf_contains_symbol(btf, "input_handle_event"));
    assert!(btf_contains_symbol(btf, "input_dev"));
    // Substrings and superstrings are NOT matches (the NUL fence).
    assert!(!btf_contains_symbol(btf, "input"));
    assert!(!btf_contains_symbol(btf, "event"));
    // A name that merely ENDS with the token does not match...
    let prefixed = b"\0xinput_event\0input_event2\0";
    assert!(!btf_contains_symbol(prefixed, "input_event"));
    // ...and a lone exact token does.
    assert!(btf_contains_symbol(b"\0input_event\0", "input_event"));
    // Empty BTF carries nothing.
    assert!(!btf_contains_symbol(b"", "input_event"));
    assert!(!btf_contains_symbol(b"\0\0", "input_event"));
}

// ── The blacklist parser ─────────────────────────────────────────────────

#[test]
fn blacklist_parser_reads_the_documented_line_shape() {
    // <address> <symbol>[\t<module>] — the shape
    // /sys/kernel/tracing/kprobes/blacklist documents.
    let content = "\
0000000000000000 __traceiter_sched_switch
0000000000000001 input_event\tdrivers/input/input
0000000000000002 __do_softirq
";
    assert_eq!(
        parse_blacklist(content, "input_event"),
        BlacklistStatus::Listed
    );
    assert_eq!(
        parse_blacklist(content, "input_handle_event"),
        BlacklistStatus::NotListed
    );
    assert_eq!(
        parse_blacklist(content, "__do_softirq"),
        BlacklistStatus::Listed
    );
    assert_eq!(
        parse_blacklist("", "input_event"),
        BlacklistStatus::NotListed
    );
    // A symbol that appears only as another line's MODULE field does
    // not count (field 2 is the symbol, field 3+ is module).
    assert_eq!(
        parse_blacklist(content, "drivers/input/input"),
        BlacklistStatus::NotListed
    );
}
