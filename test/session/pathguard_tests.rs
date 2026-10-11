// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// pathguard_tests.rs — the session/pathguard.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

fn with_test_env<F: FnOnce()>(home: &str, xdg_runtime: Option<&str>, f: F) {
    // Recover from poison if a previous test panicked while holding the lock.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old_home = std::env::var_os("HOME");
    let old_xdg = std::env::var_os("XDG_RUNTIME_DIR");

    // SAFETY (night-improve-4, edition 2024): `set_var`/`remove_var`
    // are unsafe because they race with concurrent environment
    // reads. ENV_LOCK is held for the whole save/mutate/test/restore
    // window, and every env-mutating test in this crate takes the
    // same crate-wide lock, so no concurrent read can observe a
    // torn state here.
    unsafe {
        std::env::set_var("HOME", home);
        match xdg_runtime {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
    }

    f();

    // SAFETY (night-improve-4, edition 2024): restore path — same
    // serialized ENV_LOCK window as the mutation block above.
    unsafe {
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        match old_xdg {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
    }
}

#[test]
fn test_default_runtime_dir_when_unset() {
    with_test_env("/home/testuser", None, || {
        let dir = resolve_runtime_dir();
        assert_eq!(dir, PathBuf::from("/tmp"));
    });
}

#[test]
fn test_xdg_runtime_dir_respected_when_safe() {
    with_test_env("/home/testuser", Some("/run/user/1000"), || {
        let dir = resolve_runtime_dir();
        // /run/user/1000 might not exist in test env, so we get
        // lexical normalize. Check it's not /tmp and not dangerous.
        assert_ne!(dir, PathBuf::from("/tmp"));
        assert!(!is_dangerous(&dir));
    });
}

#[test]
fn test_xdg_to_etc_is_blocked() {
    with_test_env("/home/testuser", Some("/etc"), || {
        let dir = resolve_runtime_dir();
        assert_eq!(dir, PathBuf::from("/tmp"));
    });
}

#[test]
fn test_xdg_to_user_ssh_is_blocked() {
    with_test_env("/home/testuser", Some("/home/testuser/.ssh"), || {
        let dir = resolve_runtime_dir();
        assert_eq!(dir, PathBuf::from("/tmp"));
    });
}

#[test]
fn test_xdg_to_var_is_blocked() {
    with_test_env("/home/testuser", Some("/var/lib"), || {
        let dir = resolve_runtime_dir();
        assert_eq!(dir, PathBuf::from("/tmp"));
    });
}

#[test]
fn test_xdg_to_proc_sys_dev_blocked() {
    with_test_env("/home/testuser", None, || {
        for p in [
            "/proc", "/sys", "/dev", "/boot", "/root", "/usr", "/bin", "/sbin", "/lib",
        ] {
            // SAFETY (night-improve-4, edition 2024): mutation under
            // the held ENV_LOCK — see `with_test_env`.
            unsafe {
                std::env::set_var("XDG_RUNTIME_DIR", p);
            }
            assert_eq!(
                resolve_runtime_dir(),
                PathBuf::from("/tmp"),
                "XDG_RUNTIME_DIR={p} must fall back to /tmp"
            );
        }
    });
}

#[test]
fn test_dangerous_paths_detected() {
    with_test_env("/home/testuser", None, || {
        assert!(is_dangerous(Path::new("/etc/passwd")));
        assert!(is_dangerous(Path::new("/etc/zylaxion.sock")));
        assert!(is_dangerous(Path::new("/usr/bin/bash")));
        assert!(is_dangerous(Path::new("/var/log/x")));
        assert!(is_dangerous(Path::new("/boot/vmlinuz")));
        assert!(is_dangerous(Path::new("/root/.bashrc")));
        assert!(is_dangerous(Path::new("/proc/self/status")));
        assert!(is_dangerous(Path::new("/sys/kernel")));
        assert!(is_dangerous(Path::new("/dev/null")));
        assert!(is_dangerous(Path::new("/home/testuser/.ssh/id_rsa")));
        assert!(is_dangerous(Path::new("/home/testuser/.gnupg/secring.gpg")));
        // Not dangerous — valid runtime locations
        assert!(!is_dangerous(Path::new("/run/user/1000")));
        assert!(!is_dangerous(Path::new("/tmp")));
        assert!(!is_dangerous(Path::new(
            "/home/testuser/.local/share/zylaxion"
        )));
    });
}

#[test]
fn test_lexical_normalize_collapses_dots() {
    let p = Path::new("/run/user/1000/../1001/zylaxion");
    let n = lexical_normalize(p).unwrap();
    assert_eq!(n, PathBuf::from("/run/user/1001/zylaxion"));
}
