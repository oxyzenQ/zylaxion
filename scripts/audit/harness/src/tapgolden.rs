// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-frontier-Z5: the tap-behavior golden corpus — the kernel side
//! pinned as bytes.
//!
//! Entry 40 pins the userspace render ("the sound IS the bytes"); this
//! mode pins the OTHER half of the pipeline: the production tap
//! object's observable kernel-side behavior, driven through the same
//! lane PROOF 7 rides (aya-obj parse, fresh maps from the parsed
//! definitions, the loader's own map-fd relocation, a raw bpf() load
//! as BPF_PROG_TYPE_SYSCALL, one BPF_PROG_TEST_RUN per synthetic
//! event) — and drained through the generic ringbuf consumer lane.
//!
//! The golden expectations are COMPILED constants: they are the tap's
//! own semantics read off `ebpf/src/main.rs` (the gate, the keymap
//! death, the autorepeat filter, the stats-before-reserve ordering,
//! the drop-on-full posture), not a machine-fingerprint. The ring's
//! saturation capacity is MEASURED in-run, not pinned: the exact
//! usable-byte arithmetic is the kernel's own property (the first
//! guest leg caught the derived constant guessing it wrong — 1792
//! pinned, 2047 measured, against a 2048 byte ceiling), while the
//! CONTRACT the lane pins is the behavior around it: the flood fills
//! the ring to its capacity, the overflow presses are dropped, the
//! ring recovers, and the counters count every press either way. The
//! overflow is GUARANTEED by arithmetic, not tuning: 2203 drives at
//! 32 bytes occupancy exceed the reported 65536 bytes on any kernel.
//! A kernel or a tap change that moves one record tuple or one stats
//! delta fails this lane with the delta named; a capacity move lands
//! as the named measured row.
//!
//! The golden contract, phase by phase (KEY = 30, KEY2 = 31, both
//! keymapped to slot 0):
//!
//!   A gate-off    4 presses of KEY        -> 0 records, 0 stats
//!   B gate on
//!   C unmapped    2 presses of 272 (BTN)  -> 0 records (keymap death)
//!   D press       1x KEY                  -> 1 record (30,1,0); stats 1
//!   E release     1x KEY                  -> 1 record (30,0,0); stats 0
//!   F autorepeat  3x value=2              -> 0 records (kernel filter)
//!   G burst       10x press of KEY        -> 10 records; stats 10
//!   DRAIN 1                               -> exactly 12 records in order
//!   H flood       2200 presses of KEY     -> the ring fills; the rest drop
//!   I post-flood  1 press of KEY          -> dropped (ring full)
//!   J second key  2 presses of KEY2       -> dropped (ring full)
//!   DRAIN 2                               -> exactly CAPACITY records
//!                                         (capacity MEASURED in-run,
//!                                         <= the 2048 byte ceiling)
//!   K post-drain  1 press of KEY2         -> 1 record (31,1,0)
//!   DRAIN 3                               -> exactly 1 record
//!   FINAL stats   the counter law: stats count EVERY press (the
//!                 fetch-add rides BEFORE the ring reservation), so
//!                 dropped events still count — the drop degrades the
//!                 sound, never the telemetry.
//!
//! The record's `ts_us` field is the kernel clock (bpf_ktime_get_ns),
//! not behavior: the comparison masks it and pins (keycode, value,
//! slot) in order. The record shape mirrors the tap's own
//! `KeyRingEvent` (24 bytes: u64 ts + 3x u32, repr(C) padded) — the
//! harness is a separate crate and mirrors the contract the same way
//! the pin paths are mirrored; a shape drift shows up as a named
//! mismatch, not a silent pass.

use crate::progrun::{
    map_lookup, map_update, prepare_tap, prog_load, prog_test_run, pt_regs, PreparedTap,
    BPF_PROG_TYPE_SYSCALL,
};
use crate::ringdrain::{drain_ring_records, ringbuf_data_size};

/// The keymapped keys (slot 0 — every shipped preset sounds one class).
const KEY: u32 = 30;
const KEY2: u32 = 31;
const SLOT: u32 = 0;
/// BTN_MOUSE (0x110): the unmapped class the keymap death kills.
const BTN_UNMAPPED: u32 = 272;
/// EV_KEY from linux/input-event-codes.h.
const EV_KEY: u32 = 0x01;
/// 1 = press, 0 = release, 2 = autorepeat (the kernel filter's lane).
const VALUE_PRESS: u32 = 1;
const VALUE_RELEASE: u32 = 0;
const VALUE_AUTOREPEAT: u32 = 2;

