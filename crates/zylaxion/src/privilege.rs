// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Privilege guard — the root-usage audit (night-improve-16).
//!
//! The sibling of [`crate::pathguard`]: pathguard pins WHERE runtime
//! state may live, this module pins WHO may run the privileged
//! surfaces. Both exist because zylaxion is a user-session program —
//! every byte of state it writes (lock, socket, PID) belongs inside
//! the invoking user's session, and every hazard it guards (input
//! grab, audio device) is per-session.
//!
//! # The audit verdict, surface by surface
//!
//! | Surface          | Verdict under euid 0 | Why |
//! |------------------|----------------------|-----|
//! | `--check-update` | REFUSE               | privileged network round-trip that buys nothing; root-owned files if a download step ever joins |
//! | `start`, `daemon`| REFUSE               | plants root-owned lock/socket/PID inside the user's runtime dir — the next user-mode run then fails on flock/bind (the fatal pollution), plus a root input grab and a root audio session no user desktop mixes with |
//! | `stop`, `status` | allowed              | the rescue path — must stay reachable to stop a foreign-session daemon; read-only against session state |
//! | `doctor`         | warn                 | the verdicts describe root's session (root is always "in" the input group, root's audio is a different server) — diagnosed as misleading, not fatal |
//! | everything else  | allowed              | read-only config/preset surfaces, no state, no network |
//!
//! The refusal wordings are pure functions of the euid fact so both
//! the decision and the wording are unit-pinned below — the same
//! drift-pin discipline as the zelynic reference contract
//! ("do not use sudo for check download").

/// The euid fact every refusal rides. `true` when the process runs
/// with effective uid 0 — sudo, su, a root login shell, or a root
/// systemd unit all land here, which is the point: the guard keys on
/// the privilege itself, not on how it was obtained.
pub fn euid_is_root() -> bool {
    nix::unistd::geteuid().is_root()
}

/// Refusal wording for the network surface (`--check-update`).
///
/// The zelynic reference contract, verbatim: the check is a plain
/// network fetch (curl against the GitHub API) — running it as root
/// is a privileged network round-trip that buys nothing, and any
/// future download step would plant root-owned files into the
/// invoking user's home. The guard fires before any network I/O.
pub fn network_refusal(euid_is_root: bool) -> Option<&'static str> {
    if euid_is_root {
        Some(
            "root refused — update check performs a network fetch, do not use sudo\n  \
             tip: re-run without sudo",
        )
    } else {
        None
    }
}

/// Refusal wording for the daemon surfaces (`start`, `daemon`).
///
/// The mirror image of the network refusal: here root is not a
/// wasted privilege but a polluting one. The daemon writes its lock,
/// socket, and PID files into `$XDG_RUNTIME_DIR` — under sudo that
/// directory is still the invoking user's, so the files materialize
/// as root-owned entries inside a user-owned session: the next
/// user-mode start fails on the flock or the socket bind with no
/// repair hint. A root daemon also grabs every input device
/// system-wide and opens audio in root's session, which no user
/// desktop shares. The guard fires before the instance lock —
/// acquiring the lock as root IS the pollution event.
pub fn session_refusal(euid_is_root: bool) -> Option<&'static str> {
    if euid_is_root {
        Some(
            "root refused — the daemon plants root-owned session state (lock, socket, PID) \
             and grabs input devices as root, do not use sudo\n  \
             tip: re-run without sudo",
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift pin (the zelynic NIGHT-hunt-11 shape): the network
    /// refusal carries the owner-facing verdict ("do not use sudo"),
    /// the reason (a network fetch as root), and the fix (re-run
    /// without sudo).
    #[test]
    fn network_refusal_carries_the_do_not_use_sudo_contract() {
        let msg = network_refusal(true).expect("euid 0 must be refused");
        assert!(
            msg.contains("do not use sudo"),
            "refusal must carry the do-not-use-sudo verdict, got: {msg}"
        );
        assert!(
            msg.contains("network fetch"),
            "refusal must say why: network fetch as root, got: {msg}"
        );
        assert!(
            msg.contains("tip: re-run without sudo"),
            "refusal must carry the fix tip, got: {msg}"
        );
    }

    /// The network surface allows every non-root invocation.
    #[test]
    fn network_refusal_allows_non_root() {
        assert!(
            network_refusal(false).is_none(),
            "non-root must pass the guard"
        );
    }

    /// Drift pin for the daemon surfaces: the session refusal names
    /// the pollution (root-owned session state), the grab (input
    /// devices), and the fix.
    #[test]
    fn session_refusal_names_pollution_and_grab() {
        let msg = session_refusal(true).expect("euid 0 must be refused");
        assert!(
            msg.contains("do not use sudo"),
            "refusal must carry the do-not-use-sudo verdict, got: {msg}"
        );
        assert!(
            msg.contains("session state") && msg.contains("input devices"),
            "refusal must name the pollution and the grab, got: {msg}"
        );
        assert!(
            msg.contains("tip: re-run without sudo"),
            "refusal must carry the fix tip, got: {msg}"
        );
    }

    /// The session surface allows every non-root invocation.
    #[test]
    fn session_refusal_allows_non_root() {
        assert!(
            session_refusal(false).is_none(),
            "non-root must pass the guard"
        );
    }
}
