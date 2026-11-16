// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The internal player (`zylaxion _play`, night-improve-23).
//!
//! The ONE process that exists while sound plays — and the only one,
//! stateless, owning nothing:
//!
//! - ALL state comes from the pinned BPF maps (preset, volume, enabled,
//!   player pid) — there is no config, no pidfile, no socket, nothing to
//!   supervise.
//! - The input arrives from the kernel-side tap through the pinned
//!   events ring buffer: the bridge blocks in poll(2) with a 100 ms
//!   cap, and the render loop blocks on its channel (1 ms while
//!   voices render, 100 ms while silent — the idle-CPU audit's
//!   adaptive wait). No busy-wait anywhere; `ps` reads 0.0 percent
//!   idle CPU.
//! - The control plane is watched by one thread that polls the
//!   notice ring and the wake pipe (night-frontier-Z2 B1: the
//!   evented control wake — the writer drives the pinned relay, the
//!   kernel produces the record, the watcher wakes in microseconds)
//!   with a 10 s liveness heartbeat underneath, and re-reads the
//!   control map on EVERY wake — enabled=0
//!   stops the player (the CLI's `off`/`exit` writes exactly that),
//!   preset and volume changes apply live through the ArcSwap model
//!   swap and the pool's atomic volume cell. night-hardening-2 adds
//!   the fast path: the verbs raise a zero-payload SIGUSR1 after the
//!   map write, the watcher's poll wakes instantly, and the map —
//!   still the only authority — is re-read at once instead of at the
//!   next heartbeat.
//!
//! Crash posture: cattle, not pet. A dead player loses nothing — the
//! next `zylaxion on` sees the stale pid in the control map, replants
//! itself, and keeps playing. There is nothing to supervise because
//! there is nothing to inherit.

use std::process;

use std::os::fd::{AsFd as _, BorrowedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use aya::maps::MapData;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

use crate::core::Orchestrator;
use crate::ebpf::loader::KeyRingEvent;
use crate::ebpf::pin;
use crate::profiles::MechanicalClick;

/// The control watcher's poll cadence: the worst-case latency of an
/// `off`, a preset switch, or a volume change. 100 ms is invisible to
/// a human and costs ~10 Hz of tiny map reads.
/// The bridge's poll cap: the render-side bounded wait — the shutdown
/// latency law and the idle-CPU audit's cadence. The CONTROL plane no
/// longer rides this number (night-frontier-Z2 B1 demoted it from
/// latency path to liveness heartbeat; see CONTROL_HEARTBEAT_MS).
const BRIDGE_POLL_MS: u16 = 100;

/// night-frontier-Z2 B1: the control watcher's wait is now ONE poll
/// over [the notice ring, the wake pipe], and this value is its
/// liveness heartbeat — the worst-case latency ONLY when every wake
/// lane is dead (no ring, no signal). The evented wakes answer in
/// microseconds; this number demoted from the latency path (the old
/// 100 ms CONTROL_POLL_MS) to a floor nobody should ever feel.
const CONTROL_HEARTBEAT_MS: u16 = 10_000;

/// night-dragon-lts: the read-failure patience. The LAW is unchanged
/// — 10 s of consecutive failures mean the pinned map is genuinely
/// gone and the watcher self-terminates — but the accounting moves
/// from a read count to a clock: the old 100-read count was a proxy
/// for 10 s at the 100 ms cadence, and that cadence is gone. At the
/// 10 s heartbeat (and the wake-driven re-reads in between), the
/// window is measured, not counted — the same patience, honestly
/// derived (entry 24's law survives; the proxy retires).
const WATCHER_READ_FAILURE_GRACE: Duration = Duration::from_secs(10);

/// The percent-of-default gain law: the control map carries a percent,
/// the pool wants a linear gain. 100 percent means the compiled laptop
/// default (5.5x); 0 percent is silence.
fn gain_from_percent(percent: u32) -> f32 {
    percent as f32 / 100.0 * crate::engine::VoicePool::DEFAULT_MASTER_VOLUME
}

/// CLOCK_MONOTONIC nanoseconds — the same epoch `bpf_ktime_get_ns`
/// stamps, so the notice wake latency is `now - record.ns` with no
/// clock sync between processes.
fn now_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid timespec pointer; the call cannot fail for
    // CLOCK_MONOTONIC.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    (ts.tv_sec as u64) * 1_000_000_000 + ts.tv_nsec as u64
}

