// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Signal handling for graceful shutdown.
//!
//! Registers handlers for `SIGTERM`, `SIGINT` (Ctrl+C), `SIGQUIT`
//! (Ctrl+\), and `SIGHUP` that atomically set a shared `stop_flag` to
//! `true`. The orchestrator's main loop polls this flag every iteration
//! and breaks cleanly — dropping `CpalSink` (with fade-out),
//! `LibinputSource`, and the `Flock` instance lock.
//!
//! # Why not `std::process::exit()` in the handler?
//!
//! `std::process::exit()` does NOT run `Drop` guards. If we called it
//! from a signal handler, the audio stream would be torn down mid-buffer
//! (causing PipeWire pops), the libinput capture would leak (leaving
//! the terminal in a confused state), and the flock would remain held
//! until the kernel reaped the process (potentially blocking the next
//! `zylaxion` start for seconds).
//!
//! By only setting an atomic flag, we let the main thread finish its
//! cleanup path: `fade_out_before_drop()` → `CpalSink::drop()` →
//! `LibinputSource::drop()` → `Flock::drop()` → process exit.
//!
//! # Why `signal-hook`?
//!
//! `signal_hook::flag::register` writes to an `Arc<AtomicBool>` directly
//! from the signal handler. `AtomicBool::store(Relaxed)` is
//! async-signal-safe, so this is sound. We never allocate, never call
//! non-async-signal-safe functions, and never hold locks in the handler.
//!
//! # SIGHUP (v10.2.0+ — dragonzen audit B11)
//!
//! The previous version registered only SIGTERM/SIGINT/SIGQUIT. When
//! `cmd_start` ran in the foreground and the user closed the terminal,
//! the kernel sent SIGHUP — the default action (terminate) ran
//! `process::exit` from the signal handler, skipping all Drop guards.
//! The result: no fade-out, PipeWire pop, stale PID/lock files. The
//! fix adds SIGHUP to the graceful-shutdown set. In `cmd_daemon
//! --foreground` mode (systemd), SIGHUP is harmless — systemd doesn't
//! send it during normal operation.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::flag;

/// Register signal handlers that set `stop_flag` to `true` on
/// `SIGTERM`, `SIGINT` (Ctrl+C), `SIGQUIT` (Ctrl+\), and `SIGHUP`
/// (terminal close).
///
/// Call this ONCE per process, after the `stop_flag` is created and
/// before the orchestrator loop starts.
///
/// # Errors
///
/// Returns an error string if any signal registration fails. This is
/// non-fatal — the caller should log a warning and continue (the
/// orchestrator loop will still exit on input-channel disconnect or
/// IPC "stop" command, just not on Ctrl+C / `kill` / terminal close).
pub fn install_graceful_shutdown_handlers(stop_flag: Arc<AtomicBool>) -> Result<(), String> {
    for sig in [SIGTERM, SIGINT, SIGQUIT, SIGHUP] {
        flag::register(sig, Arc::clone(&stop_flag))
            .map_err(|e| format!("failed to register signal {sig} handler: {e}"))?;
    }
    Ok(())
}
