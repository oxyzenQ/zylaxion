// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
//
// zylaxion eBPF tap, the pure-Rust BPF source (night-improve-23;
// there is no C twin — the aya-ebpf program IS the production
// source, the zelynic lineage precedent).
//
// One program, one job: watch the kernel's input core deliver
// keyboard events and — ONLY while the sound is enabled and the key
// is mapped — publish a 16-byte record to a ring buffer the
// stateless player drains. Everything the old daemon did in
// userspace (config watching, PID files, IPC, seat capture) is
// replaced by three pinned maps and this tap:
//
//   control  (Array<u32>)   — the control plane: enabled flag,
//                             preset id, volume percent, player pid,
//                             player state. The kernel reads slot
//                             enabled; the player and the CLI
//                             read/write the rest. Introspectable
//                             with stock bpftool.
//   keymap   (HashMap<u32,u32>) — the sound intent: keycode → sound
//                             class, written by the CLI at preset
//                             load. Unmapped keys (mouse buttons,
//                             everything the preset does not sound)
//                             die HERE, in the kernel — the player
//                             is never woken for them.
//   events   (RingBuf)      — the event stream: press and release
//                             records the player drains via epoll.
//   stats    (Array<AtomicU64>) — per-keycode press counters plus a
//                             total slot, accumulated in the kernel
//                             at zero userspace cost (the typing
//                             telemetry lane).
//
// The tap is PASSIVE by construction: attaching a tracepoint program
// takes nothing away from the input path — no evdev grab, no
// /dev/input access, no udev rules, no interference with libinput or
// the compositor, and device hotplug/suspend/resume never matter
// because no device fd exists.
//
// Tracepoint: input:input_event (include/trace/events/input.h,
// stable since 5.2, present on the verified 5.15 floor). Entry
// layout (the contract this program reads):
//   [0]  u32 __data_loc dev_name   (dynamic string, unused here)
//   [4]  u32 type                  (EV_KEY = 0x01 sounds)
//   [8]  u32 code                  (the keycode)
//   [12] i32 value                 (1 press, 0 release, 2 autorepeat)
//
// Deliberate behavioral deltas versus the old libinput capture
// (documented, not accidents):
//   1. Autorepeat (value 2) never sounds. libinput suppressed
//      kernel repeats for clients too, so the audible behavior is
//      identical — the tap now enforces it in the kernel.
//   2. Mouse buttons (BTN_* keycodes, >= 0x100) never sound. The
//      keymap carries keyboard keys only; a keyboard sound tool not
//      sounding on mouse clicks is the correct posture, and the
//      filter runs in the kernel for free.
//
// Map update discipline: the stats counters ride 64-bit BPF_ATOMIC
// fetch-add (the zelynic NIGHT-improve-29 lost-update lesson — the
// BPF ISA has RMW atomics but no atomic load/store, so the counters
// are core AtomicU64 and the program performs no plain shared-field
// access on its update path). The atomic RMW instructions require
// cpu v3 bytecode (see .cargo/config.toml) — Linux 5.12+, under the
// verified 5.15 floor.
//
// Build: cd ebpf && cargo build --release

#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU64, Ordering};

use aya_ebpf::{
    helpers::bpf_ktime_get_ns,
    macros::{map, tracepoint},
    maps::{Array, HashMap, RingBuf},
    programs::TracePointContext,
};

/// EV_KEY from linux/input-event-codes.h — the only event type that
/// sounds.
const EV_KEY: u32 = 0x01;

/// Control map slots (the index contract with the userspace loader
/// — src/ebpf/loader.rs reads and writes the same slots; keep both
/// sides in lockstep).
pub mod control_slots {
    /// 1 while the sound is on. The kernel gate reads ONLY this slot.
    pub const ENABLED: u32 = 0;
    /// Registry index of the active preset (userspace-resolved).
    pub const PRESET_ID: u32 = 1;
    /// Master volume, percent of the compiled default gain.
    pub const VOLUME_PCT: u32 = 2;
    /// The player process pid (the "pidfile" lives in the kernel now).
    pub const PLAYER_PID: u32 = 3;
    /// Player lifecycle: 0 idle, 1 starting, 2 running, 3 failed.
    pub const STATE: u32 = 4;
}

/// The stats slot that counts every press (per-keycode counters ride
/// slots 0..=255).
pub const STATS_TOTAL: u32 = 256;

