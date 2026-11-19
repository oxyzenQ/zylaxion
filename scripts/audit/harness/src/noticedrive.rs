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
use std::sync::atomic::AtomicU32;
use std::time::Instant;

use crate::progrun::{Attr, BPF_PROG_TEST_RUN, bpf};
use crate::ringdrain::{drain_ring_records, ringbuf_data_size};

const BPF_OBJ_GET: u64 = 7;

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
        Err(e) => skip(format!(
            "the pinned relay is not openable ({e}) — attach first; recorded, not faked"
        )),
    };
    let ring = match open_pinned(PIN_MAP_NOTICES) {
        Ok(fd) => fd,
        Err(e) => skip(format!(
            "the pinned notice ring is not openable ({e}) — attach first; recorded, not faked"
        )),
    };
    // The control map: the relay's argument channel. The request slots
    // ride THIS map (slots 6/7 — the slot transport), never the ring:
    // a ringbuf has no update op and the kernel answers a map update
    // on it with EINVAL (the Z2 CI guest named the mislaid fd — the
    // ring's fd went where the control map's belonged).
    let control = match open_pinned(PIN_MAP_CONTROL) {
        Ok(fd) => fd,
        Err(e) => skip(format!(
            "the pinned control map is not openable ({e}) — attach first; recorded, not faked"
        )),
    };

    // The ring's data size comes from the KERNEL (the map info), not
    // from a local constant: the mmap layout must match the object
    // the loader actually built, and the kernel is the authority.
    let data_size = match ringbuf_data_size(ring.as_raw_fd()) {
        Ok(n) => n,
        Err(e) => skip(format!("the notice ring's size did not read back ({e})")),
    };
    println!("PROOF 8: the notice ring is open (data size {data_size} bytes, kernel-queried)");

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
    //    the record echoes the request. The guest leg has had TWO
    //    consumers racing one ring (the player's watcher drained the
    //    player-half records, and this lane drains its own), so the
    //    echo is searched across every record this drain holds — the
    //    first-consumer assumption was a named lie the guest caught.
    let records = match drain_ring(ring.as_raw_fd(), data_size) {
        Ok(recs) => recs,
        Err(e) => fail(format!("the ring did not drain: {e}")),
    };
    println!("PROOF 8: the drain holds {} record(s)", records.len());
    let record = records
        .iter()
        .find(|r| r.kind == KIND_VOLUME && r.value == 61);
    let Some(record) = record else {
        let shapes: Vec<String> = records
            .iter()
            .map(|r| format!("kind {} value {}", r.kind, r.value))
            .collect();
        fail(format!(
            "the ring holds no echo of this publish ({} record(s): {})",
            records.len(),
            if shapes.is_empty() {
                "none".to_string()
            } else {
                shapes.join(", ")
            }
        ));
    };
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
    println!("MASS-RESULT: PROOF 8 notice-drive lane — SKIP (recorded, not faked)");
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
    // SAFETY: fd is a fresh fd the kernel returned from a successful
    // BPF_OBJ_GET (the rc<0 path errored above), so this constructor
    // takes sole ownership exactly once.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// mmap-drain the notice ring as the consumer through the generic
/// ringbuf lane (night-frontier-Z5: the kernel's mmap law lives in
/// `ringdrain.rs`, shared with the tap-golden corpus) and decode the
/// 16-byte notice records.
fn drain_ring(map_fd: i32, data_size: usize) -> io::Result<Vec<ControlNotice>> {
    let records = drain_ring_records(map_fd, data_size, NOTICE_RECORD_SIZE)?;
    Ok(records
        .into_iter()
        .map(|r| {
            let b = &r.bytes;
            ControlNotice {
                ns: u64::from_le_bytes(b[0..8].try_into().expect("fixed record size")),
                kind: u32::from_le_bytes(b[8..12].try_into().expect("fixed record size")),
                value: u32::from_le_bytes(b[12..16].try_into().expect("fixed record size")),
            }
        })
        .collect())
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
    // SAFETY: borrow_raw wraps an fd the caller owns for the whole
    // call; the BorrowedFd never outlives the poll below.
    let fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(map_fd) };
    let mut fds = [libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];
    // SAFETY: poll takes the array's own pointer and length — the
    // array outlives the call and the kernel only writes the revents
    // field.
    let rc = unsafe { libc::poll(fds.as_mut_ptr(), 1, WAKE_POLL_TIMEOUT_MS) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(rc > 0 && fds[0].revents & libc::POLLIN != 0)
}
