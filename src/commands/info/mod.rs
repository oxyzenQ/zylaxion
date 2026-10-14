// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Information subcommands: `doctor`, `list-presets`, `list-backends`,
//! and the no-subcommand `overview`.
//!
//! night-improve-22: the `testconf` subcommand died with `config.toml`
//! — there is no file left to validate. The compiled preset registry
//! is type-checked Rust data, proven in-range by the registry test
//! tree; a validator command for it would validate nothing.
//!
//! Layout (the LOC law's dir/mod.rs shape, night-hunter-5): one theme
//! per file — the ALSA stderr guard, the doctor report, the compiled
//! catalog surfaces, the no-subcommand overview — with the shared
//! active-preset decision here, in the module the test tree pins.

mod alsa_guard;
mod catalog;
mod doctor;
mod overview;

pub use catalog::{cmd_list_backends, cmd_list_presets};
pub use doctor::cmd_doctor;
pub use overview::cmd_overview;

use crate::session::preset;

// The tests pin the table render; the render lives in catalog.rs.
#[cfg(test)]
use catalog::presets_table_from;

/// The live session's preset name, read straight from the pinned
/// control map (night-hunter-2): `Some` only when a live player holds
/// the plane — a pid of zero or a stale/zombie pid answers `None`,
/// because a dead session's leftover slot must not masquerade as the
/// active pick (the next `on` re-resolves the slot anyway).
#[cfg(feature = "ebpf")]
pub(super) fn live_session_preset_name() -> Option<&'static str> {
    let control = crate::commands::sound::open_control().ok()?;
    let pid = control
        .get(&crate::commands::sound::PLAYER_PID_SLOT, 0)
        .unwrap_or(0);
    if pid == 0 || !crate::session::orphan::pid_alive(pid as i32) {
        return None;
    }
    let index = control
        .get(&crate::commands::sound::PRESET_ID_SLOT, 0)
        .unwrap_or(0);
    crate::profiles::registry_all()
        .get(index as usize)
        .map(|e| e.name)
}

/// The dormant lane of the live read (night-hunter-2): a binary built
/// without the eBPF loader has no control plane to consult, so no live
/// session can ever answer.
#[cfg(not(feature = "ebpf"))]
pub(super) fn live_session_preset_name() -> Option<&'static str> {
    None
}

/// The pure core of the active-name decision (night-hunter-2): the
/// live session's preset when one holds the control plane, the
/// would-start resolution otherwise. All decision logic lives here so
/// the test tree pins it without a control map or a process
/// environment (the resolve_from precedent).
fn active_preset_name_from(live: Option<&'static str>, preset_env: Option<&str>) -> &'static str {
    match live {
        Some(name) => name,
        None => preset::context_preset_name_from(preset_env),
    }
}

/// The env-reading shell around [`active_preset_name_from`] (the
/// thin-shell precedent — see [`preset::context_preset_name`]).
pub(super) fn active_preset_name(live: Option<&'static str>) -> &'static str {
    let preset_env = std::env::var(preset::PRESET_ENV_VAR).ok();
    active_preset_name_from(live, preset_env.as_deref())
}

#[cfg(test)]
#[path = "../../../test/commands/info_tests.rs"]
mod tests;
