// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The internal player (`zylaxion _play`, night-improve-23).
//!
//! The ONE process that exists while sound plays — and the only one,
//! stateless, owning nothing:
//!
//! - ALL state comes from the pinned BPF maps (preset, volume, enabled,
//!   player pid) — there is no config, no pidfile, no socket, nothing to
//!   supervise.
//! - The input arrives from the kernel-side tap through the pinned
//!   events ring buffer: the bridge blocks in a FOREVER poll(2) on the
//!   ring fd (the wind-down pipe is its one exit lane; the 100 ms cap
//!   remains only as the exotic no-pipe fallback), and the render loop
//!   parks with no timer while silent (night-frontier-Z4: the last
//!   timer dies — the 1 ms pacing arms only while voices render). No
//!   busy-wait anywhere; `ps` reads 0.0 percent idle CPU and the
//!   scheduling threads emit zero timer wakeups between keystrokes.
//! - The control plane is watched by one thread that polls the
//!   notice ring and the wake pipe (night-frontier-Z2 B1: the
//!   evented control wake — the writer drives the pinned relay, the
//!   kernel produces the record, the watcher wakes in microseconds)
//!   as a FOREVER poll, and re-reads the control map on EVERY wake —
//!   enabled=0
//!   stops the player (the CLI's `off`/`exit` writes exactly that),
//!   preset and volume changes apply live through the ArcSwap model
//!   swap and the pool's atomic volume cell. night-hardening-2 adds
//!   the fast path: the verbs raise a zero-payload SIGUSR1 after the
//!   map write, the watcher's poll wakes instantly, and the map —
//!   still the only authority — is re-read at once instead of at the
//!   next heartbeat. The 10 s heartbeat retired to a 60 s backstop
//!   that arms ONLY when the poll set is empty (both evented lanes
//!   dead — an exotic sandbox), because a timer is the only remaining
//!   wake channel there and `off` must always land.
//!
//! Crash posture: cattle, not pet. A dead player loses nothing — the
//! next `zylaxion on` sees the stale pid in the control map, replants
//! itself, and keeps playing. There is nothing to supervise because
//! there is nothing to inherit.

use std::os::fd::AsRawFd as _;
use std::process;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use arc_swap::ArcSwap;
use aya::maps::MapData;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

use crate::core::Orchestrator;
use crate::ebpf::loader::KeyRingEvent;
use crate::ebpf::pin;
use crate::profiles::MechanicalClick;

/// The bridge's poll cap, the FALLBACK shutdown lane
/// (night-frontier-Z4): with the wind-down pipe the bridge's poll is
/// a FOREVER BLOCK and this value never arms; it stays for the exotic
/// sandbox where pipe(2) was refused, and as that posture's
/// documented worst-case exit latency.
const BRIDGE_POLL_MS: u16 = 100;

/// The percent-of-default gain law: the control map carries a percent,
/// the pool wants a linear gain. 100 percent means the compiled laptop
/// default (5.5x); 0 percent is silence.
///
/// night-netrunner-Z2: the body's home is `engine::gain` (the
/// gain-law core the detached proofs package includes — the SAME
/// bytes the watcher writes); this seat stays as the tree's own
/// import path (a pure move, zero behavior change).
pub(crate) fn gain_from_percent(percent: u32) -> f32 {
    crate::engine::gain::gain_from_percent(percent)
}

