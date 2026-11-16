// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The relay drive lane (night-frontier-Z2 B1): the raw-syscall half
//! of the evented control plane — one `BPF_PROG_TEST_RUN` on the
//! pinned notice relay, the kernel producing the record. Split from
//! `bpf_syscall.rs` under the 600-line no-mercy law: a pure move, the
//! call sites keep the same discipline (open the pin, drive once, no
//! state held).

use std::io;
use std::os::fd::RawFd;

use crate::ebpf::bpf_syscall::BPF_PROG_TEST_RUN;

/// The skb test run's own frame floor: `bpf_prog_test_run_skb` refuses
/// `data_size_in < ETH_HLEN` (14) with EINVAL — the kernel's own gate
/// in net/bpf/test_run.c. The slot transport carries the relay's
/// arguments in the control map's request slots, so the frame bytes
/// are transport overhead the relay never reads; the floor is met
/// with zero padding, enforced HERE (the single source) so no caller
/// can rediscover the EINVAL with an empty payload (the redesign's
/// first CI guest named exactly that refusal).
pub const TEST_RUN_SKB_MIN_FRAME: usize = 14;

/// Drive the pinned notice relay once (BPF_PROG_TEST_RUN, skb
/// shape): the caller writes the relay's argument channel into the
/// control map's request slots first (kind, then value — the slot
/// transport, night-frontier-Z2 B1), then drives the relay — the
/// kernel fabricates the skb the socket filter's signature expects,
/// the relay reads its arguments from the map and publishes the
/// record into the pinned notices ring. Returns the
/// program's verdict (0 — the filter verdict nobody consumes; the
/// record is the result).
///
/// repeat stays 0: the kernel runs the program exactly once per
/// call, and the caller publishes exactly one notice per control
/// change — no amplification, no batching, one record per verb.
pub fn test_run_skb(prog_fd: RawFd, data_in: &[u8]) -> Result<u32, io::Error> {
    // The frame floor (see TEST_RUN_SKB_MIN_FRAME): a shorter payload
    // rides the zero-padded minimum — the relay never reads the
    // packet, so the padding carries nothing but the kernel's own
    // legality.
    let padded;
    let frame: &[u8] = if data_in.len() < TEST_RUN_SKB_MIN_FRAME {
        padded = [0u8; TEST_RUN_SKB_MIN_FRAME];
        &padded
    } else {
        data_in
    };
    // No `mut`: the kernel writes retval/duration back through the
    // raw pointer (invisible to the borrow checker), and the struct
    // is only read after the call.
    let attr = TestRunAttr {
        prog_fd: prog_fd as u32,
        data_size_in: frame.len() as u32,
        data_in: frame.as_ptr() as u64,
        ..TestRunAttr::zeroed()
    };
    // SAFETY: the attr is a repr(C) TestRunAttr with the uapi head
    // the struct doc spells (prog_fd@0, data_size_in@8, data_in@16);
    // the frame pointer outlives the call, the kernel writes
    // retval/duration back through the caller's attr, and the syscall
    // is the only unsafe surface here.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            BPF_PROG_TEST_RUN,
            &attr as *const TestRunAttr,
            std::mem::size_of::<TestRunAttr>(),
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(attr.retval)
}

/// Attribute struct for `BPF_PROG_TEST_RUN` (the relay drive lane):
/// the head of the `test` member — `{ prog_fd, retval,
/// data_size_in, data_size_out, data_in, data_out, repeat,
/// duration }`. The kernel writes `retval` and `duration` back
/// through the caller's pointer, so the struct is read AFTER the
/// syscall for the verdict. repeat stays 0 (the kernel runs the
/// program exactly once — one control change, one record) and every
/// ctx field stays 0 (the relay reads its arguments from the control
/// map's request slots, not the context — the slot transport,
/// night-frontier-Z2 B1). The kernel zero-fills the attr tail it
/// never copies, so the short 64-byte shape presents clean ctx,
/// flags, cpu and batch_size fields.
#[repr(C)]
struct TestRunAttr {
    prog_fd: u32,
    retval: u32,
    data_size_in: u32,
    data_size_out: u32,
    data_in: u64,
    data_out: u64,
    repeat: u32,
    duration: u32,
    _pad: [u8; 24],
}

impl TestRunAttr {
    const fn zeroed() -> Self {
        Self {
            prog_fd: 0,
            retval: 0,
            data_size_in: 0,
            data_size_out: 0,
            data_in: 0,
            data_out: 0,
            repeat: 0,
            duration: 0,
            _pad: [0u8; 24],
        }
    }
}
