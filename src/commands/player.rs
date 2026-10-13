// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The internal player (`zylaxion _play`, night-improve-23).
//!
//! The ONE process that exists while sound plays — and the only one,
//! stateless, owning nothing:
//!
//! - ALL state comes from the pinned BPF maps (preset, volume, enabled,
//!   player pid) — there is no config, no pidfile, no socket, nothing to
//!   supervise.
//! - The input arrives from the kernel-side tap through the pinned
//!   events ring buffer (epoll, zero idle CPU).
//! - The control plane is watched by one thread that re-reads the
//!   control map every 100 ms: enabled=0 stops the player (the CLI's
//!   `off`/`exit` writes exactly that), preset and volume changes apply
//!   live through the ArcSwap model swap and the pool's atomic volume
//!   cell.
//!
//! Crash posture: cattle, not pet. A dead player loses nothing — the
//! next `zylaxion on` sees the stale pid in the control map, replants
//! itself, and keeps playing. There is nothing to supervise because
//! there is nothing to inherit.

use std::process;

#[cfg(feature = "ebpf")]
use std::sync::Arc;
#[cfg(feature = "ebpf")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "ebpf")]
use std::time::Duration;

#[cfg(feature = "ebpf")]
#[cfg(feature = "ebpf")]
use arc_swap::ArcSwap;
#[cfg(feature = "ebpf")]
use aya::maps::MapData;
#[cfg(feature = "ebpf")]
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

#[cfg(feature = "ebpf")]
use crate::core::Orchestrator;
#[cfg(feature = "ebpf")]
use crate::ebpf::pin;
#[cfg(feature = "ebpf")]
use crate::profiles::MechanicalClick;

/// The control watcher's poll cadence: the worst-case latency of an
/// `off`, a preset switch, or a volume change. 100 ms is invisible to
/// a human and costs ~10 Hz of tiny map reads.
const CONTROL_POLL_MS: u16 = 100;

/// The percent-of-default gain law: the control map carries a percent,
/// the pool wants a linear gain. 100 percent means the compiled laptop
/// default (5.5x); 0 percent is silence.
fn gain_from_percent(percent: u32) -> f32 {
    percent as f32 / 100.0 * crate::engine::VoicePool::DEFAULT_MASTER_VOLUME
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

#[cfg(feature = "ebpf")]
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

/// Open the pinned events ring buffer for consuming.
#[cfg(feature = "ebpf")]
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
#[cfg(feature = "ebpf")]
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
        Ok(live_pid) => match pid_alive(live_pid as i32) {
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

    // ── Register self: starting.
    let my_pid = nix::unistd::getpid().as_raw();
    if let Err(e) = control.set(PLAYER_PID_SLOT, my_pid as u32, 0) {
        crate::output::error(format!("cannot register the player pid: {e}"));
        process::exit(1);
    }
    if let Err(e) = control.set(STATE_SLOT, STATE_STARTING, 0) {
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
                        if item.len() < std::mem::size_of::<KeyRingEventRaw>() {
                            continue;
                        }
                        let raw = unsafe { &*(item.as_ptr() as *const KeyRingEventRaw) };
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
                    match poll(&mut fds, PollTimeout::from(CONTROL_POLL_MS)) {
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

    // ── The control-plane watcher: 100 ms reads of the pinned control
    //    map. enabled=0 stops (the CLI's off/exit is one map write);
    //    preset and volume changes apply live.
    let stop_flag = Arc::new(AtomicBool::new(false));
    let watcher = {
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
        std::thread::Builder::new()
            .name("zylaxion-watch".to_string())
            .spawn(move || {
                let mut last_preset = preset_index;
                let mut last_volume = volume_percent;
                loop {
                    std::thread::sleep(Duration::from_millis(CONTROL_POLL_MS.into()));
                    if stop_flag.load(Ordering::Relaxed) {
                        return;
                    }
                    let enabled = watch_control.get(&ENABLED_SLOT, 0).unwrap_or(0);
                    if enabled != 1 {
                        log::info!("control map cleared the enabled flag — stopping");
                        stop_flag.store(true, Ordering::Relaxed);
                        return;
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
                        log::info!("preset switched to index {preset}");
                    }
                    // Volume retune: one atomic store; the next render
                    // call picks it up.
                    let volume = watch_control
                        .get(&VOLUME_PCT_SLOT, 0)
                        .unwrap_or(last_volume);
                    if volume != last_volume {
                        volume_factor.store(gain_from_percent(volume).to_bits(), Ordering::Relaxed);
                        last_volume = volume;
                        log::info!("volume retuned to {volume}%");
                    }
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
    if let Err(e) = control.set(STATE_SLOT, STATE_RUNNING, 0) {
        log::warn!("cannot set the running state: {e}");
    }
    log::info!("ready — press any key to hear it");

    // ── Run until stopped. run() consumes the receiver; when it
    //    returns (stop flag, channel disconnect, or audio failure),
    //    unregister and exit.
    orchestrator.run(&model, &rx, Arc::clone(&stop_flag));

    // ── Unregister: the control map goes back to the no-player
    //    posture. The tap stays attached — pins survive, the next `on`
    //    is instant.
    let _ = control.set(STATE_SLOT, STATE_IDLE, 0);
    let _ = control.set(PLAYER_PID_SLOT, 0, 0);
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

/// The raw ring record — the byte-level twin of `KeyRingEvent` in
/// ebpf/src/main.rs (24 bytes; the loader's test tree pins the layout
/// and this alias reads the same bytes). A local mirror keeps the hot
/// parse a plain pointer cast.
#[cfg(feature = "ebpf")]
#[repr(C)]
struct KeyRingEventRaw {
    ts_us: u64,
    keycode: u32,
    value: u32,
    slot: u32,
}

/// Liveness probe for the pid stored in the control map: signal 0 is a
/// no-op that checks existence.
fn pid_alive(pid: i32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_ok()
}

/// A fatal player error: mark the control map FAILED, unregister, and
/// exit. The stderr message is all the user sees (the player runs
/// detached), so it carries the fix.
#[cfg(feature = "ebpf")]
fn fail(control: &mut aya::maps::Array<MapData, u32>, message: String) -> ! {
    let _ = control.set(STATE_SLOT, 3, 0);
    let _ = control.set(PLAYER_PID_SLOT, 0, 0);
    crate::output::error(message);
    process::exit(1);
}

/// Shared logger wiring (the night-improve-13 lane).
fn init_logger() {
    use std::io::Write as _;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {:<5} {}] {}",
                crate::info::clock::iso_utc_millis(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
}

/// Dormant lane (_play): a binary built without the eBPF loader cannot
/// play — the honest refusal.
#[cfg(not(feature = "ebpf"))]
pub fn cmd_play() {
    crate::output::error(crate::ebpf::dormant::DORMANT_REFUSAL);
    process::exit(1);
}
