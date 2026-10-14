// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The sound verbs: `on`, `off`, `preset`, `volume`, `status`
//! (night-improve-23, the daemonless control plane; night-improve-24
//! merged the `stats` verb into `status --verbose` — one master
//! status report, the deep read a flag away).
//!
//! Every verb here is a short-lived CLI invocation: it opens the pinned
//! control map, reads or writes a few slots, prints, and exits. The
//! kernel owns the state, so there is nothing to keep alive and nothing
//! to synchronize — the Unix configuration surface IS a map write now.
//!
//! The lifecycle in one read:
//!
//! ```text
//! sudo zylaxion attach     ONCE per boot: tap + pins live in the kernel
//! zylaxion on [preset]     spawn the stateless player, enable the gate
//! zylaxion preset --switch topre   one map write — the playing preset swaps live
//! zylaxion volume 60       one map write — the gain retunes live
//! zylaxion off             clear the gate: player self-exits, ZERO processes
//! zylaxion detach          (sudo) unpin everything: nothing in the kernel
//! ```
//!
//! Layout (the LOC law's dir/mod.rs shape, night-hunter-5): one theme
//! per file — the player lifecycle (on/off), the live map-write tuning
//! verbs (preset/volume), the status report — with the shared control
//! plane (slots, the map opener, the policy gates, the dormant
//! refusals) here.

use std::process;
use std::time::Duration;

#[cfg(feature = "ebpf")]
use aya::maps::{Array, MapData};

#[cfg(feature = "ebpf")]
use crate::ebpf::pin;

#[cfg(feature = "ebpf")]
mod lifecycle;
#[cfg(feature = "ebpf")]
mod status;
#[cfg(feature = "ebpf")]
mod tuning;

#[cfg(feature = "ebpf")]
pub use lifecycle::{cmd_off, cmd_on};
#[cfg(feature = "ebpf")]
pub use status::cmd_status;
#[cfg(feature = "ebpf")]
pub use tuning::{cmd_preset, cmd_volume};

// ── Control slot constants ──────────────────────────────────────────────
// The userspace mirror of control_slots in ebpf/src/main.rs (and the
// loader's and player's aliases) — the loader's test tree pins the
// cross-side contract. Crate-visible (night-hunter-2): the overview and
// the preset listing read the same plane through these named constants
// instead of raw slot numbers that can drift.

pub(crate) const ENABLED_SLOT: u32 = 0;
pub(crate) const PRESET_ID_SLOT: u32 = 1;
pub(crate) const VOLUME_PCT_SLOT: u32 = 2;
pub(crate) const PLAYER_PID_SLOT: u32 = 3;
pub(crate) const STATE_SLOT: u32 = 4;
/// The attach-chain step that owns the live tap (night-improve-25):
/// the mirror of `control::TAP_KIND` in the loader (and, behind it,
/// control_slots::TAP_KIND on the ebpf side). Crate-visible
/// (night-hunter-2): the overview reads the same slot through the
/// named constant.
pub(crate) const TAP_KIND_SLOT: u32 = 5;
/// The player state vocabulary (night-hunter-8): the full word list
/// the player writes and `status` reads — the writer side (player.rs)
/// carries its own private mirror of these per the established
/// private-alias shape, but every name now exists on both sides; the
/// failed state used to be an anonymous `3` in two files.
const STATE_IDLE: u32 = 0;
const STATE_STARTING: u32 = 1;
const STATE_RUNNING: u32 = 2;
const STATE_FAILED: u32 = 3;

/// The player-wait budget after spawning (and after `off`): the player
/// reaches the running state (or dies) well inside a second; 2 s is the
/// patient ceiling that still feels instant.
const PLAYER_SETTLE_BUDGET: Duration = Duration::from_secs(2);

/// The per-poll step while waiting on the player.
const SETTLE_STEP: Duration = Duration::from_millis(20);

/// Open the pinned control map for read-write. Every verb here needs
/// exactly this; the error names the fix (attach).
#[cfg(feature = "ebpf")]
pub(crate) fn open_control() -> Result<Array<MapData, u32>, String> {
    let data = MapData::from_pin(pin::PIN_MAP_CONTROL).map_err(|e| {
        format!(
            "the tap is not attached (cannot open {}): {e}\n  \
             tip: sudo zylaxion attach — once per boot, then all verbs are unprivileged",
            pin::PIN_MAP_CONTROL
        )
    })?;
    let owned = aya::maps::Map::Array(data);
    Array::try_from(owned).map_err(|e| format!("the pinned control map is not an array: {e}"))
}

