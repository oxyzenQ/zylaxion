// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-frontier-Z2 B1: the notice-drive lane — PROOF 8, the
//! kernel-side half of the evented control plane.
//!
//! The full loop a control change rides: verb → control map write →
//! ONE BPF_PROG_TEST_RUN on the pinned relay → the kernel produces a
//! record into the pinned notices ring → the watcher's poll(2) wakes
//! → the control map re-read (authority). This lane proves the
//! kernel half end-to-end on a real kernel:
//!
//! 1. the pinned relay OPENS and its TEST_RUN is accepted (the
//!    program RAN — the verdict names it);
//! 2. the notice ring's fd WAKES a poller (the evented wake — the
//!    measured number is the row, gated far under the 10 s heartbeat);
//! 3. the ring's bytes are mmap-introspected and the record echoes
//!    the frame the writer sent (writer bytes → relay → ring →
//!    reader bytes, nothing invented between).
//!
//! The player half (the two-fd poll, the authority row) runs in the
//! e2e leg beside this lane: the leg starts the real player, drives
//! a real verb, and greps the wake and apply rows from its log. The
//! lane rides the PINS (the attach hand-over owns the paths), so a
//! host without an attach reads the honest skip — recorded, never
//! faked.
//!
//! Pin paths are mirrored from the loader's single source
//! (`src/ebpf/pin.rs`) — the harness is a separate crate and cannot
//! import them; a path drift between the two shows up as a skip and
//! a failing e2e leg, named loudly.

use std::io;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use crate::progrun::{bpf, Attr, BPF_PROG_TEST_RUN};

const BPF_OBJ_GET: u64 = 7;
const BPF_OBJ_GET_INFO_BY_FD: u64 = 15;

/// The pinned notice plane (mirrors src/ebpf/pin.rs).
const PIN_PROG_RELAY: &str = "/sys/fs/bpf/zylaxion/relay";
const PIN_MAP_NOTICES: &str = "/sys/fs/bpf/zylaxion/notices";
const PIN_MAP_CONTROL: &str = "/sys/fs/bpf/zylaxion/control";

/// The notice record's schema — the harness crate compiles the
/// crate's OWN copy through the #[path] tree, so the schema is
/// single-sourced for real; only the pin strings above are mirrors.
use crate::notice::{ControlNotice, KIND_VOLUME, NOTICE_RECORD_SIZE};

const BPF_MAP_UPDATE_ELEM: u64 = 2;

/// The e2e guest's boot sequence chowns the pins; the watcher and
/// this lane open them as the desktop user. Local runs (no attach)
/// read the skip.
static SEQ: AtomicU32 = AtomicU32::new(1);

/// The poll gate: an evented wake is microseconds; anything beyond
/// five seconds means the ring never rang — the heartbeat lane's
/// territory (10 s), and a FAIL.
const WAKE_POLL_TIMEOUT_MS: i32 = 5_000;
const WAKE_GATE_NS: u64 = 5_000_000_000;

pub fn run_notice_drive() {
    // The relay pin: open or skip.
    let relay = match open_pinned(PIN_PROG_RELAY) {
        Ok(fd) => fd,
        Err(e) => skip(format!("the pinned relay is not openable ({e}) — attach first; recorded, not faked")),
    };
    let ring = match open_pinned(PIN_MAP_NOTICES) {
        Ok(fd) => fd,
        Err(e) => skip(format!("the pinned notice ring is not openable ({e}) — attach first; recorded, not faked")),
    };
    // The control map: the relay's argument channel. The request slots
    // ride THIS map (slots 6/7 — the slot transport), never the ring:
    // a ringbuf has no update op and the kernel answers a map update
    // on it with EINVAL (the Z2 CI guest named the mislaid fd — the
    // ring's fd went where the control map's belonged).
    let control = match open_pinned(PIN_MAP_CONTROL) {
        Ok(fd) => fd,
        Err(e) => skip(format!("the pinned control map is not openable ({e}) — attach first; recorded, not faked")),
    };

    // The ring's data size comes from the KERNEL (the map info), not
    // from a local constant: the mmap layout must match the object
    // the loader actually built, and the kernel is the authority.
    let data_size = match ringbuf_data_size(ring.as_raw_fd()) {
        Ok(n) => n,
        Err(e) => skip(format!("the notice ring's size did not read back ({e})")),
    };
    println!(
        "PROOF 8: the notice ring is open (data size {data_size} bytes, kernel-queried)"
    );

    // 1. Publish: write the request slots on the pinned control map,
    //    then drive the relay once with an empty payload (the slot
    //    transport — the relay reads the control map's request
    //    channel, not the packet, and never the ring).
    write_request_slots(control.as_raw_fd(), KIND_VOLUME, 61)
        .unwrap_or_else(|e| fail(format!("the request slots did not write: {e}")));
    let started = Instant::now();
    let (retval, duration) = match test_run_once(relay.as_raw_fd()) {
        Ok(pair) => pair,
        Err(e) => fail(format!("the relay's test run refused: {e}")),
    };
    let drive_ns = started.elapsed().as_nanos() as u64;
    println!(
        "PROOF 8: the pinned relay RAN (test run accepted, verdict {retval}, {duration} ns kernel-reported, {drive_ns} ns syscall round trip)"
    );

    // 2. The evented wake: the ring's fd wakes the poller.
    let started = Instant::now();
    match poll_ring(ring.as_raw_fd()) {
        Ok(true) => {}
        Ok(false) => fail(format!(
            "the notice ring did NOT wake inside {} ms — the evented wake is dead",
            WAKE_POLL_TIMEOUT_MS
        )),
        Err(e) => fail(format!("the wake poll failed: {e}")),
    }
    let wake_ns = started.elapsed().as_nanos() as u64;
    if wake_ns >= WAKE_GATE_NS {
        fail(format!(
            "the wake took {wake_ns} ns — beyond the gate (the heartbeat is 10 s; the evented wake must be far under)"
        ));
    }
    println!(
        "PROOF 8: the notice ring WOKE the poller in {wake_ns} ns (evented; the heartbeat floor is 10 s)"
    );

    // 3. The bytes: mmap the ring, drain as the consumer, and verify
    //    the record echoes the request. The guest leg is a
    //    single consumer, so the first record is this publish's.
    let record = match drain_ring(ring.as_raw_fd(), data_size) {
        Ok(recs) => recs.into_iter().next(),
        Err(e) => fail(format!("the ring did not drain: {e}")),
    };
    let Some(record) = record else {
        fail("the ring holds no record — the relay ran but the ring never heard it".to_string());
    };
    if record.kind != KIND_VOLUME || record.value != 61 {
        fail(format!(
            "the record does not echo the request (kind {} value {} — expected kind {} value 61)",
            record.kind, record.value, KIND_VOLUME
        ));
    }
    println!(
        "PROOF 8: the record echoes the request — kind {} value {}, kernel-stamped ns {}",
        record.kind, record.value, record.ns
    );
    println!(
        "MASS-RESULT: PROOF 8 notice-drive lane (the kernel produced the wake, or the lane is honestly classified) — PASS"
    );
    std::process::exit(0);
}

