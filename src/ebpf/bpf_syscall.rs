// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Raw syscall helpers for the tracepoint attach + link pin
//! (night-improve-23, the zelynic bpf_syscall precedent).
//!
//! ## Why this module exists
//!
//! Aya 0.14's `TracePoint::attach()` creates a `bpf_link` (fd-based,
//! kernel 5.10+ via the BPF_PERF_EVENT link type) — but the resulting
//! `TracePointLink` wraps a `pub(crate)` inner (`PerfLinkInner::Fd`),
//! so from outside the crate the FdLink cannot be extracted to call its
//! `pin()` method. Without a pinned link, the attachment dies the moment
//! the attach process exits — and a persistent attachment IS the product
//! here: the tap must keep tapping after `sudo zylaxion attach` returns.
//!
//! ## The lane
//!
//! Bypass aya's `attach()` and do it ourselves via raw syscalls, the
//! same shape aya's own `perf_attach` uses when the kernel supports
//! bpf_perf_link (5.10+, far inside the verified 5.15 floor):
//!
//! 1. `perf_event_open` — a PERF_TYPE_TRACEPOINT event on the input
//!    tracepoint (the attach handle; the kernel stores the program on
//!    the tracepoint object itself, so one handle suffices)
//! 2. `BPF_LINK_CREATE` (attach type `BPF_PERF_EVENT`) — the link fd
//! 3. `BPF_OBJ_PIN` — pin the link to bpffs so it survives process exit
//!
//! The pin file keeps the link alive in the kernel even after both fds
//! are closed and the process exits. Removing the pin file detaches.

use std::io;
use std::os::fd::RawFd;

// ━━ Syscall constants (linux/bpf.h, linux/perf_event.h) ━━

/// BPF syscall command: create a bpf_link (kernel 5.7+).
const BPF_LINK_CREATE: i32 = 28;
/// BPF syscall command: pin a bpf object to bpffs.
const BPF_OBJ_PIN: i32 = 6;
/// BPF attach type for perf-event-backed programs (tracepoints,
/// kprobes) — the value aya's own generated bindings carry.
const BPF_PERF_EVENT: u32 = 41;
/// perf_event_open type for tracepoints (linux/perf_event.h).
const PERF_TYPE_TRACEPOINT: u32 = 2;
/// perf_event_open flag: close the fd on exec.
const PERF_FLAG_FD_CLOEXEC: u64 = 8;

/// Attribute struct for `BPF_LINK_CREATE`.
///
/// Layout matches the head of `union bpf_attr`'s link_create member:
/// `{ prog_fd, target_fd, attach_type, flags }`. The kernel reads
/// fields by the attr size handed to the syscall, so the 56-byte shape
/// (16 field bytes + 40 zero bytes) is sufficient — the zelynic
/// precedent, carried over.
#[repr(C)]
struct LinkCreateAttr {
    prog_fd: u32,
    target_fd: u32,
    attach_type: u32,
    flags: u32,
    _pad: [u8; 40],
}

impl Default for LinkCreateAttr {
    fn default() -> Self {
        Self {
            prog_fd: 0,
            target_fd: 0,
            attach_type: 0,
            flags: 0,
            _pad: [0u8; 40],
        }
    }
}

/// Attribute struct for `BPF_OBJ_PIN`.
#[repr(C)]
#[derive(Default)]
struct ObjPinAttr {
    pathname: u64,
    bpf_fd: u32,
    file_flags: u32,
}

/// Attribute struct for `perf_event_open` (linux/perf_event.h). Only
/// the fields the tracepoint lane needs are named; the rest zero.
#[repr(C)]
struct PerfEventAttr {
    attr_type: u32,
    size: u32,
    config: u64,
    sample_period_or_freq: u64,
    sample_type: u64,
    read_format: u64,
    wakeup_events: u32,
    _pad: u32,
    raw: u64,
}

impl Default for PerfEventAttr {
    fn default() -> Self {
        // Zeroed everywhere, then the size field — the kernel rejects a
        // size of 0 and reads exactly this many bytes.
        Self {
            attr_type: 0,
            size: std::mem::size_of::<PerfEventAttr>() as u32,
            config: 0,
            sample_period_or_freq: 0,
            sample_type: 0,
            read_format: 0,
            wakeup_events: 0,
            _pad: 0,
            raw: 0,
        }
    }
}

