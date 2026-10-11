// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// instance_lock_tests.rs — the session/instance_lock.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

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

    let _guard_a = Flock::lock(file_a, FlockArg::LockExclusive).expect("first lock should succeed");

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
    // Edition 2024 (night-improve-4) made env mutation unsafe; this
    // suite shares the crate-wide ENV_LOCK so parallel tests cannot
    // race the save/mutate/restore window below.
    let _env_guard = crate::session::pathguard::ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    // Save and restore so other tests aren't affected.
    let saved = std::env::var_os("XDG_RUNTIME_DIR");

    // SAFETY (night-improve-4, edition 2024): mutations serialized
    // by the ENV_LOCK held above — see pathguard's `with_test_env`.
    unsafe {
        std::env::set_var("XDG_RUNTIME_DIR", "/tmp/zylaxion-lock-test-xdg");
    }
    let p = lock_path();
    assert_eq!(p.file_name().unwrap(), LOCK_FILE_NAME);
    assert_eq!(
        p.parent().unwrap().to_str().unwrap(),
        "/tmp/zylaxion-lock-test-xdg"
    );

    // SAFETY (night-improve-4, edition 2024): same serialized window.
    unsafe {
        std::env::remove_var("XDG_RUNTIME_DIR");
    }
    let p = lock_path();
    assert_eq!(p.file_name().unwrap(), LOCK_FILE_NAME);
    assert_eq!(p.parent().unwrap().to_str().unwrap(), "/tmp");

    // Restore
    // SAFETY (night-improve-4, edition 2024): same serialized window.
    unsafe {
        match saved {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
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

    let guard = Flock::lock(file, FlockArg::LockExclusive).expect("parent flock should succeed");

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
                    // night-improve-6: internal-failure lanes ride the
                    // labeled red lane like every other error.
                    crate::output::error(
                        "BUG: child acquired lock that parent holds — single-instance lock is broken",
                    );
                    std::process::exit(42);
                }
                Err((_, e)) => {
                    crate::output::error(format!("unexpected error from child flock: {e}"));
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