fn skip(why: String) -> ! {
    println!("PROOF 8: SKIP — {why}");
    println!(
        "MASS-RESULT: PROOF 8 notice-drive lane — SKIP (recorded, not faked)"
    );
    std::process::exit(0);
}

fn fail(why: String) -> ! {
    println!("PROOF 8: FAIL — {why}");
    println!("MASS-RESULT: PROOF 8 notice-drive lane — FAIL");
    std::process::exit(1);
}

/// BPF_OBJ_GET on a pin path.
fn open_pinned(path: &str) -> io::Result<OwnedFd> {
    let c_path = std::ffi::CString::new(path)
        .map_err(|e| io::Error::other(format!("{path:?} is not a C string: {e}")))?;
    let mut attr = Attr::new();
    attr.put64(0, c_path.as_ptr() as u64);
    let rc = bpf(BPF_OBJ_GET, &attr).map_err(|e| io::Error::from_raw_os_error(e))?;
    // The fd rides the attr's first word on success (the kernel writes
    // it back through the caller's attr copy).
    let fd = rc;
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// The ring's data size from the kernel (BPF_OBJ_GET_INFO_BY_FD —
/// bpf_map_info.max_entries at offset 16).
fn ringbuf_data_size(map_fd: i32) -> io::Result<usize> {
    let mut info = [0u8; 128];
    let mut attr = Attr::new();
    attr.put32(0, map_fd as u32);
    attr.put32(4, info.len() as u32);
    attr.put64(8, info.as_mut_ptr() as u64);
    bpf(BPF_OBJ_GET_INFO_BY_FD, &attr)
        .map_err(|e| io::Error::from_raw_os_error(e))?;
    let max_entries = u32::from_le_bytes(
        info[16..20]
            .try_into()
            .expect("the info buffer is fixed 128 bytes"),
    );
    if max_entries == 0 || !max_entries.is_power_of_two() {
        return Err(io::Error::other(format!(
            "the kernel reported a non-ringbuf-shaped size: {max_entries}"
        )));
    }
    Ok(max_entries as usize)
}

/// One TEST_RUN on the pinned relay with the frame floor the skb
/// test run itself enforces: `bpf_prog_test_run_skb` refuses
/// data_size_in < ETH_HLEN (14) with EINVAL, and the slot transport
/// carries the arguments in the control map's request slots, so the
/// 14 zero bytes are transport overhead the relay never reads (the
/// userspace crate's own bpf_syscall::TEST_RUN_SKB_MIN_FRAME names
/// the same law; the harness cannot import it — a separate crate —
/// so the constant is mirrored here, and a drift shows up as this
/// exact EINVAL, named loudly).
fn test_run_once(prog_fd: i32) -> io::Result<(u32, u32)> {
    const MIN_FRAME: usize = 14;
    let frame = [0u8; MIN_FRAME];
    let mut attr = Attr::new();
    attr.put32(0, prog_fd as u32);
    attr.put32(8, frame.len() as u32);
    attr.put64(16, frame.as_ptr() as u64);
    bpf(BPF_PROG_TEST_RUN, &attr).map_err(|e| io::Error::from_raw_os_error(e))?;
    Ok((attr.get32(4), attr.get32(36)))
}

/// Write the two request slots on the pinned control map
/// (BPF_MAP_UPDATE_ELEM on an array: key = slot index, value = the
/// u32). The slots are the relay's argument channel, never state.
/// The attr field law is the map_elem head — `{ map_fd@0, key@8,
/// value@16, flags@24 }` — the same shape the PROOF 7 lane's
/// map_update rides; the Z2 CI guest named the one-slot-late
/// version (the key pointer slot carrying the integer 4, the flags
/// slot carrying a pointer — the kernel answers the huge flags with
/// EINVAL before any byte moves).
fn write_request_slots(map_fd: i32, kind: u32, value: u32) -> io::Result<()> {
    for (slot, val) in [(6u32, kind), (7u32, value)] {
        let mut attr = Attr::new();
        attr.put64(0, map_fd as u64);
        attr.put64(8, &slot as *const u32 as u64);
        attr.put64(16, &val as *const u32 as u64);
        attr.put32(24, 0);
        bpf(BPF_MAP_UPDATE_ELEM, &attr).map_err(|e| io::Error::from_raw_os_error(e))?;
    }
    Ok(())
}

/// poll(2) on the ring fd — the evented wake the watcher rides.
fn poll_ring(map_fd: i32) -> io::Result<bool> {
    let fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(map_fd) };
    let mut fds = [libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];
    let rc = unsafe { libc::poll(fds.as_mut_ptr(), 1, WAKE_POLL_TIMEOUT_MS) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(rc > 0 && fds[0].revents & libc::POLLIN != 0)
}

/// mmap-drain the ring as the consumer: producer page (read-only),
/// consumer page (read-write — draining ADVANCES the consumer
/// position, the contract every ringbuf consumer obeys), then the
/// data pages. Returns every committed record found.
fn drain_ring(map_fd: i32, data_size: usize) -> io::Result<Vec<ControlNotice>> {
    use std::sync::atomic::AtomicU32;
    const PAGE: usize = 4096;
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
    // The kernel maps pages, not byte counts: the data area spans
    // ceil(data_size / page) pages plus the wrap page.
    let data_pages = data_size.div_ceil(page_size) + 1;
    let prod = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            page_size,
            libc::PROT_READ,
            libc::MAP_SHARED,
            map_fd,
            0,
        )
    };
    if prod == libc::MAP_FAILED {
        return Err(io::Error::last_os_error());
    }
    let cons = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            page_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            map_fd,
            page_size as libc::off_t,
        )
    };
    if cons == libc::MAP_FAILED {
        unsafe { libc::munmap(prod, page_size) };
        return Err(io::Error::last_os_error());
    }
    let data = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            data_pages * page_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            map_fd,
            2 * page_size as libc::off_t,
        )
    };
    if data == libc::MAP_FAILED {
        unsafe { libc::munmap(cons, page_size) };
        unsafe { libc::munmap(prod, page_size) };
        return Err(io::Error::last_os_error());
    }

    // Positions ride the atomic acquire/release discipline libbpf's
    // consumer uses (the kernel publishes prod with a release store;
    // the consumer publishes its drain with a release store back).
    let prod_pos = unsafe { &*(prod as *const AtomicU32) };
    let cons_pos = unsafe { &*(cons as *const AtomicU32) };
    let data_bytes = unsafe { std::slice::from_raw_parts(data as *const u8, data_pages * page_size) };

    let mut records = Vec::new();
    let start = prod_pos.load(Ordering::Acquire);
    let mut pos = cons_pos.load(Ordering::Relaxed);
    while pos != start {
        let off = (pos as usize) & (data_size - 1);
        let hdr = u64::from_le_bytes(
            data_bytes[off..off + 8]
                .try_into()
                .expect("off is 8-aligned inside the ring"),
        );
        // Busy (bit 14) means the producer is mid-reserve — nothing to
        // read yet; discard (bit 15) records are skipped.
        const BUSY: u64 = 1 << 14;
        const DISCARD: u64 = 1 << 15;
        let len = (hdr & !(BUSY | DISCARD)) as usize;
        let committed = hdr & BUSY == 0;
        if committed && len >= NOTICE_RECORD_SIZE {
            let body = &data_bytes[off + 8..off + 8 + len];
            let ns = u64::from_le_bytes(body[0..8].try_into().expect("fixed"));
            let kind = u32::from_le_bytes(body[8..12].try_into().expect("fixed"));
            let value = u32::from_le_bytes(body[12..16].try_into().expect("fixed"));
            records.push(ControlNotice { ns, kind, value });
        }
        pos = pos.wrapping_add((8 + len).max(8).next_multiple_of(8) as u32);
    }
    // The drain: publish the advanced consumer position.
    cons_pos.store(pos, Ordering::Release);

    unsafe { libc::munmap(data, data_pages * page_size) };
    unsafe { libc::munmap(cons, page_size) };
    unsafe { libc::munmap(prod, page_size) };
    Ok(records)
}
