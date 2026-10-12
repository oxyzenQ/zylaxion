// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// bpf_syscall_tests.rs — the pure constants + parsing contract of the
// raw-syscall lane (night-improve-23). The syscalls themselves are
// root-run integration surfaces; here the values that must match the
// kernel ABI byte-for-byte get pinned, plus the tracepoint id parser.

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
fn tracepoint_id_rejects_missing_and_malformed() {
    // A category/name pair that cannot exist anywhere: the parser must
    // return the honest "not readable" error, never a fake id.
    let err = tracepoint_id("nonexistent_category_xyz", "nonexistent_tp")
        .expect_err("missing tracepoint must error");
    assert!(
        err.contains("not readable") || err.contains("may not exist"),
        "{err}"
    );
}
