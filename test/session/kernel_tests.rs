// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

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
        unpriv_bpf_disabled: 0,
        unpriv_bpf_boot_locked: false,
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

// ── The unprivileged-bpf posture (night-badas-4 round 5) ────────────────

#[test]
fn unpriv_bpf_parser_reads_the_sysctl_shapes() {
    // The sysctl renders one integer and a newline.
    assert_eq!(parse_unpriv_bpf_disabled("0\n"), 0);
    assert_eq!(parse_unpriv_bpf_disabled("1\n"), 1);
    assert_eq!(parse_unpriv_bpf_disabled("2\n"), 2);
    // No trailing newline and surrounding space both parse.
    assert_eq!(parse_unpriv_bpf_disabled("2"), 2);
    assert_eq!(parse_unpriv_bpf_disabled("  0  "), 0);
    // Empty or malformed content reads as allowed — a probe must
    // never invent a lock it cannot witness.
    assert_eq!(parse_unpriv_bpf_disabled(""), 0);
    assert_eq!(parse_unpriv_bpf_disabled("\n"), 0);
    assert_eq!(parse_unpriv_bpf_disabled("garbage"), 0);
}

#[test]
fn unpriv_bpf_allowed_law() {
    let report = |raw: u8, boot_locked: bool| KernelReport {
        release: String::new(),
        parsed: Some((6, 8)),
        btf: false,
        bpffs: true,
        input_tap: String::new(),
        lockdown_readable: false,
        lockdown_blocks_bpf: false,
        unpriv_bpf_disabled: raw,
        unpriv_bpf_boot_locked: boot_locked,
    };
    assert!(report(0, false).unpriv_bpf_allowed());
    assert!(!report(1, false).unpriv_bpf_allowed());
    assert!(!report(2, false).unpriv_bpf_allowed());
    assert!(!report(2, true).unpriv_bpf_allowed());
}

#[test]
fn narrowed_unpriv_gate_law() {
    // Measured from the mainline source (night-ebpf-era-1): 5.15
    // through 5.18 gate every plain-user bpf() call at the dispatch
    // level, 5.19 narrows the gate to the creation commands, and the
    // pinned-fd element ops ride the file-permission model from
    // there on.
    assert!(!narrowed_unpriv_gate(Some((5, 14))));
    assert!(!narrowed_unpriv_gate(Some((5, 15))));
    assert!(!narrowed_unpriv_gate(Some((5, 18))));
    assert!(narrowed_unpriv_gate(Some((5, 19))));
    assert!(narrowed_unpriv_gate(Some((6, 0))));
    assert!(narrowed_unpriv_gate(Some((6, 12))));
    assert!(narrowed_unpriv_gate(Some((7, 1))));
    // An unparsable release reads conservative — never guess an era
    // the kernel did not state.
    assert!(!narrowed_unpriv_gate(None));
}

#[test]
fn unpriv_map_ops_refusal_law() {
    let report = |raw: u8, parsed: (u32, u32), boot_locked: bool| KernelReport {
        release: String::new(),
        parsed: Some(parsed),
        btf: false,
        bpffs: true,
        input_tap: String::new(),
        lockdown_readable: false,
        lockdown_blocks_bpf: false,
        unpriv_bpf_disabled: raw,
        unpriv_bpf_boot_locked: boot_locked,
    };
    // The open posture: no refusal, whatever the era and the euid.
    assert!(
        report(0, (5, 15), false)
            .unpriv_map_ops_refusal(false)
            .is_none()
    );
    assert!(
        report(0, (6, 12), false)
            .unpriv_map_ops_refusal(false)
            .is_none()
    );
    // Root is never gated — the kernel's gate keys on capability,
    // not on the sysctl (the rescue path stays reachable).
    assert!(
        report(2, (5, 15), true)
            .unpriv_map_ops_refusal(true)
            .is_none()
    );
    assert!(
        report(2, (6, 12), true)
            .unpriv_map_ops_refusal(true)
            .is_none()
    );
    // The dispatch era (5.15-5.18): a gated plain user gets the
    // classified refusal, carrying the value and the substring the
    // E2E engine pins (sm_audio.py).
    let locked = report(2, (5, 15), true)
        .unpriv_map_ops_refusal(false)
        .unwrap();
    assert!(locked.contains("unprivileged bpf"), "{locked}");
    assert!(locked.contains("(2)"), "{locked}");
    assert!(locked.contains("doctor"), "{locked}");
    assert!(locked.contains("sysctl"), "{locked}");
    let disabled = report(1, (5, 15), false)
        .unpriv_map_ops_refusal(false)
        .unwrap();
    assert!(disabled.contains("unprivileged bpf"), "{disabled}");
    assert!(disabled.contains("(1)"), "{disabled}");
    // The narrowed era (5.19+): the verbs create nothing, so the
    // pinned-fd ops ride the file permissions — no refusal at any
    // locked posture, and the product must not self-refuse what the
    // kernel allows.
    assert!(
        report(2, (5, 19), false)
            .unpriv_map_ops_refusal(false)
            .is_none()
    );
    assert!(
        report(1, (6, 12), false)
            .unpriv_map_ops_refusal(false)
            .is_none()
    );
}