/// Look up a preset profile by its registry index. Out-of-range indexes
/// (a registry that changed between binaries) fall back to the compiled
/// default — the alternative (refusing to play) strands the user on a
/// number the kernel map happened to carry.
fn preset_by_index(index: u32) -> crate::profiles::KeyProfile {
    let all = crate::profiles::registry_all();
    match all.get(index as usize) {
        Some(entry) => entry.profile,
        None => {
            log::warn!(
                "control map preset index {index} is out of range (registry holds {}) — \
                 using the compiled default",
                all.len()
            );
            crate::profiles::registry_lookup(crate::profiles::DEFAULT_PRESET)
                .map(|e| e.profile)
                .unwrap_or_else(|| all[0].profile)
        }
    }
}

/// Open the pinned control map twice-shaped: one owned Map per handle.
/// Main thread and watcher each get their own fd — no shared borrow, no
/// Clone requirement, the same map underneath.
fn open_control_array() -> Result<aya::maps::Array<MapData, u32>, String> {
    let data = MapData::from_pin(pin::PIN_MAP_CONTROL).map_err(|e| {
        format!(
            "cannot open the pinned control map (is the tap attached? `sudo zylaxion attach`): {e}"
        )
    })?;
    let owned = aya::maps::Map::Array(data);
    aya::maps::Array::try_from(owned)
        .map_err(|e| format!("the pinned control map is not an array: {e}"))
}

/// Open the pinned notice ring (night-frontier-Z2 B1) for consuming.
/// The watcher's degradation posture: an unopenable ring logs a
/// warning and the heartbeat alone remains fully correct (the same
/// law the wake pipe obeys — the fast path is an optimization over a
/// correct poller, never a correctness dependency).
fn open_notices_ring() -> Result<aya::maps::RingBuf<MapData>, String> {
    let data = MapData::from_pin(pin::PIN_MAP_NOTICES)
        .map_err(|e| format!("cannot open the pinned notice ring: {e}"))?;
    let owned = aya::maps::Map::RingBuf(data);
    aya::maps::RingBuf::try_from(owned)
        .map_err(|e| format!("the pinned notices map is not a ring buffer: {e}"))
}

/// Open the pinned events ring buffer for consuming.
fn open_events_ring() -> Result<aya::maps::RingBuf<MapData>, String> {
    let data = MapData::from_pin(pin::PIN_MAP_EVENTS)
        .map_err(|e| format!("cannot open the pinned events ring buffer: {e}"))?;
    let owned = aya::maps::Map::RingBuf(data);
    aya::maps::RingBuf::try_from(owned)
        .map_err(|e| format!("the pinned events map is not a ring buffer: {e}"))
}

