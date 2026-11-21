// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The control-plane wakeup (night-hardening-2, the last polling
//! hole).
//!
//! The player's control watcher re-reads the pinned control map every
//! 100 ms — so the worst-case latency of an `off`, a preset switch,
//! or a volume change was one full watcher cadence, and the
//! heartbeat woke the thread ten times a second even when nothing
//! changed. This module adds the zero-payload fast path on top of the
//! same heartbeat:
//!
//! - A pipe whose write end is owned by the process-lifetime SIGUSR1
//!   handler. The handler does exactly one async-signal-safe thing:
//!   `write(2)` a single byte. No allocation, no lock, no logging —
//!   the same discipline `signal-hook`'s flag handlers follow, minus
//!   the atomic store (a flag cannot interrupt the watcher's sleep;
//!   a pipe byte can, through `poll(2)`).
//! - The watcher waits in `poll(2)` on the pipe's read end with the
//!   same 100 ms timeout — the heartbeat, unchanged: the
//!   read-failure budget of night-dragon-lts still counts 10 s of
//!   consecutive failures, and the cadence law is untouched. A wake
//!   byte or an interrupted poll returns immediately; the watcher
//!   then re-reads the control map, which remains the ONLY authority.
//!   The signal carries no data, so there is no protocol to parse,
//!   version, or defend — it is a wake hint, the same role the
//!   events ring buffer's epoll wake plays for keystrokes.
//!
//! The CLI side (`commands::sound::wake_player`) raises the signal
//! only after the map write landed, only for a pid whose handlers are
//! provably installed (the RUNNING state), and only while the pid
//! passes the orphan module's zombie-aware liveness probe — a
//! STARTING player has no SIGUSR1 handler yet, and the signal's
//! default action is terminate, so the state gate is load-bearing,
//! not cosmetic.
//!
//! Degradation posture: if the pipe or the sigaction registration
//! fails (an exotic sandbox, an fd-exhausted host), the player logs a
//! warning and continues on the heartbeat alone — the 100 ms cadence
//! remains fully correct on its own. The fast path is an optimization
//! over a correct poller, never a correctness dependency.

use std::os::fd::{AsFd, AsRawFd};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, Ordering};

use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

/// The wakeup signal — a conventional, unused-by-default user signal.
/// SIGUSR1's default disposition is terminate, which is exactly why
/// the handler is installed BEFORE the player registers its pid in
/// the control map: a verb racing the startup window hits the
/// handler, never the default.
pub const CONTROL_WAKE_SIGNAL: libc::c_int = libc::SIGUSR1;

/// The process-lifetime write end. The handler writes to it on every
/// wake; the fd outlives every thread, so the pipe never closes while
/// the process runs.
static WAKE_WRITE_FD: OnceLock<OwnedFdHolder> = OnceLock::new();

/// The raw mirror of the fd above: the handler cannot lock or
/// allocate, so it reads this plain atomic instead (an atomic load is
/// async-signal-safe).
static WAKE_WRITE_FD_RAW: AtomicI32 = AtomicI32::new(-1);

/// A leak-free holder for the write end. `OnceLock<OwnedFd>` would
/// need the value moved in before the sigaction call reads its raw
/// fd; this holder is the same thing without the borrow dance.
struct OwnedFdHolder(std::os::fd::OwnedFd);

/// The handler: one byte, one syscall, nothing else. Every operation
/// is async-signal-safe (an atomic load and `write(2)`); a failed
/// write (EAGAIN on an implausibly full 64 KiB pipe) is dropped on
/// purpose — the heartbeat re-reads the map anyway, so a lost wake
/// byte costs latency, never correctness.
extern "C" fn control_wake_handler(_sig: libc::c_int) {
    let fd = WAKE_WRITE_FD_RAW.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = 1u8;
        // SAFETY: write(2) takes a plain const pointer used for the
        // call's duration; `byte` is a local of exactly the type
        // pointed to and nothing else sees it.
        let _ = unsafe { libc::write(fd, (&byte as *const u8).cast(), 1) };
    }
}

/// The installed wakeup lane, owned by the player's watcher.
pub struct ControlWake {
    /// The pipe's read end — what the watcher polls.
    read_end: std::os::fd::OwnedFd,
}

