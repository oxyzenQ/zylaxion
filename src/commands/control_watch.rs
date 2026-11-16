// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The control-plane watcher (night-frontier-Z2 B1) — its own module
//! so the player stays under the no-mercy line ledger.
//!
//! The wait is ONE poll over [the notice ring, the wake pipe] with the
//! 10 s heartbeat underneath; the drained records are kernel-stamped
//! hints, the control map is the only authority, and a wake that
//! carried no change says "authority held" out loud. The 10 s read-
//! failure patience survived the cadence demotion, honestly
//! re-derived from a read count to a clock (the night-dragon-lts law).

use std::os::fd::{AsFd as _, BorrowedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use aya::maps::MapData;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

use crate::ebpf::pin;
use crate::profiles::MechanicalClick;
use crate::session::wake::ControlWake;

use super::player::{gain_from_percent, open_control_array, preset_by_index};

// The userspace mirror of control_slots in ebpf/src/main.rs (and
// ebpf::loader::control) — the same private-alias pattern the player
// module carries; the loader's test tree pins the cross-side
// contract.
const ENABLED_SLOT: u32 = 0;
const PRESET_ID_SLOT: u32 = 1;
const VOLUME_PCT_SLOT: u32 = 2;

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

/// Spawn the control watcher. `None` means the pinned control map
/// could not be opened — the caller stops (a player without its
/// control plane is an immortal orphan, not a player).
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    stop_flag: Arc<AtomicBool>,
    model: Arc<ArcSwap<MechanicalClick>>,
    control_wake: Option<ControlWake>,
    preset_index: u32,
    volume_percent: u32,
    sample_rate: u32,
    volume_factor: Arc<AtomicU32>,
) -> Option<std::thread::JoinHandle<()>> {
    // ── The control-plane watcher: the evented wake (night-frontier-Z2
    //    B1) — one poll over [the notice ring, the wake pipe], the 10 s
    //    heartbeat underneath, and a control-map re-read (the ONLY
    //    authority) on every wake. enabled=0 stops (the CLI's off/exit
    //    is one map write plus one notice); preset and volume changes
    //    apply live.
    let watcher =
        {
            let stop_flag = Arc::clone(&stop_flag);
            let model = Arc::clone(&model);
            let watch_control = match open_control_array() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("watcher cannot open the control map: {e}");
                    stop_flag.store(true, Ordering::Relaxed);
                    return None;
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
                                "control notice kind {} value {} — wake {} ns",
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
    Some(watcher)
}