// The compiled phase counts (see the module contract above).
const GATE_OFF_PRESSES: u32 = 4;
const UNMAPPED_PRESSES: u32 = 2;
const AUTOREPEATS: u32 = 3;
const BURST_PRESSES: u32 = 10;
const FLOOD_PRESSES: u32 = 2200;
const SECOND_KEY_DROPS: u32 = 2;
const POST_DRAIN_PRESSES: u32 = 1;

/// The tap's ring record: 24 bytes (u64 ts_us masked in comparison,
/// then keycode, value, slot).
const RECORD_SIZE: usize = 24;
/// `BPF_RINGBUF_HDR_SZ` (kernel/bpf/ringbuf.c): the 8-byte record
/// header every reservation consumes beside the payload.
const RINGBUF_HDR: usize = 8;

/// One expected record tuple (ts masked away).
type Rec = (u32, u32, u32);

/// The drained record tuple: (keycode, value, slot) — ts_us masked.
fn decode_record(bytes: &[u8]) -> Rec {
    let keycode = u32::from_le_bytes(bytes[8..12].try_into().expect("record is 24 bytes"));
    let value = u32::from_le_bytes(bytes[12..16].try_into().expect("record is 24 bytes"));
    let slot = u32::from_le_bytes(bytes[16..20].try_into().expect("record is 24 bytes"));
    (keycode, value, slot)
}

