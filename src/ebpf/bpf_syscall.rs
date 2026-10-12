// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Raw syscall helpers for the probe-link attach lanes + the failure
//! marker (night-improve-23; night-improve-25 retools the lane for the
//! fentry → kprobe chain, the zelynic bpf_syscall precedent).
//!
//! ## Why this module exists
//!
//! Aya 0.14's typed attach methods return links whose inner handles
//! are `pub(crate)` (`FEntryLink` wraps an `FdLink`, `KProbeLink` a
//! `PerfLinkInner`), so from outside the crate the link cannot be
//! extracted to call its `pin()` method. Without a pinned link, the
//! attachment dies the moment the attach process exits — and a
//! persistent attachment IS the product here: the tap must keep
//! tapping after `sudo zylaxion attach` returns.
//!
//! ## The lanes
//!
//! All three mirror aya's own shapes exactly:
//!
//! 1. **fentry link** — `BPF_RAW_TRACEPOINT_OPEN` with a NULL name
//!    (aya's `FEntry::attach` shape: the kernel routes a tracing
//!    program with a null name to `bpf_tracing_prog_attach`, which
//!    binds the link to the program's load-time `attach_btf_id`).
//!    Kernel 5.5+, inside the 5.15 floor.
//! 2. **kprobe link** — `perf_event_open` on the dynamic `kprobe` PMU
//!    (`/sys/bus/event_source/devices/kprobe/type`, the value aya's
//!    `read_sys_fs_perf_type` reads) with `config1` carrying the symbol
//!    name pointer (the kernel auto-registers the probe, 4.17+), then
//!    `BPF_LINK_CREATE` with `BPF_PERF_EVENT` (5.10+, the same
//!    `perf_attach` shape the tracepoint lane used).
//! 3. **the failure marker** — `BPF_MAP_CREATE` then
//!    `BPF_MAP_UPDATE_ELEM` then `BPF_OBJ_PIN`: a one-entry array map
//!    on bpffs holding the [`AttachError::code`] of the last failed
//!    attach, so the doctor can name the failure after the process is
//!    gone.
//!
//! Every lane ends with the same step: `BPF_OBJ_PIN` — the pin file
//! keeps the link (or the marker map) alive in the kernel even after
//! all fds are closed and the process exits. Removing the pin file
//! detaches (or forgets the marker).

use std::io;
use std::os::fd::RawFd;

use super::attach_error::AttachError;

// ━━ Syscall constants (linux/bpf.h, linux/perf_event.h) ━━

/// BPF syscall command: create a map (the marker lane).
const BPF_MAP_CREATE: i32 = 0;
/// BPF syscall command: write one map entry (the marker lane).
const BPF_MAP_UPDATE_ELEM: i32 = 2;
/// BPF syscall command: pin a bpf object to bpffs.
const BPF_OBJ_PIN: i32 = 6;
/// BPF syscall command: attach a tracing program (aya's FEntry shape).
const BPF_RAW_TRACEPOINT_OPEN: i32 = 17;
/// BPF syscall command: create a bpf_link (kernel 5.7+).
const BPF_LINK_CREATE: i32 = 28;
/// BPF map type: array (the marker is a one-entry array).
const BPF_MAP_TYPE_ARRAY: u32 = 2;
/// BPF attach type for perf-event-backed programs (kprobes) — the
/// value aya's own generated bindings carry.
const BPF_PERF_EVENT: u32 = 41;
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

/// Attribute struct for `BPF_RAW_TRACEPOINT_OPEN`: the kernel's
/// `raw_tracepoint` member is `{ __aligned_u64 name; __u32 prog_fd; }`
/// — a NULL name with a tracing program routes to the tracing-link
/// path (aya's own `FEntry::attach` lane).
#[repr(C)]
#[derive(Default)]
struct RawTracepointOpenAttr {
    name: u64,
    prog_fd: u32,
    _pad: u32,
}

/// Attribute struct for `perf_event_open` (linux/perf_event.h).
///
/// Only the head the kprobe lane needs is carried: through `config2`
/// (offset 64), the PERF_ATTR_SIZE_VER1 shape. `config1` is the
/// `kprobe_func` slot — the kernel reads the symbol name pointer from
/// it and auto-registers the probe; `config2` is the probe offset
/// (zero). The `flags` word at offset 40 stays zero: the event opens
/// enabled, and the kernel enables the program via the link (the
/// exact posture aya's own kprobe lane uses).
#[repr(C)]
struct PerfEventAttr {
    attr_type: u32,
    size: u32,
    config: u64,
    sample_period_or_freq: u64,
    sample_type: u64,
    read_format: u64,
    flags: u64,
    wakeup: u32,
    bp_type: u32,
    config1: u64,
    config2: u64,
}

impl PerfEventAttr {
    /// The 72-byte PERF_ATTR_SIZE_VER1 shape (through `config2`):
    /// accepted by every kernel since 4.1 for the fields this lane
    /// sets, and it never overstates the attr size to older kernels
    /// (the syscall rejects sizes larger than the running kernel's
    /// own struct with E2BIG — the reason this stays 72 and not the
    /// 128-byte modern shape).
    const KPROBE_ATTR_SIZE: u32 = 72;
}