/// Run the internal player. Exits when the control map's enabled slot
/// clears (the CLI's off/exit), a termination signal arrives, or the
/// audio/input machinery fails.
pub fn cmd_play() {
    // The privilege verdict (night-improve-16's audit): a root player
    // opens audio in root's session, which no user desktop shares.
    if let Some(refusal) =
        crate::session::privilege::session_refusal(crate::session::privilege::euid_is_root())
    {
        crate::output::error(refusal);
        process::exit(1);
    }

    init_logger();

    // ── The core-dump posture (night-private-research-2xblade-3.6X,
    //    the hardening the 3.6 audit pre-named): a core dump of this
    //    process is the one recovery carrier for a bounded recent
    //    keystroke window (the ring pages, the event channel, the
    //    active voices' scancode fields), so the player drops
    //    RLIMIT_CORE to zero — soft AND hard — before anything that
    //    can hold a scancode exists. Zeroing the hard limit too means
    //    even this process cannot re-raise it later without
    //    CAP_SYS_RESOURCE. Best-effort by contract: a failure is
    //    logged, never fatal. Honest scope, named in
    //    docs/ZACTRIX_SECURITY.md: a piped core_pattern spawns its
    //    helper regardless of the limit — the operator's policy
    //    governs there; this closes the file-backed dump path, the
    //    one the kernel enforces from the limit itself.
    force_no_coredump();

    // The kernel-policy gate (night-badas-4 round 5): the player's
    // whole registration ride is control-map writes — a kernel that
    // gates unprivileged bpf() ends the run here, classified, instead
    // of an EPERM surfacing as a cryptic map error.
    if let Some(refusal) = crate::session::kernel::probe()
        .unpriv_map_write_refusal(crate::session::privilege::euid_is_root())
    {
        crate::output::error(refusal);
        process::exit(1);
    }

    // ── Open the pinned control plane (unprivileged: the pins were
    //    chowned to this user at attach time).
    let mut control = match open_control_array() {
        Ok(c) => c,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };

    // ── Single-instance: a live player registered in the control map
    //    means zylaxion is already playing. A dead pid (crashed player)
    //    is taken over — cattle, not pet.
    match control.get(&PLAYER_PID_SLOT, 0) {
        Ok(0) => {}
        Ok(live_pid) => match crate::session::orphan::pid_alive(live_pid as i32) {
            true => {
                crate::output::error(format!(
                    "zylaxion is already playing (player pid {live_pid}) — stop it with `zylaxion off`"
                ));
                process::exit(1);
            }
            false => {
                log::info!("stale player pid {live_pid} in the control map — taking over");
            }
        },
        Err(e) => {
            crate::output::error(format!("cannot read the control map: {e}"));
            process::exit(1);
        }
    }

    // ── The control wakeup (night-hardening-2): the pipe and the
    //    SIGUSR1 handler go live BEFORE the pid registration, so a
    //    verb racing this startup window hits the handler, never
    //    SIGUSR1's default terminate. Best-effort: a failure degrades
    //    to the heartbeat alone, which is fully correct on its own.
    let control_wake = match crate::session::wake::install() {
        Ok(w) => Some(w),
        Err(e) => {
            log::warn!("control wakeup lane unavailable ({e}) — the 100 ms heartbeat remains");
            None
        }
    };

    // ── Register self: starting.
    let my_pid = nix::unistd::getpid().as_raw();
    if let Err(e) = control.set(PLAYER_PID_SLOT, &(my_pid as u32), 0) {
        crate::output::error(format!("cannot register the player pid: {e}"));
        process::exit(1);
    }
    if let Err(e) = control.set(STATE_SLOT, &STATE_STARTING, 0) {
        log::warn!("cannot set the starting state: {e}");
    }

    // ── Resolve the initial preset and volume from the control map.
    let preset_index = control.get(&PRESET_ID_SLOT, 0).unwrap_or_else(|e| {
        log::warn!("cannot read the preset slot ({e}) — using 0");
        0
    });
    let volume_percent = control.get(&VOLUME_PCT_SLOT, 0).unwrap_or(100);
    let profile = preset_by_index(preset_index);
    let gain = gain_from_percent(volume_percent);
    log::info!(
        "player starting (pid {my_pid}, preset index {preset_index}, volume {volume_percent}%)"
    );

    // ── Open the pinned events ring buffer.
    let mut events = match open_events_ring() {
        Ok(r) => r,
        Err(e) => {
            fail(&mut control, format!("player cannot start: {e}"));
        }
    };

    // ── Build the orchestrator (audio + engine).
    let mut orchestrator = match Orchestrator::with_master_volume(gain) {
        Ok(o) => o,
        Err(e) => {
            fail(
                &mut control,
                format!("audio error: {e} — make sure PipeWire or PulseAudio is running"),
            );
        }
    };
    let sample_rate = orchestrator.sample_rate();
    log::info!("audio device sample rate: {sample_rate} Hz");

    // The volume factor handle: the watcher thread retunes the pool's
    // atomic cell while the render loop owns the orchestrator mutably.
    let volume_factor = orchestrator.volume_factor();

    // ── The model swap point (the same ArcSwap shape the daemon used).
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_profile(profile, sample_rate),
    ));

    // ── The event bridge: kernel ring buffer → the channel the
    //    orchestrator's loop consumes. The exact shape the old libinput
    //    thread had, minus libinput: the events come from the kernel
    //    tap, and the thread's only IO is the ring buffer fd.
    let (tx, rx) = crossbeam_channel::unbounded();
    let bridge_stop = Arc::new(AtomicBool::new(false));
    let bridge = {
        let bridge_stop = Arc::clone(&bridge_stop);
        std::thread::Builder::new()
            .name("zylaxion-bridge".to_string())
            .spawn(move || {
                use std::os::fd::AsFd as _;
                while !bridge_stop.load(Ordering::Relaxed) {
                    // Drain everything available first. A short record
                    // is skipped, never failed — the tap stays truthful.
                    while let Some(item) = events.next() {
                        if item.len() < std::mem::size_of::<KeyRingEvent>() {
                            continue;
                        }
                        // night-dragon-lts: aya hands out 8-byte-aligned
                        // ring records (the ring buf consumer pages are
                        // aligned); the cast below relies on it. The
                        // length check above guards the size half of the
                        // contract — this asserts the alignment half in
                        // debug builds and compiles out on the release
                        // hot path.
                        debug_assert!(
                            (item.as_ptr() as usize)
                                .is_multiple_of(std::mem::align_of::<KeyRingEvent>())
                        );
                        let raw = unsafe { &*(item.as_ptr() as *const KeyRingEvent) };
                        let event = crate::core::KeyEvent {
                            scancode: raw.keycode,
                            pressed: raw.value == 1,
                            timestamp: raw.ts_us,
                        };
                        if tx.send(event).is_err() {
                            // The orchestrator loop is gone — shut down.
                            return;
                        }
                    }
                    // Block on the ring buffer fd until the tap rings
                    // again or the 100 ms timeout fires (the watcher's
                    // cadence — the bridge needs no control access).
                    let fd = events.as_fd();
                    let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
                    match poll(&mut fds, PollTimeout::from(BRIDGE_POLL_MS)) {
                        Ok(_) => {}
                        Err(nix::errno::Errno::EINTR) => {}
                        Err(e) => {
                            log::warn!("ring buffer poll error: {e}");
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
            })
            .expect("failed to spawn the bridge thread")
    };

    // ── The control-plane watcher: the evented wake (night-frontier-Z2
    //    B1) — one poll over [the notice ring, the wake pipe], the 10 s
    //    heartbeat underneath, and a control-map re-read (the ONLY
    //    authority) on every wake. enabled=0 stops (the CLI's off/exit
    //    is one map write plus one notice); preset and volume changes
    //    apply live.
    let stop_flag = Arc::new(AtomicBool::new(false));
    let watcher =
        {
            let stop_flag = Arc::clone(&stop_flag);
            let model = Arc::clone(&model);
            let watch_control = match open_control_array() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("watcher cannot open the control map: {e}");
                    stop_flag.store(true, Ordering::Relaxed);
                    return;
                }
            };
            let mut notices = match open_notices_ring() {
                Ok(ring) => Some(ring),
                Err(e) => {
                    log::warn!(
                        "the notice ring did not open — the watcher rides the wake pipe and the \
                     heartbeat alone (the B1 degradation posture): {e}"
                    );
                    None
                }
            };
            std::thread::Builder::new()
            .name("zylaxion-watch".to_string())
            .spawn(move || {
                let mut last_preset = preset_index;
                let mut last_volume = volume_percent;
                // night-dragon-lts: a failed map read is UNKNOWN state,
                // not "disabled" — one transient lookup error used to
                // end the session because the error was folded into
                // enabled=0. The two are now distinct: a successful read
                // resets the budget, a failed read spends it, and only a
                // successful read of a cleared flag stops the player.
                // The spent budget (10 s of consecutive failures) is the
                // "the pinned map is genuinely gone" lane — the honest
                // self-termination beats an immortal orphan spinning on
                // a dead fd.
                let mut first_read_failure: Option<Instant> = None;
                let mut notices_drained: u32 = 0;
                let mut applied_something = false;
                loop {
                    // The wait (night-frontier-Z2 B1): ONE poll over
                    // [the notice ring, the wake pipe] with the 10 s
                    // heartbeat timeout as the liveness floor. The
                    // notice ring carries the evented control wake (a
                    // verb drives the pinned relay, the KERNEL produces
                    // the record — no signal permission, no pid
                    // targeting, an auditable trail); the wake pipe
                    // stays the session fast path. Either lane — or the
                    // heartbeat itself — lands in the map re-read that
                    // follows, which remains the only authority.
                    let mut fds: Vec<PollFd> = Vec::new();
                    let notice_fd = notices.as_ref().map(|ring| ring.as_fd());
                    if let Some(fd) = notice_fd {
                        fds.push(PollFd::new(fd, PollFlags::POLLIN));
                    }
                    if let Some(wake) = &control_wake {
                        fds.push(PollFd::new(
                            // SAFETY: the read end is owned by the
                            // ControlWake for the thread's whole
                            // lifetime; the borrow spans the poll call.
                            unsafe { BorrowedFd::borrow_raw(wake.read_fd()) },
                            PollFlags::POLLIN,
                        ));
                    }
                    match poll(&mut fds, PollTimeout::from(CONTROL_HEARTBEAT_MS)) {
                        Ok(_) => {}
                        Err(nix::errno::Errno::EINTR) => {}
                        Err(e) => {
                            log::warn!("watcher poll error: {e}");
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                    if stop_flag.load(Ordering::Relaxed) {
                        return;
                    }
                    // The pipe's readable slot drains its coalesced
                    // wake bytes (night-hardening-2's law, kept: the
                    // byte count never accumulates toward a full-pipe
                    // handler write). Whether this slot, the ring, or
                    // the heartbeat answered — the map re-read below
                    // is the next step either way.
                    if let Some(wake) = &control_wake
                        && let Some(slot) = fds.last()
                        && slot.revents().is_some_and(|f| f.intersects(PollFlags::POLLIN))
                    {
                        wake.drain();
                    }
                    // Drain the notice ring: every record is a
                    // kernel-timestamped wake hint, and the row names
                    // how long the hint waited — the evented plane's
                    // own evidence line. The record's alignment is the
                    // ringbuf contract (8-aligned), asserted like the
                    // events ring's.
                    if let Some(ring) = notices.as_mut() {
                        while let Some(item) = ring.next() {
                            if item.len() < crate::ebpf::notice::NOTICE_RECORD_SIZE {
                                continue;
                            }
                            debug_assert!((item.as_ptr() as usize).is_multiple_of(
                                std::mem::align_of::<crate::ebpf::notice::ControlNotice>()
                            ));
                            let raw = unsafe {
                                &*(item.as_ptr() as *const crate::ebpf::notice::ControlNotice)
                            };
                            notices_drained += 1;
                            let wake_ns = now_ns().saturating_sub(raw.ns);
                            log::info!(
                                "control notice seq {} kind {} value {} — wake {} ns",
                                raw.seq,
                                raw.kind,
                                raw.value,
                                wake_ns
                            );
                        }
                    }
                    // The authority re-read (night-dragon-lts): a failed
                    // map read is UNKNOWN state, not "disabled" — and
                    // the failure patience is now a CLOCK, not a count
                    // (the 100-read count was a proxy for 10 s at the
                    // dead cadence; the law survives, the proxy
                    // retires). A successful read resets the window; a
                    // cleared flag stops the player.
                    match watch_control.get(&ENABLED_SLOT, 0) {
                        Ok(1) => first_read_failure = None,
                        Ok(_) => {
                            log::info!("control map cleared the enabled flag — stopping");
                            stop_flag.store(true, Ordering::Relaxed);
                            return;
                        }
                        Err(e) => match first_read_failure {
                            None => {
                                first_read_failure = Some(Instant::now());
                                log::warn!(
                                    "control map read failed: {e} — the failure window opens \
                                     (10 s of patience, the night-dragon-lts law)"
                                );
                            }
                            Some(first) => {
                                let into = first.elapsed();
                                if into >= WATCHER_READ_FAILURE_GRACE {
                                    log::error!(
                                        "control map unreadable for {into:?} — the pinned map \
                                         is gone; stopping"
                                    );
                                    stop_flag.store(true, Ordering::Relaxed);
                                    return;
                                }
                                log::warn!(
                                    "control map read failed again ({into:?} into the window): {e}"
                                );
                            }
                        },
                    }
                    // Preset switch: rebuild the model and swap it in —
                    // running voices finish on the old model, new
                    // keypresses pick the new one up immediately.
                    let preset = watch_control.get(&PRESET_ID_SLOT, 0).unwrap_or(last_preset);
                    if preset != last_preset {
                        let profile = preset_by_index(preset);
                        let swapped = MechanicalClick::with_profile(profile, sample_rate);
                        model.store(Arc::new(swapped));
                        last_preset = preset;
                        applied_something = true;
                        log::info!("preset switched to index {preset}");
                    }
                    // Volume retune: one atomic store; the next render
                    // call picks it up.
                    let volume = watch_control
                        .get(&VOLUME_PCT_SLOT, 0)
                        .unwrap_or(last_volume);
                    if volume != last_volume {
                        // night-hunter-1: the store rides the pool's own
                        // clamp law (volume_bits), not raw bits — the
                        // startup lane clamps through with_volume, so the
                        // live-retune lane must clamp identically or the
                        // same map value means two loudnesses depending on
                        // when it was written.
                        volume_factor.store(
                            crate::engine::VoicePool::volume_bits(gain_from_percent(volume)),
                            Ordering::Relaxed,
                        );
                        last_volume = volume;
                        applied_something = true;
                        log::info!("volume retuned to {volume}%");
                    }
                    // night-frontier-Z2 B1, the authority row: a wake
                    // that carried notices but changed NOTHING says so
                    // out loud — the notice is a hint, the map is the
                    // law, and a notice alone never moves the sound.
                    // This is the gimmick door, welded shut: publishing
                    // a record without a map write produces THIS row,
                    // never an apply.
                    if notices_drained > 0 && !applied_something {
                        log::info!(
                            "control wake carried {} notice(s) with no pending change — \
                             authority held",
                            notices_drained
                        );
                    }
                    notices_drained = 0;
                    applied_something = false;
                }
            })
            .expect("failed to spawn the watcher thread")
        };

    // ── Graceful shutdown on SIGTERM / SIGINT / SIGQUIT (same contract
    //    the daemon honored: fade-out, sink drop, clean state).
    if let Err(e) =
        crate::session::signals::install_graceful_shutdown_handlers(Arc::clone(&stop_flag))
    {
        log::warn!("failed to install signal handlers: {e}");
    }

    // ── Announce readiness: the state slot flips to running, and the
    //    control plane is now fully live for the CLI surfaces.
    if let Err(e) = control.set(STATE_SLOT, &STATE_RUNNING, 0) {
        log::warn!("cannot set the running state: {e}");
    }
    log::info!("ready — press any key to hear it");

    // ── Run until stopped. run() consumes the receiver; when it
    //    returns (stop flag, channel disconnect, or audio failure),
    //    unregister and exit.
    orchestrator.run(&model, &rx, Arc::clone(&stop_flag));

    // ── Unregister: the control map goes back to the no-player
    //    posture. The tap stays attached — pins survive, the next `on`
    //    is instant. The enabled gate clears too: this exit may be
    //    signal-driven (SIGTERM, Ctrl+C, terminal close — the graceful
    //    handler set) rather than gate-driven, and a gate left on with
    //    no player is the exact "status reads sound on while nothing
    //    plays" lie the off verb's comments call load-bearing. When the
    //    stop WAS gate-driven (the off verb's enabled=0 write), this
    //    write stores the same value — a harmless no-op.
    let _ = control.set(STATE_SLOT, &STATE_IDLE, 0);
    let _ = control.set(PLAYER_PID_SLOT, &0, 0);
    let _ = control.set(ENABLED_SLOT, &0, 0);
    bridge_stop.store(true, Ordering::Relaxed);
    let _ = bridge.join();
    let _ = watcher.join();
    log::info!("player stopped cleanly");
}

// ── Control slot constants ──────────────────────────────────────────────
// The userspace mirror of control_slots in ebpf/src/main.rs (and
// ebpf::loader::control). Redeclared here as private aliases so the
// player reads without a module path hop; the loader's test tree pins
// the cross-side contract.

const ENABLED_SLOT: u32 = 0;
const PRESET_ID_SLOT: u32 = 1;
const VOLUME_PCT_SLOT: u32 = 2;
const PLAYER_PID_SLOT: u32 = 3;
const STATE_SLOT: u32 = 4;
const STATE_IDLE: u32 = 0;
const STATE_STARTING: u32 = 1;
const STATE_RUNNING: u32 = 2;
/// The failed state (night-hunter-8): the value `fail` writes when the
/// player dies of a fatal error — named here because an anonymous `3`
/// in the write path and an anonymous `3` in the status reader were
/// two ways to drift.
const STATE_FAILED: u32 = 3;

// The player's liveness probe is the orphan module's zombie-aware one
// (night-hunter-1): a ZOMBIE answers the raw kill(0) probe but is a
// dead player by any honest measure — the local kill-only copy here
// is deleted so every surface shares the one tested probe.

/// A fatal player error: mark the control map FAILED, unregister, and
/// exit. The stderr message is all the user sees (the player runs
/// detached), so it carries the fix. The enabled gate clears with the
/// registration — a failed start must not leave the gate on, or status
/// reads "sound on" against a player that died before it ever ran
/// (the same law the clean-exit unregister below follows).
fn fail(control: &mut aya::maps::Array<MapData, u32>, message: String) -> ! {
    let _ = control.set(STATE_SLOT, &STATE_FAILED, 0);
    let _ = control.set(PLAYER_PID_SLOT, &0, 0);
    let _ = control.set(ENABLED_SLOT, &0, 0);
    crate::output::error(message);
    process::exit(1);
}

/// Force `RLIMIT_CORE = 0` (soft and hard) for this process — the
/// coredump hardening the security audit's Dimension 1 pre-named
/// (night-private-research-2xblade-3.6X). A core dump of the player is
/// the one recovery carrier for recent scancodes (the ring pages, the
/// event channel, the active voices); zeroing the limit takes the
/// file-backed dump path away from every crash of this process.
/// Best-effort: the only failure paths are an exotic sandbox (a
/// seccomp filter that denies setrlimit, or a hard floor already above
/// zero — lowering a hard limit is unprivileged, raising is not) —
/// logged and lived with; the player stays cattle, not pet. Scope per
/// core(5): a piped core_pattern spawns its helper regardless of the
/// limit, so that lane is the operator's policy and stays outside
/// this call's reach — docs/ZACTRIX_SECURITY.md names it.
fn force_no_coredump() {
    let zero = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // Safety: setrlimit takes a plain const pointer to the ABI's own
    // struct type; `zero` is a local of exactly that type and the call
    // neither stores the pointer nor aliases anything.
    let rc = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &zero) };
    if rc != 0 {
        log::warn!(
            "cannot force RLIMIT_CORE=0 (core dumps stay at the operator's policy): {}",
            std::io::Error::last_os_error()
        );
    }
}

/// Shared logger wiring (the night-improve-13 lane). The default
/// filter is WARN (night-hunter-6): the player is a background
/// process, and an info-level default made every state transition a
/// terminal line — the noise without a reader. RUST_LOG=info restores
/// the full story in a foreground `zylaxion _play` run, which is the
/// diagnostic lane the parent's failure messages already name.
/// night-dragon-maintenance-xenric-1: the builder itself moved to
/// `crate::output::journal` — the one door for every verb's journal
/// wiring; this surface keeps only its loudness decision.
fn init_logger() {
    crate::output::journal::init("warn");
}

#[cfg(test)]
#[path = "../../test/commands/player_tests.rs"]
mod tests;
