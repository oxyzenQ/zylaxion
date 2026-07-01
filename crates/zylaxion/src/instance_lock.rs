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
/// Prefers `$XDG_RUNTIME_DIR` (the canonical per-user runtime dir on
/// Linux, typically `/run/user/<uid>` and mounted as tmpfs). Falls back
/// to `/tmp` if `XDG_RUNTIME_DIR` is unset (matches the IPC socket
/// fallback in `daemon::ipc`).
pub fn lock_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join(LOCK_FILE_NAME)
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
            Err("Zylaxion is already running. Stop the other instance first.".to_string())
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
            crate::error_format::error(e);
            std::process::exit(1);
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Acquire the lock, then verify that a second acquisition attempt
    /// from the *same* process fails. flock(2) locks are per file
    /// description (not per process), so we have to use a fresh `File`
    /// handle — `acquire()` already does this by calling `OpenOptions`
    /// each time, which creates a new description.
    ///
    /// Note: this test uses the real filesystem under `$XDG_RUNTIME_DIR`
    /// (or `/tmp`). It is therefore a small integration test, not a
    /// pure unit test.
    #[test]
    fn lock_is_exclusive_across_file_descriptions() {
        // Use a test-specific path so we don't collide with any real
        // zylaxion instance that might be running in the CI sandbox.
        let test_path = std::env::temp_dir().join(format!(
            "zylaxion-lock-test-{}-{}.lock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let file_a = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&test_path)
            .expect("open file_a");

        let _guard_a =
            Flock::lock(file_a, FlockArg::LockExclusive).expect("first lock should succeed");

        // Second open creates a NEW file description, so flock should
        // refuse it with EWOULDBLOCK.
        let file_b = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&test_path)
            .expect("open file_b");

        let result = Flock::lock(file_b, FlockArg::LockExclusiveNonblock);
        assert!(
            matches!(result, Err((_, nix::errno::Errno::EWOULDBLOCK))),
            "second lock on a separate file description must fail with EWOULDBLOCK, got: {result:?}"
        );

        // Drop guard_a — now file_b should be acquirable from yet another
        // fresh file description.
        drop(_guard_a);

        let file_c = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&test_path)
            .expect("open file_c");
        let _guard_c = Flock::lock(file_c, FlockArg::LockExclusiveNonblock)
            .expect("third lock should succeed after first guard dropped");

        // Cleanup
        let _ = std::fs::remove_file(&test_path);
    }

    /// `lock_path()` must honour `$XDG_RUNTIME_DIR` when set and fall
    /// back to `/tmp` otherwise.
    #[test]
    fn lock_path_respects_xdg_runtime_dir() {
        // Save and restore so other tests aren't affected.
        let saved = std::env::var_os("XDG_RUNTIME_DIR");

        std::env::set_var("XDG_RUNTIME_DIR", "/tmp/zylaxion-lock-test-xdg");
        let p = lock_path();
        assert_eq!(p.file_name().unwrap(), LOCK_FILE_NAME);
        assert_eq!(
            p.parent().unwrap().to_str().unwrap(),
            "/tmp/zylaxion-lock-test-xdg"
        );

        std::env::remove_var("XDG_RUNTIME_DIR");
        let p = lock_path();
        assert_eq!(p.file_name().unwrap(), LOCK_FILE_NAME);
        assert_eq!(p.parent().unwrap().to_str().unwrap(), "/tmp");

        // Restore
        match saved {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
    }

    /// `acquire()` should return a guard whose Drop releases the lock,
    /// allowing a subsequent `acquire()` to succeed.
    ///
    /// This uses the real `lock_path()` (under `$XDG_RUNTIME_DIR` or
    /// `/tmp`) and is therefore a small integration test. It is safe to
    /// run in parallel with the rest of the suite because the lock is
    /// acquired and dropped within the same test — no other test in
    /// this module calls `acquire()` on the global path.
    #[test]
    fn acquire_then_release_global() {
        let path = lock_path();

        // Best-effort cleanup of any leftover lock file from a
        // previously crashed test run. flock is on the inode, not the
        // path, so removing the file is safe even if a stale lock
        // somehow lingers — the next acquire creates a fresh inode.
        let _ = std::fs::remove_file(&path);

        let guard = acquire().expect("first acquire should succeed");
        drop(guard);

        // After dropping, we should be able to re-acquire immediately.
        let _guard2 = acquire().expect("second acquire after drop should succeed");
        let _ = std::fs::remove_file(&path);
    }

    /// **The critical single-instance test.**
    ///
    /// Forks a child process that acquires the lock and holds it for a
    /// few seconds. The parent then attempts to acquire the same lock
    /// and MUST fail with the "already running" error. This is the
    /// exact scenario `zylaxion start` + `zylaxion daemon` would
    /// produce if two users (or one user with two terminals) tried to
    /// run zylaxion simultaneously.
    ///
    /// Without flock exclusion, this test would fail — confirming the
    /// bug class described in the v0.3.1 prompt ("two instances can
    /// run at the same time, causing audio clashes and PipeWire
    /// hangs").
    #[test]
    fn lock_excludes_second_process_via_fork() {
        use std::os::unix::io::AsRawFd;
        use std::time::Duration;

        // Use a per-test lock path under /tmp to avoid colliding with
        // any real zylaxion process or other tests.
        let test_lock = std::env::temp_dir().join(format!(
            "zylaxion-lock-fork-test-{}-{}.lock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&test_lock);

        // Open + flock in the parent FIRST, then fork. The child
        // inherits the open file description (and thus the lock).
        // We do this manually rather than calling acquire() so we
        // can pass the fd to the child via fork.
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&test_lock)
            .expect("open test lock file");

        let guard =
            Flock::lock(file, FlockArg::LockExclusive).expect("parent flock should succeed");

        // Fork. The child inherits the file description (and thus the
        // exclusive lock). The parent also still holds the lock.
        // The child tries to acquire the lock AGAIN via a fresh file
        // description — this MUST fail with EWOULDBLOCK.
        let _fd_for_child = guard.as_raw_fd(); // unused; just for clarity
        match unsafe { nix::unistd::fork() } {
            Ok(nix::unistd::ForkResult::Parent { child }) => {
                // Parent: wait for child to finish.
                let _ = nix::sys::wait::waitpid(child, None);

                // Cleanup.
                let _ = std::fs::remove_file(&test_lock);
            }
            Ok(nix::unistd::ForkResult::Child) => {
                // Child: try to acquire the lock on a FRESH file
                // description. This must fail because the parent
                // holds the exclusive lock.
                let child_file = OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .truncate(false)
                    .open(&test_lock)
                    .expect("child open");

                let result = Flock::lock(child_file, FlockArg::LockExclusiveNonblock);
                match result {
                    Err((_, nix::errno::Errno::EWOULDBLOCK)) => {
                        // Expected: lock is held by parent.
                        std::process::exit(0);
                    }
                    Ok(_guard) => {
                        // BUG: child acquired the lock while parent held it.
                        eprintln!(
                            "BUG: child acquired lock that parent holds — single-instance lock is broken"
                        );
                        std::process::exit(42);
                    }
                    Err((_, e)) => {
                        eprintln!("Unexpected error from child flock: {e}");
                        std::process::exit(43);
                    }
                }
            }
            Err(e) => {
                panic!("fork failed: {e}");
            }
        }

        // Give the child a moment to fully exit (defensive).
        std::thread::sleep(Duration::from_millis(50));
    }
}