impl Default for PerfEventAttr {
    fn default() -> Self {
        // Zeroed everywhere, then the size field — the kernel rejects
        // a size of 0 and reads exactly this many bytes.
        Self {
            attr_type: 0,
            size: Self::KPROBE_ATTR_SIZE,
            config: 0,
            sample_period_or_freq: 0,
            sample_type: 0,
            read_format: 0,
            flags: 0,
            wakeup: 0,
            bp_type: 0,
            config1: 0,
            config2: 0,
        }
    }
}

/// Attribute struct for `BPF_MAP_CREATE` (the marker lane): the head
/// of the map_create member, `{ map_type, key_size, value_size,
/// max_entries, map_flags, inner_map_fd, numa_node }` — 28 bytes; the
/// kernel reads the rest of its union copy as zero (map_name empty,
/// an anonymous map, addressable by its pin path exactly like the
/// tap's own maps).
#[repr(C)]
#[derive(Default)]
struct MapCreateAttr {
    map_type: u32,
    key_size: u32,
    value_size: u32,
    max_entries: u32,
    map_flags: u32,
    inner_map_fd: u32,
    numa_node: u32,
}

/// Attribute struct for `BPF_MAP_UPDATE_ELEM` (the marker lane):
/// `{ map_fd, pad, key ptr, value ptr, flags }` — the 32-byte head of
/// the map_elem member.
#[repr(C)]
#[derive(Default)]
struct MapUpdateAttr {
    map_fd: u32,
    _pad: u32,
    key: u64,
    value: u64,
    flags: u64,
}

// ━━ The fentry lane ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Create the tracing link for an fentry program and pin it — the
/// whole attach, in two syscalls. The program must have been loaded
/// by aya's `FEntry::load` (which resolved the target's BTF id at
/// load time; the kernel binds the link to it here). On success the
/// caller owns nothing: the pin keeps the link (and the attachment)
/// alive in the kernel, and the raw link fd is closed here.
pub fn create_and_pin_fentry_link(prog_fd: RawFd, pin_path: &str) -> Result<(), AttachError> {
    // 1. The tracing link (aya's FEntry::attach shape: a raw
    //    tracepoint open with a NULL name routes tracing programs to
    //    the trampoline link).
    let attr = RawTracepointOpenAttr {
        name: 0, // NULL: not a raw tracepoint name — the tracing path
        prog_fd: prog_fd as u32,
        _pad: 0,
    };
    let link_fd = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            BPF_RAW_TRACEPOINT_OPEN,
            &attr as *const RawTracepointOpenAttr,
            std::mem::size_of::<RawTracepointOpenAttr>(),
        )
    };
    if link_fd < 0 {
        return Err(AttachError::from_fentry_link(
            io::Error::last_os_error().raw_os_error().unwrap_or(0),
        ));
    }
    let link_fd = link_fd as RawFd;

    // 2. Pin the link.
    let pinned = pin_object_fd(link_fd, pin_path);
    // The raw fd is owned nowhere: on pin success the bpffs file owns
    // the link; on failure it closes. Either way nothing leaks.
    unsafe { libc::close(link_fd) };
    pinned.map_err(|e| pin_failure(pin_path, &e))
}

// ━━ The kprobe lane ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The dynamic PMU type of the kernel's kprobe event source — the
/// value aya's own `read_sys_fs_perf_type("kprobe")` reads from
/// `/sys/bus/event_source/devices/kprobe/type`. Every kernel since
/// 4.17 registers this PMU; a missing or unparsable file means the
/// kprobe lane cannot run at all.
fn kprobe_pmu_type() -> Result<u32, io::Error> {
    let content = std::fs::read_to_string("/sys/bus/event_source/devices/kprobe/type")?;
    content
        .trim()
        .parse::<u32>()
        .map_err(|e| io::Error::other(format!("kprobe PMU type is not a number: {e}")))
}

/// Open the kprobe perf event (the attach handle). Mirrors aya's own
/// `perf_event_open_probe` shape: the dynamic kprobe PMU type, config
/// zero (an ENTRY probe — this lane never retprobes), `config1` the
/// symbol name pointer, `config2` the offset (zero). The kernel
/// auto-registers the probe from the name; registration failures are
/// the classified errnos ([`AttachError::from_kprobe_open`]).
fn perf_event_open_kprobe(pmu_type: u32, symbol: &str) -> Result<RawFd, io::Error> {
    let c_symbol = std::ffi::CString::new(symbol).map_err(|e| {
        io::Error::other(format!(
            "probe symbol {symbol:?} is not a valid C string: {e}"
        ))
    })?;
    // Field-literal init (the clippy field_reassign_with_default rule):
    // Default fills the size field — zeroed everywhere else is
    // exactly what the kernel wants.
    let attr = PerfEventAttr {
        attr_type: pmu_type,
        config1: c_symbol.as_ptr() as u64,
        ..PerfEventAttr::default()
    };
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
        return Err(io::Error::last_os_error());
    }
    Ok(fd as RawFd)
}