/// One kernel-policy gate for the verbs that must talk to the pinned
/// control plane (night-badas-4 round 5): the classified refusal beats
/// the misleading "not attached" shape an EPERM takes on a kernel that
/// gates unprivileged bpf(). Root is never gated (capability-keyed).
#[cfg(feature = "ebpf")]
fn kernel_policy_gate() {
    let kernel = crate::session::kernel::probe();
    if let Some(refusal) =
        kernel.unpriv_map_write_refusal(crate::session::privilege::euid_is_root())
    {
        crate::output::error(refusal);
        process::exit(1);
    }
}

/// Registry index for a preset name, or the canonical did-you-mean
/// error (the night-improve-19 lineage — same shape the resolver uses).
#[cfg(feature = "ebpf")]
fn preset_index_or_error(name: &str) -> Result<u32, String> {
    let all = crate::profiles::registry_all();
    all.iter()
        .position(|e| e.name == name)
        .map(|i| i as u32)
        .ok_or_else(|| {
            let names = crate::profiles::registry_names();
            let mut msg = format!("Preset '{name}' not found. Available: {}", names.join(", "));
            if let Some(best) = crate::cli::suggestion::closest_value_match(name, &names) {
                msg.push_str(&format!("\ntip: did you mean '{best}'?"));
            }
            msg
        })
}

// The volume percent lane moved to the session layer (night-improve-27:
// the volume policy belongs beside MAX_VOLUME_PERCENT and the env-lane
// gain law in crate::session::preset). The re-export keeps the test
// tree's `use super::volume_percent_from_gain` resolving.
#[cfg(feature = "ebpf")]
pub(crate) use crate::session::preset::context_volume_percent;
#[cfg(all(test, feature = "ebpf"))]
pub(crate) use crate::session::preset::volume_percent_from_gain;

// ── The player wire (night-improve-26) ───────────────────────────────
// The exact string passed to Command::new when `on` execs the stateless
// player: an alias of the orphan sweep's match constant (night-improve-27)
// so the spawn wire and the /proc sweep cannot drift. The underscore
// prefix is the internal-verb convention, not a typo.
#[cfg(feature = "ebpf")]
pub(crate) const PLAYER_SPAWN_VERB: &str = crate::session::orphan::PLAYER_ARG;

/// The root refusal for the `on` surface (night-improve-26): the
/// parent owns the door, the child owns the audio. The verdict is the
/// same `session_refusal` wording the player-side guard prints — one
/// wording, two doors — so a root `on` fails here, BEFORE it spawns,
/// instead of failing inside the player after it does.
#[cfg(feature = "ebpf")]
pub(crate) fn on_root_refusal(euid_is_root: bool) -> Option<&'static str> {
    crate::session::privilege::session_refusal(euid_is_root)
}

/// Shared logger wiring (the night-improve-13 lane): the verbs log the
/// interesting transitions at info (preset swaps, retunes, stops).
fn init_logger() {
    use std::io::Write as _;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {:<5} {}] {}",
                crate::info::clock::iso_utc_millis(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
}

/// Dormant lane (cmd_on): this binary was built without the eBPF
/// loader (--no-default-features) — the honest refusal, every time.
#[cfg(not(feature = "ebpf"))]
pub fn cmd_on(_cli_preset: Option<String>) {
    crate::output::error(crate::ebpf::dormant::DORMANT_REFUSAL);
    process::exit(1);
}

/// Dormant lane (cmd_off): this binary was built without the eBPF
/// loader (--no-default-features) — the honest refusal, every time.
#[cfg(not(feature = "ebpf"))]
pub fn cmd_off() {
    crate::output::error(crate::ebpf::dormant::DORMANT_REFUSAL);
    process::exit(1);
}

/// Dormant lane (cmd_preset): this binary was built without the eBPF
/// loader (--no-default-features) — the honest refusal, every time.
#[cfg(not(feature = "ebpf"))]
pub fn cmd_preset(_name: &str) {
    crate::output::error(crate::ebpf::dormant::DORMANT_REFUSAL);
    process::exit(1);
}

/// Dormant lane (cmd_volume): this binary was built without the eBPF
/// loader (--no-default-features) — the honest refusal, every time.
#[cfg(not(feature = "ebpf"))]
pub fn cmd_volume(_percent: u32, _force_this: bool) {
    crate::output::error(crate::ebpf::dormant::DORMANT_REFUSAL);
    process::exit(1);
}

/// Dormant lane (cmd_status): this binary was built without the eBPF
/// loader (--no-default-features) — the honest refusal, every time.
#[cfg(not(feature = "ebpf"))]
pub fn cmd_status(_verbose: bool) {
    crate::output::error(crate::ebpf::dormant::DORMANT_REFUSAL);
    process::exit(1);
}

#[cfg(all(test, feature = "ebpf"))]
#[path = "../../../test/commands/sound_tests.rs"]
mod tests;
