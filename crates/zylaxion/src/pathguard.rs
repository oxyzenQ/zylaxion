// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Path security guard for zylaxion's runtime state I/O.
//!
//! ## Scope
//!
//! Zylaxion writes three runtime files: a lock file, a Unix socket, and a
//! PID file. All three live under `$XDG_RUNTIME_DIR` (or `/tmp` as
//! fallback). Without this guard, a malicious or careless environment
//! could set `XDG_RUNTIME_DIR=/etc` and cause zylaxion to write socket
//! and PID files into protected directories.
//!
//! ## Policy
//!
//! The runtime directory must NOT resolve to a known-dangerous system
//! path. When `XDG_RUNTIME_DIR` points to a dangerous location, this
//! module falls back to `/tmp` (the same behavior as when
//! `XDG_RUNTIME_DIR` is unset) rather than refusing to start — the
//! daemon needs to run, but it must not write state into `/etc`, `/usr`,
//! `~/.ssh`, etc.
//!
//! Config file reads from `/etc/zylaxion/` and `/usr/local/share/zylaxion/`
//! are BY DESIGN (system-wide config) and are NOT gated here — those are
//! read-only paths with fixed names, not user-controlled arbitrary paths.

use std::path::{Component, Path, PathBuf};

/// Lexically normalize a path: collapse `.` and `..` without requiring
/// the file to exist. Does NOT follow symlinks.
fn lexical_normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    let _ = out.pop();
                }
                Some(Component::RootDir) | None => {}
                _ => {
                    let _ = out.pop();
                }
            },
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Canonicalize for comparison — tries `std::fs::canonicalize`, falls
/// back to lexical normalization for paths that don't exist yet.
fn canonical_for_compare(path: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(path) {
        return c;
    }
    lexical_normalize(path).unwrap_or_else(|| path.to_path_buf())
}

/// Return true if the canonical path falls inside a known-dangerous prefix
/// that zylaxion's runtime state must NEVER touch — even if
/// `XDG_RUNTIME_DIR` points there.
fn is_dangerous(canonical: &Path) -> bool {
    let s = canonical.to_string_lossy();
    let home = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).to_string_lossy().into_owned())
        .unwrap_or_default();

    // System paths — never valid for zylaxion runtime state, even if
    // XDG_RUNTIME_DIR points here. Zylaxion is a user-space desktop app;
    // its runtime state must live in /run/user/$UID or /tmp, NOT in /etc,
    // /usr, /var, etc.
    let deny_prefixes: &[&str] = &[
        "/etc", "/usr", "/var", "/bin", "/sbin", "/lib", "/lib64", "/boot", "/root", "/proc",
        "/sys", "/dev",
    ];

    for prefix in deny_prefixes {
        // Match exact path OR path with trailing slash OR path starting
        // with prefix + "/" (e.g. "/etc" matches, "/etc/" matches, "/etc/zylaxion.sock" matches)
        if s == *prefix || s.starts_with(&format!("{prefix}/")) {
            return true;
        }
    }

    // User credential stores — never valid for runtime state
    let user_deny_subdirs: &[&str] = &[".ssh", ".gnupg", ".kwallet", ".local/share/keyrings"];
    if !home.is_empty() {
        for sub in user_deny_subdirs {
            let full = format!("{home}/{sub}");
            if s == full || s.starts_with(&format!("{full}/")) {
                return true;
            }
        }
    }

    false
}

/// Resolve the safe runtime directory for zylaxion state files.
///
/// Returns the canonicalized, validated directory path. If
/// `$XDG_RUNTIME_DIR` is set and safe, uses it. If it's set but
/// dangerous (e.g. `/etc`), falls back to `/tmp`. If it's unset,
/// uses `/tmp`.
pub fn resolve_runtime_dir() -> PathBuf {
    let candidate: PathBuf =
        if let Some(xdg) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
            PathBuf::from(&xdg)
        } else {
            PathBuf::from("/tmp")
        };

    let canonical = canonical_for_compare(&candidate);
    if is_dangerous(&canonical) {
        // Fall back to /tmp — don't refuse to start, but don't write
        // state into the dangerous location either.
        PathBuf::from("/tmp")
    } else {
        canonical
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialize env-mutating tests.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_test_env<F: FnOnce()>(home: &str, xdg_runtime: Option<&str>, f: F) {
        // Recover from poison if a previous test panicked while holding the lock.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old_home = std::env::var_os("HOME");
        let old_xdg = std::env::var_os("XDG_RUNTIME_DIR");

        std::env::set_var("HOME", home);
        match xdg_runtime {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }

        f();

        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        match old_xdg {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
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
                std::env::set_var("XDG_RUNTIME_DIR", p);
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
}
