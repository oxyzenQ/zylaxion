// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
//
// zylaxion eBPF tap, the pure-Rust BPF source (night-improve-23;
// night-improve-25 restores the approved plan: a passive kernel-side
// probe on the exported input_event() function — there is no C twin,
// the aya-ebpf program IS the production source, the zelynic lineage
// precedent).
//
// One body, two attach programs, one job: watch the kernel's input
// core deliver keyboard events and — ONLY while the sound is enabled
// and the key is mapped — publish a 16-byte record to a ring buffer
// the stateless player drains. Everything the old daemon did in
// userspace (config watching, PID files, IPC, seat capture) is
// replaced by three pinned maps and this tap:
//
//   control  (Array<u32>)   — the control plane: enabled flag,
//                             preset id, volume percent, player pid,
//                             player state, tap kind. The kernel reads
//                             slot enabled; the player and the CLI
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
// The tap is PASSIVE by construction: a probe on the exported
// input_event() function takes nothing away from the input path — no
// evdev grab, no /dev/input access, no udev rules, no interference
// with libinput or the compositor, and device hotplug/suspend/resume
// never matter because no device fd exists.
//
// The two programs (the loader picks, in order — see
// src/ebpf/loader.rs, the attach chain):
//
//   fentry/input_event   BPF_PROG_TYPE_TRACING on input_event() via
//                        the BTF trampoline. Zero-overhead. Requires
//                        CONFIG_DEBUG_INFO_BTF=y and input_event
//                        present in vmlinux BTF (aya resolves the
//                        attach_btf_id at load time from
//                        /sys/kernel/btf/vmlinux).
//   kprobe/input_event   BPF_PROG_TYPE_KPROBE on the same symbol via
//                        pt_regs. Works without BTF. The same program
//                        also serves the kprobe/input_handle_event
//                        fallback (the loader passes the symbol at
//                        attach time).
//
// Both wrappers read the same four arguments through the one shim
// (src/ebpf/args.rs, included below via #[path] — single source,
// userspace-tested) and funnel into the one tap() body. The argument
// ABI is the function's own prototype, drivers/input/input.c, stable
// across the verified 5.15 floor and every kernel since:
//
//   void input_event(struct input_dev *dev, unsigned int type,
//                    unsigned int code, int value)
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
    macros::{fentry, kprobe, map},
    maps::{Array, HashMap, RingBuf},
    programs::{FEntryContext, ProbeContext},
};

// The argument shim, single-sourced: the userspace crate owns the
// file (so its tests run in every gate lane) and this crate compiles
// the very same bytes via #[path]. See src/ebpf/args.rs for the
// contract and the discipline.
#[path = "../../src/ebpf/args.rs"]
mod args;

use args::InputEventArgs;

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
    /// Which attach-chain step owns the live tap (night-improve-25):
    /// 0 fentry:input_event, 1 kprobe:input_event,
    /// 2 kprobe:input_handle_event. Written once by the loader at
    /// attach; the kernel never reads it (userspace-only slot, the
    /// same posture as PRESET_ID and VOLUME_PCT).
    pub const TAP_KIND: u32 = 5;
}

/// The stats slot that counts every press (per-keycode counters ride
/// slots 0..=255).
pub const STATS_TOTAL: u32 = 256;

/// The ring buffer record — layout contract with the userspace
/// player (src/ebpf/loader.rs KeyRingEvent). 24 bytes, 8-aligned:
/// repr(C) puts ts_us at 0 and the three u32s at 8/12/16, with four
/// trailing pad bytes — the identical shape on both sides is the
/// contract (same repr, same field order, same Rust ABI).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeyRingEvent {
    /// CLOCK_MONOTONIC microseconds (bpf_ktime_get_ns / 1000) — the
    /// same clock and unit the old libinput path stamped.
    pub ts_us: u64,
    /// The evdev keycode (e.g. KEY_A = 30).
    pub keycode: u32,
    /// 1 = press, 0 = release (autorepeat is filtered in-kernel).
    pub value: u32,
    /// The keymap sound class (0 = the default class for now — the
    /// per-key tuning lane is reserved, not wired).
    pub slot: u32,
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

/// The fentry program (the chain's primary step): the BTF trampoline
/// calls this at input_event()'s entry with the arguments exposed as
/// register-width slots. Section `fentry/input_event` — the loader
/// resolves the attach_btf_id from vmlinux BTF at load time.
#[fentry(function = "input_event")]
fn fentry_input_event(ctx: FEntryContext) -> i32 {
    // The one unsafe block lives inside `tap` — see its contract
    // note. The verifier re-proves the whole body at load time.
    tap(fentry_args(&ctx))
}

/// The kprobe program (the chain's fallback steps): the kernel calls
/// this at the probe point with a pt_regs carrying the same four
/// arguments in PARM1..PARM4. Section `kprobe/input_event` — the
/// attach symbol is a loader-side parameter, so the same program
/// also serves the kprobe:input_handle_event alternate.
#[kprobe(function = "input_event")]
fn kprobe_input_event(ctx: ProbeContext) -> i32 {
    // Extraction failure (a pt_regs with fewer than 4 argument
    // registers — impossible on every supported arch, but the read
    // is an Option) drops the event: never fail the input path.
    match kprobe_args(&ctx) {
        Some(a) => tap(a),
        None => 0,
    }
}

/// The fentry lane's extraction: the trampoline's typed argument
/// slots, read through the same shim the kprobe lane uses. The reads
/// are the aya-ebpf API contract (slot array in, value out — the
/// trampoline guarantees the slot count matches the function's
/// prototype, which the BTF that loaded this program vouches for).
fn fentry_args(ctx: &FEntryContext) -> InputEventArgs {
    InputEventArgs::from_btf_slots(ctx.arg(0), ctx.arg(1), ctx.arg(2), ctx.arg(3))
}

/// The kprobe lane's extraction: PT_REGS_PARM1..PARM4, read through
/// the same shim. `None` means the register bank was short (a
/// contract violation this program cannot recover from — drop the
/// event, return 0).
fn kprobe_args(ctx: &ProbeContext) -> Option<InputEventArgs> {
    Some(InputEventArgs::from_pt_regs(
        ctx.arg::<u64>(0)?,
        ctx.arg::<u64>(1)?,
        ctx.arg::<u64>(2)?,
        ctx.arg::<u64>(3)?,
    ))
}

/// The tap body — written once, called by both programs. Every
/// aya-ebpf call inside (map reads, the ktime helper) is an `unsafe
/// fn` — the unsafe block below is that API contract, nothing more:
/// every operation is bounds-checked (map lookups return Option) and
/// the verifier re-proves the whole thing at load time anyway.
fn tap(a: InputEventArgs) -> i32 {
    unsafe {
        // EV_KEY (0x01) is the only event type that sounds; the
        // shim already narrowed the register values to the C
        // argument widths.
        if a.kind != EV_KEY {
            return 0;
        }

        // Press (1) and release (0) sound; autorepeat (2) never did —
        // the libinput parity rule, now enforced in the kernel.
        if a.value > 1 {
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
        let Some(&slot) = keymap.get(&a.code) else {
            return 0;
        };

        if a.value == 1 {
            // Press counters — 64-bit BPF_ATOMIC fetch-add (SMP-safe;
            // the repo-local target spec enables atomic-cas at cpu
            // v3). Array::get bounds-checks, so a keycode past the
            // table is simply uncounted, never a fault.
            if let Some(counter) = stats.get(a.code) {
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
                ts_us: bpf_ktime_get_ns() / 1000,
                keycode: a.code,
                value: a.value as u32,
                slot,
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