/// The ring buffer record — layout contract with the userspace
/// player (src/ebpf/loader.rs KeyRingEvent). 16 bytes, 8-aligned.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeyRingEvent {
    /// The evdev keycode (e.g. KEY_A = 30).
    pub keycode: u32,
    /// 1 = press, 0 = release (autorepeat is filtered in-kernel).
    pub value: u32,
    /// The keymap sound class (0 = the default class for now — the
    /// per-key tuning lane is reserved, not wired).
    pub slot: u32,
    /// CLOCK_MONOTONIC microseconds (bpf_ktime_get_ns / 1000) — the
    /// same clock and unit the old libinput path stamped.
    pub ts_us: u64,
}

// The static names ARE the userspace contract (the loader opens each
// map by name), so they stay lowercase exactly like the object's
// symbols; the allow attribute suppresses the non-upper-case globals
// lint for that reason.

#[allow(non_upper_case_globals)]
#[map]
static control: Array<u32> = Array::with_max_entries(8, 0);

#[allow(non_upper_case_globals)]
#[map]
static keymap: HashMap<u32, u32> = HashMap::with_max_entries(512, 0);

#[allow(non_upper_case_globals)]
#[map]
static events: RingBuf = RingBuf::with_byte_size(65536, 0);

#[allow(non_upper_case_globals)]
#[map]
static stats: Array<AtomicU64> = Array::with_max_entries(257, 0);

#[tracepoint]
fn input_event(ctx: TracePointContext) -> i32 {
    // The one unsafe block lives inside `tap` — see its contract note.
    // The verifier re-proves the whole body at load time.
    tap(&ctx)
}

/// The tap body. Every aya-ebpf call inside (map reads, the ktime
/// helper) is an `unsafe fn` — the unsafe block below is that API
/// contract, nothing more: every operation is bounds-checked (map
/// lookups return Option, tracepoint reads bounds-check) and the
/// verifier re-proves the whole thing at load time anyway.
fn tap(ctx: &TracePointContext) -> i32 {
    unsafe {
        // Read the tracepoint entry (layout documented in the crate
        // header). Any read failure means the record is shorter than
        // the contract — ignore the event, never fail the input path.
        let Ok(kind) = ctx.read_at::<u32>(4) else {
            return 0;
        };
        if kind != EV_KEY {
            return 0;
        }
        let Ok(code) = ctx.read_at::<u32>(8) else {
            return 0;
        };
        let Ok(value) = ctx.read_at::<i32>(12) else {
            return 0;
        };

        // Press (1) and release (0) sound; autorepeat (2) never did —
        // the libinput parity rule, now enforced in the kernel.
        if value > 1 {
            return 0;
        }

        // The kernel-side gate: with the switch off, NOTHING wakes
        // the player — no ringbuf reservation, no wakeup, no stats.
        // The whole cost is this branch.
        let Some(enabled) = control.get(control_slots::ENABLED) else {
            return 0;
        };
        if *enabled != 1 {
            return 0;
        }

        // The keymap is the sound intent: only mapped keycodes
        // produce events (keyboard keys are written at preset load;
        // mouse buttons and everything else die here).
        let Some(&slot) = keymap.get(&code) else {
            return 0;
        };

        if value == 1 {
            // Press counters — 64-bit BPF_ATOMIC fetch-add (SMP-safe;
            // the repo-local target spec enables atomic-cas at cpu
            // v3). Array::get bounds-checks, so a keycode past the
            // table is simply uncounted, never a fault.
            if let Some(counter) = stats.get(code) {
                counter.fetch_add(1, Ordering::Relaxed);
            }
            if let Some(counter) = stats.get(STATS_TOTAL) {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Publish to the ring buffer. A failed reservation (buffer
        // full — the player stalled longer than 64 KiB of keystrokes)
        // drops the event silently: a missed click sound is always
        // preferable to blocking the input path. The reservation
        // derefs to MaybeUninit — write() completes the record before
        // submit().
        if let Some(mut entry) = events.reserve::<KeyRingEvent>(0) {
            entry.write(KeyRingEvent {
                keycode: code,
                value: value as u32,
                slot,
                ts_us: bpf_ktime_get_ns() / 1000,
            });
            entry.submit(0);
        }

        0
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// The license section must stay GPL: the tap runs in the kernel's
// input path and the zylaxion family declares GPL. A mismatch here
// would fail the verifier at program load time on a real host.
#[unsafe(no_mangle)]
#[unsafe(link_section = "license")]
static LICENSE: [u8; 4] = *b"GPL\0";