/// Look up a preset profile by its registry index. Out-of-range indexes
/// (a registry that changed between binaries) fall back to the compiled
/// default — the alternative (refusing to play) strands the user on a
/// number the kernel map happened to carry.
pub(crate) fn preset_by_index(index: u32) -> crate::profiles::KeyProfile {
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
pub(crate) fn open_control_array() -> Result<aya::maps::Array<MapData, u32>, String> {
    let data = MapData::from_pin(pin::PIN_MAP_CONTROL).map_err(|e| {
        format!(
            "cannot open the pinned control map (is the tap attached? `sudo zylaxion attach`): {e}"
        )
    })?;
    let owned = aya::maps::Map::Array(data);
    aya::maps::Array::try_from(owned)
        .map_err(|e| format!("the pinned control map is not an array: {e}"))
}

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

    // The kernel-policy gate (night-badas-4 round 5, the era split
    // night-ebpf-era-1): the player's whole registration ride is
    // pinned-fd map ops — a dispatch-era kernel (5.15-5.18) that
    // gates every plain-user bpf() call ends the run here, classified,
    // instead of an EPERM surfacing as a cryptic map error. 5.19+
    // narrow the gate to object creation: the verbs create nothing,
    // and the ops ride the pins' own file permissions.
    if let Some(refusal) = crate::session::kernel::probe()
        .unpriv_map_ops_refusal(crate::session::privilege::euid_is_root())
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

    // ── The render loop's stop lane (night-frontier-Z4): the watcher
    //    forwards every stop decision as ONE ControlStop message — a
    //    parked loop is woken by a message, never by a flag (the old
    //    flag rode the loop's timer, and the timer is gone).
    let (stop_tx, stop_rx) = crossbeam_channel::unbounded::<crate::core::ControlStop>();

    // ── The bridge's wind-down wake (night-frontier-Z4): a pipe the
    //    main thread writes after the render loop exits, so the
    //    bridge's poll can be a FOREVER BLOCK — the bridge's 100 ms
    //    timer dies with the idle-CPU audit's cadence, and the byte is
    //    the shutdown lane. Best-effort: a pipe(2) failure (an exotic
    //    sandbox) keeps the old 100 ms cap as the shutdown lane, fully
    //    correct on its own.
    let (bridge_wake_read, bridge_wake_write): (
        Option<std::os::fd::OwnedFd>,
        Option<std::os::fd::OwnedFd>,
    ) = match nix::unistd::pipe2(nix::fcntl::OFlag::O_CLOEXEC | nix::fcntl::OFlag::O_NONBLOCK) {
        Ok(pair) => (Some(pair.0), Some(pair.1)),
        Err(e) => {
            log::warn!("bridge wind-down pipe unavailable ({e}) — the 100 ms cap remains");
            (None, None)
        }
    };
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
                        // SAFETY: the record is read through a reference
                        // built from the ringbuf item's own bytes: the
                        // length was bounds-checked above, the alignment is
                        // the ringbuf contract the debug_assert names, and
                        // the item outlives the read.
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
                    // Block until the tap rings — and, night-frontier-Z4,
                    // that is ALL this wait does: with the wind-down pipe
                    // the poll is a FOREVER BLOCK (no timer; the process's
                    // teardown writes one byte and the poll wakes). Without
                    // the pipe (pipe(2) refused — an exotic sandbox) the
                    // old 100 ms cap stays the shutdown lane.
                    let mut fds: Vec<PollFd> = Vec::with_capacity(2);
                    fds.push(PollFd::new(events.as_fd(), PollFlags::POLLIN));
                    if let Some(wake) = &bridge_wake_read {
                        fds.push(PollFd::new(wake.as_fd(), PollFlags::POLLIN));
                    }
                    let timeout = if bridge_wake_read.is_some() {
                        PollTimeout::NONE
                    } else {
                        PollTimeout::from(BRIDGE_POLL_MS)
                    };
                    match poll(&mut fds, timeout) {
                        Ok(_) => {
                            // The wake slot readable = the wind-down byte
                            // (the flag was stored before the byte was
                            // written) — exit. Otherwise the events slot
                            // answered and the loop top drains it.
                            if fds.len() > 1
                                && fds[1]
                                    .revents()
                                    .is_some_and(|f| f.intersects(PollFlags::POLLIN))
                            {
                                return;
                            }
                        }
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

    // ── The control-plane watcher (night-frontier-Z2 B1): the evented
    //    wake — one poll over [the notice ring, the wake pipe], the
    //    10 s heartbeat underneath, a control-map re-read (the ONLY
    //    authority) on every wake. The body lives in control_watch.rs;
    //    the handle is detached by contract (the thread's lifetime is
    //    the process's). A `None` here is the old error path: the
    //    pinned control map refused to open, so the player stops.
    let stop_flag = Arc::new(AtomicBool::new(false));
    let watcher = super::control_watch::spawn(
        Arc::clone(&stop_flag),
        Arc::clone(&model),
        control_wake,
        stop_tx,
        preset_index,
        volume_percent,
        sample_rate,
        volume_factor,
    );
    if watcher.is_none() {
        return;
    }

    // ── Graceful shutdown on SIGTERM / SIGINT / SIGQUIT (same contract
    //    the daemon honored: fade-out, sink drop, clean state).
    //    night-frontier-Z4: the handlers carry the wake byte too — the
    //    watcher parks in a forever poll, so the signal must WAKE it
    //    (the byte rides the control wake pipe), not just mark the
    //    decision (the graceful static).
    if let Err(e) = crate::session::signals::install_graceful_shutdown_handlers(
        crate::session::wake::installed_wake_write_fd(),
    ) {
        log::warn!("failed to install signal handlers: {e}");
    }

    // ── Announce readiness: the state slot flips to running, and the
    //    control plane is now fully live for the CLI surfaces.
    if let Err(e) = control.set(STATE_SLOT, &STATE_RUNNING, 0) {
        log::warn!("cannot set the running state: {e}");
    }
    log::info!("ready — press any key to hear it");

    // ── Run until stopped. run() consumes the receiver; when it
    //    returns (a ControlStop, channel disconnect, or audio
    //    failure), unregister and exit.
    orchestrator.run(&model, &rx, &stop_rx);

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
    // ── Wind down the two parked threads (night-frontier-Z4): the
    //    flag is the decision, the byte is the wake — each parked poll
    //    gets its evented exit. The watcher's byte rides the control
    //    wake pipe (the same lane SIGUSR1 uses; a lost byte costs
    //    latency only — the 60 s backstop covers the exotic no-pipe
    //    posture). The bridge's byte rides its own wind-down pipe
    //    (absent only when pipe(2) was refused, where the 100 ms cap
    //    remains).
    stop_flag.store(true, Ordering::Relaxed);
    crate::session::wake::poke();
    bridge_stop.store(true, Ordering::Relaxed);
    if let Some(fd) = bridge_wake_write.as_ref() {
        let byte = 1u8;
        // SAFETY: write(2) on the nonblocking pipe's write end with a
        // local one-byte buffer; EAGAIN is dropped — a byte already
        // pending arms the same wake.
        let _ = unsafe { libc::write(fd.as_raw_fd(), (&byte as *const u8).cast(), 1) };
    }
    let _ = bridge.join();
    if let Some(handle) = watcher {
        let _ = handle.join();
    }
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
    // SAFETY: setrlimit takes a plain const pointer to the ABI's own
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