/// The `id` file of one tracepoint: the numeric id the perf event
/// opens. Reads `/sys/kernel/tracing/events/<category>/<name>/id` (the
/// canonical tracefs location; the debugfs mirror needs root anyway —
/// and attach runs as root, so both mounts work: try tracefs first,
/// then the debugfs fallback).
pub fn tracepoint_id(category: &str, name: &str) -> Result<u64, String> {
    let candidates = [
        format!("/sys/kernel/tracing/events/{category}/{name}/id"),
        format!("/sys/kernel/debug/tracing/events/{category}/{name}/id"),
    ];
    for path in candidates {
        if let Ok(content) = std::fs::read_to_string(&path) {
            return content
                .trim()
                .parse::<u64>()
                .map_err(|e| format!("tracepoint id at {path} is not a number: {e}"));
        }
    }
    Err(
        "the input_event tracepoint id is not readable (tried tracefs and the \
         debugfs mirror) — the tracepoint may not exist on this kernel"
            .to_string(),
    )
}

/// Open the tracepoint perf event (the attach handle). Mirrors aya's
/// own `perf_event_open_trace_point` shape: system-wide handle (pid -1)
/// on cpu 0 — for BPF tracepoint programs the kernel stores the program
/// on the shared tracepoint object, so the cpu field carries no
/// scoping; it is just where the handle lives.
fn perf_event_open_tracepoint(event_id: u64) -> Result<RawFd, String> {
    // Field-literal init (the clippy field_reassign_with_default rule):
    // everything not named here stays zero, and Default fills the size
    // field — zeroed everywhere else is exactly what the kernel wants.
    let mut attr = PerfEventAttr {
        config: event_id,
        ..PerfEventAttr::default()
    };
    attr.attr_type = PERF_TYPE_TRACEPOINT;
    attr.wakeup_events = 1;
    let fd = unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            &attr as *const PerfEventAttr,
            -1_isize, // pid: all processes
            0_isize,  // cpu 0 (the handle's home, not a scoping limit)
            -1_isize, // group fd: none
            PERF_FLAG_FD_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(format!(
            "perf_event_open on the tracepoint failed: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(fd as RawFd)
}

/// Create the bpf_link for the tracepoint and pin it — the whole
/// attach, in three syscalls. On success the caller owns nothing: the
/// pin keeps the link (and the attachment) alive in the kernel, and
/// both raw fds are closed here. Errors are flattened to strings (the
/// loader's error channel); each one names the syscall that failed.
pub fn create_and_pin_tracepoint_link(
    prog_fd: RawFd,
    category: &str,
    name: &str,
    pin_path: &str,
) -> Result<(), String> {
    let tp_id = tracepoint_id(category, name)?;

    // 1. The perf event handle.
    let perf_fd = perf_event_open_tracepoint(tp_id)?;

    // 2. The link (BPF_PERF_EVENT: perf-event-backed programs).
    let link_attr = LinkCreateAttr {
        prog_fd: prog_fd as u32,
        target_fd: perf_fd as u32,
        attach_type: BPF_PERF_EVENT,
        flags: 0,
        ..Default::default()
    };
    let link_fd = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            BPF_LINK_CREATE,
            &link_attr as *const LinkCreateAttr,
            std::mem::size_of::<LinkCreateAttr>(),
        )
    };
    if link_fd < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(perf_fd) };
        return Err(format!("BPF_LINK_CREATE failed: {err}"));
    }
    let link_fd = link_fd as RawFd;

    // 3. Pin the link. A stale pin file (crashed attach) is removed
    //    first — the attach path cleaned the pin set already, this is
    //    the belt to that suspenders.
    let _ = std::fs::remove_file(pin_path);
    let c_path = std::ffi::CString::new(pin_path)
        .map_err(|e| format!("pin path {pin_path} is not a valid C string: {e}"))?;
    let pin_attr = ObjPinAttr {
        pathname: c_path.as_ptr() as u64,
        bpf_fd: link_fd as u32,
        file_flags: 0,
    };
    let pinned = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            BPF_OBJ_PIN,
            &pin_attr as *const ObjPinAttr,
            std::mem::size_of::<ObjPinAttr>(),
        )
    };
    // The raw fds are owned nowhere: on pin success the bpffs file owns
    // the link; on failure both close. Either way nothing leaks.
    unsafe { libc::close(link_fd) };
    unsafe { libc::close(perf_fd) };
    if pinned < 0 {
        return Err(format!(
            "BPF_OBJ_PIN failed for {pin_path}: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../test/ebpf/bpf_syscall_tests.rs"]
mod tests;
