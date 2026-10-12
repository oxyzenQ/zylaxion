// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Privilege guard — the root-usage audit (night-improve-16).
//!
//! The sibling of the retired pathguard (the daemon-era module that
//! pinned WHERE runtime state might live — night-improve-23 deleted it
//! with the session files it guarded): this module pins WHO may run the
//! privileged surfaces. It exists because zylaxion is a user-session
//! program — every byte of state it touches belongs to the kernel-side
//! pinned maps or the invoking user's environment, and every hazard it
//! guards (the BPF load, the audio device) is per-machine or
//! per-session.
//!
//! # The audit verdict, surface by surface
//!
//! | Surface          | Verdict under euid 0 | Why |
//! |------------------|----------------------|-----|
//! | `--check-update` | REFUSE               | privileged network round-trip that buys nothing; root-owned files if a download step ever joins |
//! | `start`, `daemon`| REFUSE               | plants root-owned lock/socket/PID inside the user's runtime dir — the next user-mode run then fails on flock/bind (the fatal pollution), plus a root input grab and a root audio session no user desktop mixes with |
//! | `attach`         | REQUIRE              | the inverse lane (night-improve-23): loading and pinning the tap needs CAP_BPF/CAP_PERFMON — root is the honest way in, and the pins are chowned to the invoking sudo user so every other surface stays unprivileged |
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

/// Requirement wording for the attach surface (`attach`).
///
/// The inverse lane of the refusals above (night-improve-23): attach is
/// the ONE surface that must run privileged — loading a BPF program and
/// pinning it needs CAP_BPF/CAP_PERFMON, and sudo is the honest way in.
/// The requirement is phrased as a refusal of the NON-root case: it
/// returns the refusal wording exactly when the passed fact is false,
/// and `None` under root — the caller passes the plain `euid_is_root()`
/// fact, the same positive polarity as the sibling refusals (this doc
/// once described the negated call as the contract; night-badas-1 fixed
/// the wiring that matched it and the tests now pin both polarities).
/// The pins are chowned to the invoking sudo user inside attach, so
/// this is the only privileged invocation the whole architecture
/// needs.
pub fn attach_requirement(euid_is_root: bool) -> Option<&'static str> {
    if !euid_is_root {
        Some(
            "attach must run as root — loading and pinning the eBPF tap needs \
             CAP_BPF/CAP_PERFMON\n  \
             tip: sudo zylaxion attach (the pins are handed to your user; every other \
             zylaxion command stays unprivileged)",
        )
    } else {
        None
    }
}

#[cfg(test)]
#[path = "../../test/session/privilege_tests.rs"]
mod tests;