/// Create the wakeup pipe and install the SIGUSR1 handler. The
/// handler goes live before this returns, so the caller's contract is
/// simply: install BEFORE registering the player pid.
pub fn install() -> Result<ControlWake, String> {
    let (read_end, write_end) =
        nix::unistd::pipe().map_err(|e| format!("cannot create the control wakeup pipe: {e}"))?;
    if WAKE_WRITE_FD.set(OwnedFdHolder(write_end)).is_err() {
        return Err("the control wakeup lane is already installed".to_string());
    }
    let raw = WAKE_WRITE_FD.get().expect("just set").0.as_raw_fd();
    WAKE_WRITE_FD_RAW.store(raw, Ordering::Relaxed);
    // SAFETY: sigaction takes a plain struct and two pointers;
    // `action` is a zeroed local of the ABI's own type, the handler is
    // an `extern "C" fn` of the exact expected signature (no
    // SA_SIGINFO, so the (c_int) shape is right), and neither pointer
    // outlives the call.
    // SAFETY: the sigaction call is given a zeroed local of the ABI's
    // own struct type and a handler of the exact expected signature;
    // the summary above carries the per-field argument.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = control_wake_handler as *const () as usize;
        // SA_RESTART keeps every unrelated syscall restartable; poll(2)
        // is on the kernel's never-restarted list (signal(7)), so it
        // still returns EINTR the moment a wake lands.
        action.sa_flags = libc::SA_RESTART;
        if libc::sigaction(CONTROL_WAKE_SIGNAL, &action, std::ptr::null_mut()) != 0 {
            let e = std::io::Error::last_os_error();
            WAKE_WRITE_FD_RAW.store(-1, Ordering::Relaxed);
            return Err(format!("cannot install the control wakeup handler: {e}"));
        }
    }
    Ok(ControlWake { read_end })
}

impl ControlWake {
    /// The read end's raw fd (night-frontier-Z2 B1): the watcher
    /// polls the notice ring and this pipe in ONE poll(2) set, so the
    /// session fast path and the evented control wake share a wait
    /// and a heartbeat instead of owning separate sleeps.
    pub fn read_fd(&self) -> std::os::fd::RawFd {
        self.read_end.as_raw_fd()
    }

    /// Wait up to `timeout_ms` for a wake byte, then drain whatever
    /// landed.
    ///
    /// night-frontier-Z2 B1: the BINARY no longer calls this — its
    /// watcher polls the notice ring and this pipe in one poll(2) set
    /// and drains via [`ControlWake::drain`]. The test tree still
    /// drives the single-fd wait, so it stays (cfg-allowed, not
    /// deleted). All three outcomes (byte, timeout, interrupted) lead to
    /// the same next step — re-read the control map — so the call
    /// returns no verdict; the drain only runs when the pipe reported
    /// readable (a read on a quiet blocking pipe would hang).
    #[allow(dead_code)]
    pub fn wait(&self, timeout_ms: u16) {
        let fd = self.read_end.as_fd();
        let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
        match poll(&mut fds, PollTimeout::from(timeout_ms)) {
            Ok(0) => {}
            Ok(_) => self.drain(),
            Err(nix::errno::Errno::EINTR) => {}
            Err(e) => log::warn!("control wake pipe poll error: {e}"),
        }
    }

    /// One bounded read of the coalesced wake bytes. A single call
    /// covers every realistic flood (one byte per wake, 4 KiB of
    /// headroom); a remainder, if one ever existed, only makes the
    /// next poll return immediately.
    pub fn drain(&self) {
        let mut buf = [0u8; 4096];
        let fd = self.read_end.as_raw_fd();
        // SAFETY: read(2) takes a plain out-pointer; `buf` is a local
        // buffer that outlives the call and nothing else aliases it.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            log::warn!(
                "control wake pipe read error: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}

/// The installed control wake pipe's write end (raw), or -1 when the
/// wake lane is not installed. The graceful signal handler
/// (`session::signals`) writes its own byte to THIS pipe so a
/// SIGTERM/SIGINT/SIGQUIT/SIGHUP wakes the watcher's parked poll —
/// the evented hop night-frontier-Z4's stop lane needs.
pub fn installed_wake_write_fd() -> libc::c_int {
    WAKE_WRITE_FD_RAW.load(Ordering::Relaxed)
}

/// Write one best-effort wake byte from NORMAL process context (this
/// is NOT async-signal-safe — the SIGUSR1 handler is the only writer
/// in signal context; everything else goes through here). The wind-
/// down lane (night-frontier-Z4): the watcher parks in a forever
/// poll, and a parked thread is woken by an fd event, not by a flag —
/// the process's post-run teardown writes this byte so the watcher's
/// join cannot hang. A lost byte costs latency, never correctness:
/// the watcher's own lanes and backstop still carry the stop.
pub fn poke() {
    let fd = installed_wake_write_fd();
    if fd >= 0 {
        let byte = 1u8;
        // SAFETY: write(2) on the pipe's write end with a local buffer
        // of exactly one byte; a failure (EAGAIN on an implausibly
        // full pipe) is dropped — a pending byte already arms the wake.
        let _ = unsafe { libc::write(fd, (&byte as *const u8).cast(), 1) };
    }
}

#[cfg(test)]
#[path = "../../test/session/wake_tests.rs"]
mod tests;