#[test]
fn unpriv_bpf_verdict_law() {
    let report = |raw: u8, parsed: (u32, u32), boot_locked: bool| KernelReport {
        release: String::new(),
        parsed: Some(parsed),
        btf: false,
        bpffs: true,
        input_tap: String::new(),
        lockdown_readable: false,
        lockdown_blocks_bpf: false,
        unpriv_bpf_disabled: raw,
        unpriv_bpf_boot_locked: boot_locked,
    };
    // Every arm carries the substring the E2E engine pins.
    for verdict in [
        report(0, (5, 15), false).unpriv_bpf_verdict(),
        report(1, (5, 15), false).unpriv_bpf_verdict(),
        report(2, (6, 12), false).unpriv_bpf_verdict(),
        report(9, (6, 12), false).unpriv_bpf_verdict(),
    ] {
        assert!(verdict.contains("unprivileged bpf"), "{verdict}");
    }
    // The transition law is the mainline handler's own, uniform
    // across the eras (measured at 5.15, 5.18 and master): 1 is the
    // permanent lock, a 2 is root-writable to 0. No arm may claim a
    // re-enable for 1 and no arm may claim an unflippable 2.
    let old_locked = report(1, (5, 15), false).unpriv_bpf_verdict();
    assert!(old_locked.contains("disabled (1)"), "{old_locked}");
    assert!(old_locked.contains("permanent lock"), "{old_locked}");
    assert!(!old_locked.contains("re-enable"), "{old_locked}");
    assert!(old_locked.contains("verbs cannot run"), "{old_locked}");
    let narrowed_locked = report(1, (6, 12), false).unpriv_bpf_verdict();
    assert!(
        narrowed_locked.contains("permanent lock"),
        "{narrowed_locked}"
    );
    assert!(
        narrowed_locked.contains("verbs still run"),
        "{narrowed_locked}"
    );
    // night-dinner-3, re-measured (night-ebpf-era-1): the boot-param
    // premise is refuted by the uniform handler law — the cmdline
    // fact is REPORTED, the lock claim is gone, and the write of 0
    // is named as the test, not trusted as a model.
    let defaulted = report(2, (6, 12), false).unpriv_bpf_verdict();
    assert!(defaulted.contains("disabled (2)"), "{defaulted}");
    assert!(!defaulted.contains("cannot be re-enabled"), "{defaulted}");
    assert!(defaulted.contains("sysctl"), "{defaulted}");
    assert!(defaulted.contains("verbs still run"), "{defaulted}");
    let boot_locked = report(2, (6, 12), true).unpriv_bpf_verdict();
    assert!(
        boot_locked.contains("bpf.unprivileged_bpf_disabled=2"),
        "{boot_locked}"
    );
    assert!(boot_locked.contains("locks only at 1"), "{boot_locked}");
    assert!(
        !boot_locked.contains("cannot be re-enabled"),
        "{boot_locked}"
    );
    // The dispatch-era consequence: the verbs cannot run.
    let old_defaulted = report(2, (5, 15), false).unpriv_bpf_verdict();
    assert!(
        old_defaulted.contains("verbs cannot run"),
        "{old_defaulted}"
    );
    assert!(
        report(9, (6, 12), false)
            .unpriv_bpf_verdict()
            .contains("disabled (9)")
    );
}

#[test]
fn unpriv_boot_param_lock_law() {
    // The exact token is the lock.
    assert!(unpriv_bpf_locked_by_boot_param(
        "quiet splash bpf.unprivileged_bpf_disabled=2"
    ));
    assert!(unpriv_bpf_locked_by_boot_param(
        "bpf.unprivileged_bpf_disabled=2"
    ));
    // A value of 1, a prefix, a suffix, or nothing at all is not.
    assert!(!unpriv_bpf_locked_by_boot_param(
        "bpf.unprivileged_bpf_disabled=1"
    ));
    assert!(!unpriv_bpf_locked_by_boot_param(
        "bpf.unprivileged_bpf_disabled=20"
    ));
    assert!(!unpriv_bpf_locked_by_boot_param(
        "xbpf.unprivileged_bpf_disabled=2"
    ));
    assert!(!unpriv_bpf_locked_by_boot_param("quiet splash"));
    assert!(!unpriv_bpf_locked_by_boot_param(""));
}

#[test]
fn mode_grants_x_law() {
    // 0o701: owner rwx, group none, other x — the Debian 13 mount
    // after the front-door fix.
    assert!(mode_grants_x(0o701, 0, 0, 1000, 1000));
    // 0o700: the mount itself — other gets nothing.
    assert!(!mode_grants_x(0o700, 0, 0, 1000, 1000));
    // The owner triad decides for the owner, with NO fall-through:
    // an owner denied by their own triad does not borrow the other
    // triad's bit.
    assert!(!mode_grants_x(0o007, 1000, 1000, 1000, 1000));
    assert!(mode_grants_x(0o100, 1000, 1000, 1000, 1000));
    // The group triad decides for the group (same no-fall-through
    // law), other for everyone else.
    assert!(mode_grants_x(0o010, 0, 1000, 1000, 1000));
    assert!(!mode_grants_x(0o001, 0, 1000, 1000, 1000));
    assert!(mode_grants_x(0o001, 0, 0, 1000, 1000));
    // The setuid/sticky high bits never leak into the decision:
    // sticky 0o1776 — other reads/writes but does not traverse.
    assert!(!mode_grants_x(0o1776, 0, 0, 1000, 1000));
}