/// Create the bpf_link for a kprobe and pin it — the whole attach, in
/// three syscalls. The symbol is a parameter (not baked into the
/// program): the same loaded program serves both `input_event` and
/// the `input_handle_event` alternate. On success the caller owns
/// nothing: the pin keeps the link alive in the kernel, and both raw
/// fds are closed here.
pub fn create_and_pin_kprobe_link(
    prog_fd: RawFd,
    symbol: &str,
    pin_path: &str,
) -> Result<(), AttachError> {
    // 0. The kprobe PMU must be registered (kernel 4.17+; the 5.15
    //    floor sits far above it).
    let pmu_type = kprobe_pmu_type().map_err(|e| AttachError::Unknown {
        detail: format!(
            "the kprobe PMU is not readable (/sys/bus/event_source/devices/kprobe/type): {e}"
        ),
    })?;

    // 1. The perf event handle — the kernel auto-registers the probe
    //    from the symbol name; the errno is the kernel's own verdict
    //    on the symbol (ENOENT missing, EINVAL blacklisted/not
    //    probeable, EPERM privilege).
    let perf_fd = perf_event_open_kprobe(pmu_type, symbol)
        .map_err(|e| AttachError::from_kprobe_open(symbol, e.raw_os_error().unwrap_or(0)))?;

    // 2. The link (BPF_PERF_EVENT: perf-event-backed programs) — the
    //    same shape the tracepoint lane used, mirrored from aya's
    //    perf_attach.
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
        let err =
            AttachError::from_link_create(io::Error::last_os_error().raw_os_error().unwrap_or(0));
        unsafe { libc::close(perf_fd) };
        return Err(err);
    }
    let link_fd = link_fd as RawFd;

    // 3. Pin the link. A stale pin file (crashed attach) is removed
    //    first — the attach path cleaned the pin set already, this is
    //    the belt to that suspenders.
    let pinned = pin_object_fd(link_fd, pin_path);
    // The raw fds are owned nowhere: on pin success the bpffs file
    // owns the link; on failure both close. Either way nothing leaks.
    unsafe { libc::close(link_fd) };
    unsafe { libc::close(perf_fd) };
    pinned.map_err(|e| pin_failure(pin_path, &e))
}

// ━━ The failure marker ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Plant the attach-failure marker: a one-entry u32 array map on
/// bpffs holding [`AttachError::code`]. The doctor reads it to name
/// the last failure; the next attach's self-heal pass and `detach`
/// remove it. A marker failure is logged, never fatal — the real
/// error it accompanies matters more.
pub fn plant_error_marker(code: u32, pin_path: &str) -> Result<(), io::Error> {
    // 1. The map (anonymous array: u32 -> u32, one entry).
    let create_attr = MapCreateAttr {
        map_type: BPF_MAP_TYPE_ARRAY,
        key_size: 4,
        value_size: 4,
        max_entries: 1,
        map_flags: 0,
        inner_map_fd: 0,
        numa_node: 0,
    };
    let map_fd = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            BPF_MAP_CREATE,
            &create_attr as *const MapCreateAttr,
            std::mem::size_of::<MapCreateAttr>(),
        )
    };
    if map_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let map_fd = map_fd as RawFd;

    // 2. The code write (key 0 <- code, no flags).
    let key: u32 = 0;
    let update_attr = MapUpdateAttr {
        map_fd: map_fd as u32,
        _pad: 0,
        key: &raw const key as u64,
        value: &raw const code as u64,
        flags: 0,
    };
    let updated = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            BPF_MAP_UPDATE_ELEM,
            &update_attr as *const MapUpdateAttr,
            std::mem::size_of::<MapUpdateAttr>(),
        )
    };
    if updated < 0 {
        let e = io::Error::last_os_error();
        unsafe { libc::close(map_fd) };
        return Err(e);
    }

    // 3. The pin (a stale marker from an even earlier failed attach
    //    is removed first — the self-heal discipline).
    let pinned = pin_object_fd(map_fd, pin_path);
    // The pin owns the map afterwards; failure closes it here.
    unsafe { libc::close(map_fd) };
    pinned
}

/// Pin one bpf object fd to bpffs. A stale pin file (a crashed attach
/// left it) is removed first — belt to the loader's suspenders.
fn pin_object_fd(fd: RawFd, pin_path: &str) -> Result<(), io::Error> {
    let _ = std::fs::remove_file(pin_path);
    let c_path = std::ffi::CString::new(pin_path).map_err(|e| {
        io::Error::other(format!(
            "pin path {pin_path:?} is not a valid C string: {e}"
        ))
    })?;
    let pin_attr = ObjPinAttr {
        pathname: c_path.as_ptr() as u64,
        bpf_fd: fd as u32,
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
    if pinned < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// The pin-failure classifier: whatever the syscall said, a failed
/// pin is [`AttachError::PinFailed`] at the named path.
fn pin_failure(pin_path: &str, e: &io::Error) -> AttachError {
    AttachError::PinFailed {
        path: format!("{pin_path} ({e})"),
    }
}

#[cfg(test)]
#[path = "../../test/ebpf/bpf_syscall_tests.rs"]
mod tests;