/// The full golden phase plan, executed against one fresh tap + maps.
pub(super) fn run_tap_golden(object_path: &str) {
    // SAFETY: geteuid is a plain libc query with no side effects and
    // no pointer arguments.
    if unsafe { libc::geteuid() } != 0 {
        println!(
            "TAP GOLDEN: SKIP — the lane needs root (CAP_BPF/CAP_PERFMON for the load); \
             it runs inside the e2e micro-VM guest beside PROOF 7"
        );
        println!("MASS-RESULT: PROOF 9 tap-golden lane — SKIP (recorded, not faked)");
        std::process::exit(0);
    }

    let PreparedTap {
        insns,
        license,
        fds,
    } = prepare_tap(object_path, "TAP GOLDEN");
    let control = *fds.get("control").expect("the tap carries the control map");
    let keymap = *fds.get("keymap").expect("the tap carries the keymap");
    let stats_fd = *fds.get("stats").expect("the tap carries the stats map");
    let events = *fds.get("events").expect("the tap carries the events ring");

    let data_size = match ringbuf_data_size(events) {
        Ok(n) => n,
        Err(e) => fail(format!("the events ring's size did not read back: {e}")),
    };
    // The byte ceiling the ring can never exceed: every record costs
    // round_up(len, 8) + the 8-byte header, so a kernel reporting
    // data_size bytes holds at most data_size / occupancy records.
    // The exact usable area inside that ceiling is the kernel's own
    // math — the saturation phase MEASURES it in-run and asserts the
    // behavior laws around it.
    let occupancy = RECORD_SIZE.next_multiple_of(8) + RINGBUF_HDR;
    let byte_ceiling = data_size / occupancy;

    // Keymap seeds: both keys -> slot 0 (the compiled preset class).
    for key in [KEY, KEY2] {
        map_update(keymap, &key.to_ne_bytes(), &SLOT.to_ne_bytes())
            .unwrap_or_else(|e| fail(format!("the keymap seed refused: {e}")));
    }

    // The program loads as the syscall type — the same bytes, the one
    // type whose TEST_RUN every at-floor kernel answers (the PROOF 7
    // law; a KPROBE program answers ENOTSUPP to a test run).
    let attr_dump = &mut [0u8; 44];
    let prog_fd = match prog_load(BPF_PROG_TYPE_SYSCALL, &insns, &license, attr_dump) {
        Ok(fd) => fd,
        Err((why, log)) => fail(format!(
            "the tap body refused the load: {why}{}",
            log.unwrap_or_default()
        )),
    };
    println!(
        "TAP GOLDEN: the production tap body is loaded unattached ({} instructions), \
         ring data {} bytes, capacity measured in-run (byte ceiling {} records)",
        insns.len(),
        data_size,
        byte_ceiling
    );

    let set_gate = |enabled: u32| {
        map_update(control, &0u32.to_ne_bytes(), &enabled.to_ne_bytes())
            .unwrap_or_else(|e| fail(format!("the gate write refused: {e}")));
    };
    let drive = |kind: &str, type_: u32, code: u32, value: u32| {
        prog_test_run(prog_fd, &pt_regs(type_, code, value))
            .unwrap_or_else(|e| fail(format!("the {kind} test run refused: {e}")));
    };
    let drain = |tag: &str| -> Vec<Rec> {
        match drain_ring_records(events, data_size, RECORD_SIZE) {
            Ok(records) => records.iter().map(|r| decode_record(&r.bytes)).collect(),
            Err(e) => fail(format!("the {tag} drain failed: {e}")),
        }
    };
    let stats = |key: u32| -> u64 {
        let mut value = [0u8; 8];
        map_lookup(stats_fd, &key.to_ne_bytes(), &mut value)
            .unwrap_or_else(|e| fail(format!("the stats lookup refused: {e}")));
        u64::from_ne_bytes(value)
    };

    let mut failures: Vec<String> = Vec::new();
    let expect_stats = |failures: &mut Vec<String>, tag: &str, key: u32, want: u64| {
        let got = stats(key);
        if got != want {
            failures.push(format!("stats[{key}] after {tag}: got {got}, golden pins {want}"));
        }
    };
    let expect_records = |failures: &mut Vec<String>, tag: &str, got: &[Rec], want: &[Rec]| {
        if got.len() != want.len() {
            failures.push(format!(
                "{tag}: got {} records, golden pins {}",
                got.len(),
                want.len()
            ));
            return;
        }
        for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
            if g != w {
                failures.push(format!("{tag}: record {i} got {g:?}, golden pins {w:?}"));
            }
        }
    };

    // ── A: the gate is the whole cost — dead silence, dead counters ──
    set_gate(0);
    for _ in 0..GATE_OFF_PRESSES {
        drive("gate-off press", EV_KEY, KEY, VALUE_PRESS);
    }
    expect_stats(&mut failures, "the gate-off phase", STATS_TOTAL_SLOT, 0);
    expect_records(&mut failures, "gate-off drain", &drain("gate-off"), &[]);

    // ── B..G: the live prefix — every filter and every record shape ──
    set_gate(1);
    for _ in 0..UNMAPPED_PRESSES {
        drive("unmapped press", EV_KEY, BTN_UNMAPPED, VALUE_PRESS);
    }
    drive("press", EV_KEY, KEY, VALUE_PRESS);
    drive("release", EV_KEY, KEY, VALUE_RELEASE);
    for _ in 0..AUTOREPEATS {
        drive("autorepeat", EV_KEY, KEY, VALUE_AUTOREPEAT);
    }
    for _ in 0..BURST_PRESSES {
        drive("burst press", EV_KEY, KEY, VALUE_PRESS);
    }
    let prefix = [
        vec![(KEY, VALUE_PRESS, SLOT)],
        vec![(KEY, VALUE_RELEASE, SLOT)],
        vec![(KEY, VALUE_PRESS, SLOT); BURST_PRESSES as usize],
    ]
    .concat();
    let drained: Vec<Rec> = drain("prefix");
    expect_records(&mut failures, "prefix drain", &drained, &prefix);
    // The counter law's prefix: the phase-D press (1) plus the burst
    // (10) — the first guest leg caught the pin forgetting the D press
    // (got 11, pinned 10: the tap was right, the golden constant was
    // not).
    let presses_so_far = 1 + BURST_PRESSES;
    expect_stats(&mut failures, "the prefix", KEY, presses_so_far as u64);
    expect_stats(&mut failures, "the prefix", STATS_TOTAL_SLOT, presses_so_far as u64);

    // ── H..J: the saturation — capacity accepted, drops leave no
    //    residue in the ring, and the counters count the drops anyway
    for _ in 0..FLOOD_PRESSES {
        drive("flood press", EV_KEY, KEY, VALUE_PRESS);
    }
    drive("post-flood press", EV_KEY, KEY, VALUE_PRESS);
    for _ in 0..SECOND_KEY_DROPS {
        drive("second-key drop press", EV_KEY, KEY2, VALUE_PRESS);
    }
    let saturated: Vec<Rec> = drain("saturation");
    // The measured capacity: whatever the ring accepted before it
    // started dropping. The behavior laws, not the number, are the
    // contract: the flood DID overflow (a drop happened — guaranteed
    // by arithmetic, 2203 x 32 bytes > 65536 reported), the overflow
    // presses left no ring residue, and the ring never exceeded its
    // own byte ceiling.
    let capacity = saturated.len();
    if capacity == 0 || capacity > byte_ceiling {
        failures.push(format!(
            "the measured capacity {capacity} violates the byte ceiling {byte_ceiling}"
        ));
    }
    let presses_into_saturation = FLOOD_PRESSES + 1 + SECOND_KEY_DROPS;
    if capacity as u32 >= presses_into_saturation {
        failures.push(format!(
            "the flood did not overflow: capacity {capacity} accepted all \
             {presses_into_saturation} saturation presses — the drop law is untestable"
        ));
    }
    let want_saturated = vec![(KEY, VALUE_PRESS, SLOT); capacity];
    expect_records(&mut failures, "saturation drain", &saturated, &want_saturated);
    let presses_after_flood = presses_so_far + FLOOD_PRESSES + 1 + SECOND_KEY_DROPS;
    expect_stats(&mut failures, "the saturation", KEY, (presses_after_flood - SECOND_KEY_DROPS) as u64);
    expect_stats(&mut failures, "the saturation", KEY2, SECOND_KEY_DROPS as u64);
    expect_stats(&mut failures, "the saturation", STATS_TOTAL_SLOT, presses_after_flood as u64);

    // ── K: the ring recovered — one record, exactly the tuple ──
    for _ in 0..POST_DRAIN_PRESSES {
        drive("post-drain press", EV_KEY, KEY2, VALUE_PRESS);
    }
    let recovered = drain("post-drain");
    expect_records(
        &mut failures,
        "post-drain drain",
        &recovered,
        &[(KEY2, VALUE_PRESS, SLOT)],
    );
    let presses_total = presses_after_flood + POST_DRAIN_PRESSES;
    expect_stats(&mut failures, "the recovery", KEY2, (SECOND_KEY_DROPS + POST_DRAIN_PRESSES) as u64);
    expect_stats(&mut failures, "the recovery", STATS_TOTAL_SLOT, presses_total as u64);
    set_gate(0);

    // ── The verdict ──
    let pinned_records = prefix.len() + capacity + POST_DRAIN_PRESSES as usize;
    println!(
        "TAP GOLDEN VERDICT: phases=7 records={}/{} capacity={} (measured, byte ceiling {}) stats={}",
        pinned_records - failures.iter().filter(|f| f.contains("records") || f.contains("record ")).count(),
        pinned_records,
        capacity,
        byte_ceiling,
        if failures.is_empty() { "match" } else { "MISMATCH" }
    );
    if !failures.is_empty() {
        for why in &failures {
            println!("  MISMATCH  {why}");
        }
        println!(
            "TAP GOLDEN FAIL: the kernel-side behavior moved — every delta is named \
             above; a re-pin needs the documented-cause law (the why in the ledger)"
        );
        println!("MASS-RESULT: PROOF 9 tap-golden lane — FAIL");
        std::process::exit(1);
    }
    println!(
        "TAP GOLDEN PASS: gate-off silence, the keymap death, the autorepeat filter, \
         the record order, the fill-until-full and drop-after posture (capacity {} \
         measured against the {} byte ceiling), and the count-the-drops counter \
         law all hold — the kernel side IS the bytes",
        capacity,
        data_size
    );
    println!("MASS-RESULT: PROOF 9 tap-golden lane — PASS");
    std::process::exit(0);
}

/// The stats slot that counts every press (the tap's STATS_TOTAL).
const STATS_TOTAL_SLOT: u32 = 256;

fn fail(why: String) -> ! {
    println!("TAP GOLDEN: FAIL — {why}");
    println!("MASS-RESULT: PROOF 9 tap-golden lane — FAIL");
    std::process::exit(1);
}
