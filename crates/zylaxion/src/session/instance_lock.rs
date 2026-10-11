// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Single-instance lock — prevents two `zylaxion` processes from running
//! concurrently on the same machine.
//!
//! ## Why
//!
//! Before this module existed, a user could start `zylaxion start` in one
//! terminal and `zylaxion daemon` in another. Both would open `cpal`
//! output streams on the same PipeWire node, producing audio clashes
//! (interleaved "TEK"s, phase cancellation) and doubling the input-device
//! polling load on libinput. The PID-file check in `daemon::is_daemon_running`
//! only guarded the `daemon` subcommand against itself — `start` was
//! completely unprotected, and `start` vs. `daemon` was never caught.
//!
//! ## How
//!
//! This module opens (creating if missing) a lock file at
//! `$XDG_RUNTIME_DIR/zylaxion.lock` and acquires an exclusive,
//! non-blocking `flock(2)` on it via `nix::fcntl::Flock`. The `Flock`
//! guard is returned to the caller, who must hold it for the entire
//! process lifetime — when the process exits (gracefully, via signal,
//! or via crash), the OS automatically releases the flock and the
//! next `zylaxion` invocation can acquire it.
//!
//! ## Why `flock` and not the PID file
//!
//! `flock(2)` is the correct primitive here because:
//!
//! 1. **Crash-safe.** When a process dies, the kernel releases all its
//!    flocks atomically. No stale-lock cleanup code is needed.
//! 2. **Race-free.** The kernel serialises the lock acquisition; there
//!    is no window where two processes can both think they hold the lock.
//! 3. **No PID recycling.** The lock is tied to the file descriptor,
//!    not to a PID number that Linux may reuse for an unrelated process.
//!
//! The PID file is still kept for `is_daemon_running()` / `cmd_status()`
//! because those need to identify the *running* daemon (e.g. to send it
//! an IPC "stop" command), not merely to detect its presence.

use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use nix::fcntl::{Flock, FlockArg};

/// Filename of the single-instance lock under the runtime directory.
const LOCK_FILE_NAME: &str = "zylaxion.lock";

/// Resolve the lock file path.
///
/// Uses `pathguard::resolve_runtime_dir()` to validate `$XDG_RUNTIME_DIR`
/// — falls back to `/tmp` if the env var points to a dangerous system
/// path (e.g. `/etc`, `/usr`, `~/.ssh`). This prevents the lock file
/// from being written into protected directories.
pub fn lock_path() -> PathBuf {
    crate::session::pathguard::resolve_runtime_dir().join(LOCK_FILE_NAME)
}

/// Acquire an exclusive, non-blocking flock on the single-instance lock
/// file.
///
/// On success, returns a [`Flock<File>`] guard that the caller must hold
/// for the rest of the process lifetime. When the guard (or the process)
/// is dropped, the kernel releases the lock automatically.
///
/// # Errors
///
/// Returns a human-readable error string in all failure cases:
///
/// - The lock file could not be created/opened (permission denied, disk
///   full, etc.).
/// - Another `zylaxion` process currently holds the lock
///   (`EWOULDBLOCK` — surfaced as the canonical "already running"
///   message so the CLI can exit cleanly).
/// - Any other `flock(2)` failure.
pub fn acquire() -> Result<Flock<File>, String> {
    let path = lock_path();

    // Open with O_CREAT | O_RDWR. We never read from or write to the
    // file — flock(2) only cares about the underlying inode, not the
    // file's contents. The file is left empty by design.
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("failed to open lock file at {}: {e}", path.display()))?;

    // Try a non-blocking exclusive lock. If another process holds it,
    // `nix` returns `EWOULDBLOCK` (wrapped in a `(T, Errno)` tuple, as
    // `Flock::lock` returns the original file on error so the caller
    // can retry) which we map to the canonical "already running"
    // message. We discard the returned file because the caller has no
    // use for it once the lock attempt failed.
    match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
        Ok(guard) => Ok(guard),
        Err((_file, nix::errno::Errno::EWOULDBLOCK)) => {
            Err("zylaxion is already running. Stop the other instance first.".to_string())
        }
        Err((_file, e)) => Err(format!("failed to acquire lock at {}: {e}", path.display())),
    }
}

/// Convenience helper for the CLI: acquire the lock or print the error
/// to stderr and exit immediately.
///
/// Used by `cmd_start` and `cmd_daemon` so the boilerplate of mapping
/// the error to `process::exit(1)` lives in one place.
pub fn acquire_or_exit() -> Flock<File> {
    match acquire() {
        Ok(guard) => guard,
        Err(e) => {
            crate::output::error(e);
            std::process::exit(1);
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "../../test/session/instance_lock_tests.rs"]
mod tests;
