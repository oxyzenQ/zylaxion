// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Signal handling for graceful shutdown (night-frontier-Z4 rewrite).
//!
//! Registers handlers for `SIGTERM`, `SIGINT` (Ctrl+C), `SIGQUIT`
//! (Ctrl+\), and `SIGHUP` (terminal close) that mark the graceful-stop
//! static and — the Z4 half — write ONE byte to the control wake pipe.
//! The two facts travel together: the flag is the decision, the byte
//! is the wake.
//!
//! # Why the byte is load-bearing now (night-frontier-Z4)
//!
//! The Z4 player's threads park in FOREVER polls and blocking channel
//! waits — a parked thread is asleep in the kernel until a channel op
//! or an fd event lands, and no atomic store can wake it. The old
//! chain leaned on the render loop's timer to notice the flag; that
//! timer is gone, so the signal itself must carry the wake: the
//! handler writes the same pipe the SIGUSR1 control-wake handler
//! writes, the watcher's `poll(2)` wakes on the byte, sees the flag,
//! forwards one [`crate::core::ControlStop`] to the render loop, and
//! the clean exit path (fade-out, sink drop, map unregister) runs
//! exactly as before — only faster: the wake is immediate, not the
//! old registry-thread hop plus a 100 ms poll.
//!
//! # Why the house's own sigaction (and why signal-hook left)
//!
//! The former lane was `signal_hook::flag::register`: the C-level
//! handler wrote a registry self-pipe and a background registry
//! thread set the flag — correct, but a thread hop per signal, one
//! extra dependency, and (the deciding point) a mechanism that cannot
//! also write our wake byte. `AtomicBool::store(Relaxed)` and a
//! single `write(2)` to a raw fd are both async-signal-safe (the same
//! discipline `session::wake`'s SIGUSR1 handler documents), so one
//! plain `sigaction` handler does the whole job; `signal-hook` leaves
//! the dependency tree with it (12 → 11 direct dependencies, the peak
//! lean law moving forward, not backward).
//!
//! # Why not `std::process::exit()` in the handler?
//!
//! `std::process::exit()` does NOT run `Drop` guards. If we called it
//! from a signal handler, the audio stream would be torn down
//! mid-buffer (causing PipeWire pops) and the pinned map fds would be
//! closed uncleanly, leaving the player's unregister step skipped
//! until the kernel reaped the process (the next `zylaxion on` would
//! then read a stale pid and take it over — correct, but noisier than
//! a clean exit). By doing only the store and the byte, the main
//! thread finishes its cleanup path: fade-out → `CpalSink::drop()` →
//! the control map unregister → process exit.
//!
//! # SIGHUP (v10.2.0+ — dragonzen audit B11)
//!
//! When `_play` runs in a foreground terminal (the diagnostic lane
//! the startup-failure tips point at) and the user closes that
//! terminal, the kernel sends SIGHUP — the default action (terminate)
//! would skip all Drop guards: no fade-out, PipeWire pop. SIGHUP
//! stays in the graceful set. For the detached spawn (`on` puts the
//! player in its own process group with stdio null), SIGHUP is
//! harmless: a terminal close signals the foreground group (and the
//! session leader) only, so the detached player is never sent one
//! during normal operation.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// The graceful-stop decision, set by the handler. The watcher reads
/// this on every wake alongside its own stop flag — two setters, one
/// readable truth (the handler cannot reach the watcher's `Arc`
/// storage; a static is the async-signal-safe bridge).
static GRACEFUL_STOP: AtomicBool = AtomicBool::new(false);

/// The control wake pipe's write end, stored by
/// [`install_graceful_shutdown_handlers`] from
/// [`crate::session::wake::installed_wake_write_fd`] (-1 when the
/// wake lane is not installed — the byte is skipped, and the stop
/// degrades to the lanes that remain, the same best-effort posture
/// every wake lane carries).
static WAKE_FD: AtomicI32 = AtomicI32::new(-1);

/// The handler: one atomic store, one pipe byte, nothing else. Every
/// operation is async-signal-safe; a failed write (EAGAIN on an
/// implausibly full pipe) is dropped on purpose — the store alone
/// still marks the stop, and the watcher's remaining lanes still wake
/// it.
extern "C" fn graceful_handler(_sig: libc::c_int) {
    GRACEFUL_STOP.store(true, Ordering::Relaxed);
    let fd = WAKE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = 1u8;
        // SAFETY: write(2) takes a plain const pointer used for the
        // call's duration; `byte` is a local of exactly the type
        // pointed to and nothing else sees it.
        let _ = unsafe { libc::write(fd, (&byte as *const u8).cast(), 1) };
    }
}

/// Whether a graceful signal has landed (SIGTERM / SIGINT / SIGQUIT /
/// SIGHUP). The control watcher checks this on every wake, next to
/// its own stop flag.
pub fn graceful_stop_requested() -> bool {
    GRACEFUL_STOP.load(Ordering::Relaxed)
}

/// Register the graceful-shutdown handlers for SIGTERM, SIGINT
/// (Ctrl+C), SIGQUIT (Ctrl+\), and SIGHUP (terminal close).
///
/// `wake_write_fd` is the control wake pipe's write end (pass
/// [`crate::session::wake::installed_wake_write_fd`] — -1 when the
/// wake lane failed to install; the handlers still mark the stop, and
/// the watcher's remaining lanes still carry it).
///
/// Call this ONCE per process, before the orchestrator loop starts.
///
/// # Errors
///
/// Returns an error string if any signal registration fails. This is
/// non-fatal — the caller logs a warning and continues (the loop
/// still exits on input-channel disconnect or the control stop lane,
/// just not on Ctrl+C / `kill` / terminal close).
pub fn install_graceful_shutdown_handlers(wake_write_fd: libc::c_int) -> Result<(), String> {
    WAKE_FD.store(wake_write_fd, Ordering::Relaxed);
    // SAFETY: sigaction takes a plain struct and two pointers;
    // `action` is a zeroed local of the ABI's own struct type, the
    // handler is an `extern "C" fn` of the exact expected signature
    // (no SA_SIGINFO, so the (c_int) shape is right), and neither
    // pointer outlives the call. SA_RESTART keeps every unrelated
    // syscall restartable; poll(2) is on the kernel's
    // never-restarted list (signal(7)), so it still returns EINTR the
    // moment a wake lands — which is exactly the evented hop the Z4
    // stop lane needs.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = graceful_handler as *const () as usize;
        action.sa_flags = libc::SA_RESTART;
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGQUIT, libc::SIGHUP] {
            if libc::sigaction(sig, &action, std::ptr::null_mut()) != 0 {
                let e = std::io::Error::last_os_error();
                return Err(format!("failed to register signal {sig} handler: {e}"));
            }
        }
    }
    Ok(())
}
