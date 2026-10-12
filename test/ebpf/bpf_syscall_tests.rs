// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// bpf_syscall_tests.rs — the pure constants + parsing contract of the
// raw-syscall lane (night-improve-23). The syscalls themselves are
// root-run integration surfaces; here the values that must match the
// kernel ABI byte-for-byte get pinned, plus the tracepoint id path
// order and content parser — all pure, no host filesystem access (the
// miri contract: the interpreter refuses host file opens, and a test
// needing one would be a test bug).

use super::*;

#[test]
fn syscall_constants_match_the_kernel_abi() {
    // From linux/bpf.h — these ARE the ABI; a wrong constant here pins
    // the wrong object or creates the wrong link type.
    assert_eq!(BPF_LINK_CREATE, 28);
    assert_eq!(BPF_OBJ_PIN, 6);
    assert_eq!(BPF_PERF_EVENT, 41);
    assert_eq!(PERF_TYPE_TRACEPOINT, 2);
    assert_eq!(PERF_FLAG_FD_CLOEXEC, 8);
}

#[test]
fn attr_struct_sizes_are_kernel_shaped() {
    // LinkCreateAttr: 16 field bytes + 40 pad = 56 (the kernel reads
    // this many bytes of the union head).
    assert_eq!(std::mem::size_of::<LinkCreateAttr>(), 56);
    // ObjPinAttr: pathname + bpf_fd + file_flags.
    assert_eq!(std::mem::size_of::<ObjPinAttr>(), 16);
    // PerfEventAttr: the canonical 64-byte perf_event_attr... minus the
    // tail extensions this lane does not carry: the kernel accepts any
    // size >= the version's base (40 bytes through the wakeup branch);
    // the struct here spans name/raw fields it actually sets.
    assert!(std::mem::size_of::<PerfEventAttr>() >= 56);
}

#[test]
fn id_file_paths_probe_tracefs_then_the_debugfs_mirror() {
    // Order is the contract: canonical tracefs first, the debugfs
    // mirror (root-only) second — the same order attach probes in.
    let paths = tracepoint_id_paths("input", "input_event");
    assert_eq!(paths[0], "/sys/kernel/tracing/events/input/input_event/id");
    assert_eq!(
        paths[1],
        "/sys/kernel/debug/tracing/events/input/input_event/id"
    );
}

#[test]
fn id_parser_accepts_padded_and_newline_terminated_ids() {
    // The kernel writes the id with a trailing newline; trim handles
    // that and any stray padding. Zero stays a valid id (it would be a
    // bizarre tracepoint, but the file said so).
    assert_eq!(parse_tracepoint_id("/p", "1234\n").unwrap(), 1234);
    assert_eq!(parse_tracepoint_id("/p", "  42  ").unwrap(), 42);
    assert_eq!(parse_tracepoint_id("/p", "0").unwrap(), 0);
}

#[test]
fn id_parser_names_the_path_on_malformed_content() {
    // A malformed id file must carry the path in the error — attach
    // reports it verbatim, so the operator learns WHICH file lied.
    let err = parse_tracepoint_id("/some/id", "not-a-number").expect_err("malformed id must error");
    assert!(err.contains("/some/id"), "{err}");
    assert!(err.contains("not a number"), "{err}");
    // An empty file is malformed too: never guess a zero id from it.
    let err = parse_tracepoint_id("/some/id", "").expect_err("empty id file must error");
    assert!(err.contains("not a number"), "{err}");
}
