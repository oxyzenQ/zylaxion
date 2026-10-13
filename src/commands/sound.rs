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

use std::process;
use std::time::Duration;
#[cfg(feature = "ebpf")]
use std::time::Instant;

#[cfg(feature = "ebpf")]
use aya::maps::{Array, MapData};

#[cfg(feature = "ebpf")]
use crate::ebpf::pin;
use crate::engine::VoicePool;

// ── Control slot constants ──────────────────────────────────────────────
// The userspace mirror of control_slots in ebpf/src/main.rs (and the
// loader's and player's aliases) — the loader's test tree pins the
// cross-side contract.

const ENABLED_SLOT: u32 = 0;
const PRESET_ID_SLOT: u32 = 1;
const VOLUME_PCT_SLOT: u32 = 2;
const PLAYER_PID_SLOT: u32 = 3;
const STATE_SLOT: u32 = 4;
/// The attach-chain step that owns the live tap (night-improve-25):
/// the mirror of `control::TAP_KIND` in the loader (and, behind it,
/// control_slots::TAP_KIND on the ebpf side).
const TAP_KIND_SLOT: u32 = 5;
const STATE_IDLE: u32 = 0;
const STATE_RUNNING: u32 = 2;

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

/// Liveness probe for the player pid.
fn pid_alive(pid: i32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_ok()
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

/// The volume percent the session context would pick: `ZYLAXION_VOLUME`
/// (a linear gain, the night-improve-22 contract) mapped to percent of
/// the compiled default, or 100. Malformed and out-of-domain values are
/// hard errors — the no-silent-fallback law — so `on` cannot smuggle a
/// louder session in through the env back door.
fn context_volume_percent() -> Result<u32, String> {
    volume_percent_from_gain(crate::session::preset::master_volume()?)
}

/// The pure core of [`context_volume_percent`]: gain in, percent (or
/// the refusal) out. All decision and error formatting lives here so
/// the test tree pins the cap without touching the process
/// environment (the resolve_from precedent).
fn volume_percent_from_gain(gain: f32) -> Result<u32, String> {
    let percent = (gain / VoicePool::DEFAULT_MASTER_VOLUME * 100.0).round();
    if percent < 0.0 {
        return Err(format!(
            "volume gain {gain} maps to {percent}% — below silence; \
             check {VOLUME_ENV_VAR}",
            VOLUME_ENV_VAR = crate::session::preset::VOLUME_ENV_VAR
        ));
    }
    if percent > crate::session::preset::MAX_VOLUME_PERCENT as f32 {
        return Err(format!(
            "volume gain {gain} maps to {percent}% — above the {}% safety cap \
             (100 = the compiled default; check {VOLUME_ENV_VAR})",
            crate::session::preset::MAX_VOLUME_PERCENT,
            VOLUME_ENV_VAR = crate::session::preset::VOLUME_ENV_VAR
        ));
    }
    Ok(percent as u32)
}

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

/// Handle `zylaxion on [preset]` — enable the sound.
#[cfg(feature = "ebpf")]
pub fn cmd_on(cli_preset: Option<String>) {
    // The privilege verdict (night-improve-26): the parent owns the
    // door — a root `on` refuses before it resolves paths, opens maps,
    // or spawns anything. Same wording the player-side guard prints;
    // a root player would open audio in root's session, which no user
    // desktop shares.
    if let Some(refusal) = on_root_refusal(crate::session::privilege::euid_is_root()) {
        crate::output::error(refusal);
        process::exit(1);
    }

    init_logger();

    // The kernel gate: one probe, the same verdicts the doctor prints.
    let kernel = crate::session::kernel::probe();
    if !kernel.supported() {
        crate::output::error(format!(
            "kernel eBPF verdict: {} — zylaxion is eBPF-only, upgrade the kernel \
             (see `zylaxion doctor` for the full verdict)",
            kernel.verdict()
        ));
        process::exit(1);
    }
    // The kernel-policy gate (night-badas-4 round 5): the classified
    // refusal beats the misleading not-attached shape an EPERM takes.
    kernel_policy_gate();

    let mut control = match open_control() {
        Ok(c) => c,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };

    // Resolve the session preset: CLI flag > ZYLAXION_PRESET > default.
    // The resolver's gain is the ZYLAXION_VOLUME linear factor; the
    // percent mirror below is what the control map stores.
    let (_profile_unused_here, preset_name, _gain) =
        match crate::session::preset::resolve(cli_preset.as_deref()) {
            Ok(r) => r,
            Err(e) => {
                crate::output::error(e);
                process::exit(1);
            }
        };
    let preset_index = match preset_index_or_error(preset_name) {
        Ok(i) => i,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    let volume_percent = match context_volume_percent() {
        Ok(v) => v,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    // Write the control plane BEFORE spawning: the player reads its
    // initial posture from the map at startup.
    if let Err(e) = control.set(PRESET_ID_SLOT, preset_index, 0) {
        crate::output::error(format!("cannot write the preset slot: {e}"));
        process::exit(1);
    }
    if let Err(e) = control.set(VOLUME_PCT_SLOT, volume_percent, 0) {
        crate::output::error(format!("cannot write the volume slot: {e}"));
        process::exit(1);
    }

    // Already playing? A live player needs no spawn — the map writes
    // above already applied (and enabled comes below).
    let existing_pid = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0);
    let already_running = existing_pid != 0
        && pid_alive(existing_pid as i32)
        && control.get(&STATE_SLOT, 0).unwrap_or(0) == STATE_RUNNING;
    if already_running {
        let _ = control.set(ENABLED_SLOT, 1, 0);
        // night-improve-27 wording: the map writes above already applied,
        // so this surface is an UPDATE, not a spawn — say so, and keep
        // the contrast with the fresh-spawn line below ("new player").
        println!(
            "sound already on: preset {preset_name}, volume {volume_percent}% \
             (player pid {existing_pid} kept — preset updated live)"
        );
        return;
    }

    // The orphan sweep (night-improve-27): belt under attach's
    // suspenders — the spawn must be single from ANY state, so a
    // player that outlived its map dies here before a new one is
    // born. Exact-match scan; a survivor refuses with the pid.
    if let Err(e) = crate::session::orphan::reap_orphans() {
        crate::output::error(e);
        process::exit(1);
    }

    // Clear any stale registration, then spawn the stateless player:
    // same binary, hidden verb, detached into its own process group, no
    // stdio worth keeping except stderr (startup errors stay visible).
    let _ = control.set(STATE_SLOT, STATE_IDLE, 0);
    let _ = control.set(PLAYER_PID_SLOT, 0, 0);
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            crate::output::error(format!("cannot resolve this binary's path: {e}"));
            process::exit(1);
        }
    };
    use std::os::unix::process::CommandExt as _;
    let spawn = std::process::Command::new(exe)
        .arg(PLAYER_SPAWN_VERB)
        .stdin(process::Stdio::null())
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::inherit())
        .process_group(0)
        .spawn();
    let mut child = match spawn {
        Ok(c) => c,
        Err(e) => {
            crate::output::error(format!("failed to spawn the player: {e}"));
            process::exit(1);
        }
    };

    // Enable the gate BEFORE waiting: the tap only rings when enabled=1,
    // and the player's readiness race loses nothing either way.
    if let Err(e) = control.set(ENABLED_SLOT, 1, 0) {
        crate::output::error(format!("cannot enable the sound: {e}"));
        let _ = child.kill();
        process::exit(1);
    }

    // Wait for the player to report running (or die trying).
    let started = Instant::now();
    loop {
        if child.try_wait().map(|s| s.is_some()).unwrap_or(true) {
            crate::output::error(
                "the player exited during startup\nrun `zylaxion doctor` for the \
                 environment verdicts, or run `zylaxion _play` in a terminal to \
                 see the player's own output",
            );
            let _ = control.set(ENABLED_SLOT, 0, 0);
            process::exit(1);
        }
        if control.get(&STATE_SLOT, 0).unwrap_or(0) == STATE_RUNNING {
            break;
        }
        if started.elapsed() >= PLAYER_SETTLE_BUDGET {
            crate::output::error(
                "the player did not reach the running state in time\nrun \
                 `zylaxion doctor` for the environment verdicts, or run \
                 `zylaxion _play` in a terminal to see the player's own output",
            );
            let _ = child.kill();
            let _ = control.set(ENABLED_SLOT, 0, 0);
            process::exit(1);
        }
        std::thread::sleep(SETTLE_STEP);
    }
    let child_pid = child.id();
    drop(child);

    println!(
        "sound on: preset {preset_name}, volume {volume_percent}% (new player pid {child_pid}, \
         detached; `zylaxion off` to stop)"
    );
}

/// Handle `zylaxion off` — silence. The player self-exits within its
/// 100 ms watcher cadence; the tap and the pins STAY attached (the next
/// `on` is instant and needs no sudo). `zylaxion detach` removes the
/// kernel side entirely.
#[cfg(feature = "ebpf")]
pub fn cmd_off() {
    init_logger();

    // The kernel-policy gate, only when a tap actually lives in the
    // kernel (night-badas-4 round 5): the pin check is filesystem-
    // level, so the idempotent shape below stays posture-independent.
    if pin::any_pin_present() {
        kernel_policy_gate();
    }

    let mut control = match open_control() {
        Ok(c) => c,
        Err(_) => {
            // Not attached: off is idempotent — nothing to stop is a
            // success shape, not an error.
            println!("sound is off (the tap is not attached — nothing to stop)");
            return;
        }
    };

    let pid = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0);
    if pid == 0 || !pid_alive(pid as i32) {
        // No live player: clear the gate and any stale registration.
        let _ = control.set(ENABLED_SLOT, 0, 0);
        let _ = control.set(PLAYER_PID_SLOT, 0, 0);
        let _ = control.set(STATE_SLOT, STATE_IDLE, 0);
        println!("sound is off (no player was running)");
        return;
    }

    // The stop protocol: clear the gate, the player's watcher sees it
    // within 100 ms, fades out, and unregisters itself.
    if let Err(e) = control.set(ENABLED_SLOT, 0, 0) {
        crate::output::error(format!("cannot clear the enabled flag: {e}"));
        process::exit(1);
    }
    let started = Instant::now();
    while pid_alive(pid as i32) {
        if started.elapsed() >= PLAYER_SETTLE_BUDGET {
            // The player ignored the gate (wedged audio thread?): the
            // honest report, with the manual escape hatch.
            crate::output::warning(format!(
                "the player (pid {pid}) is still alive after the stop budget — \
                 kill -TERM {pid} if it stays wedged"
            ));
            println!("sound off (stop pending)");
            return;
        }
        std::thread::sleep(SETTLE_STEP);
    }
    println!("sound off (no processes remain; the tap stays attached)");
}

/// Handle `zylaxion preset --switch <name>` — one map write, live
/// swap (night-improve-24: the flag is `--switch`, renamed from the
/// silent `--name`).
#[cfg(feature = "ebpf")]
pub fn cmd_preset(name: &str) {
    init_logger();
    kernel_policy_gate();
    let preset_index = match preset_index_or_error(name) {
        Ok(i) => i,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    let mut control = match open_control() {
        Ok(c) => c,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    if let Err(e) = control.set(PRESET_ID_SLOT, preset_index, 0) {
        crate::output::error(format!("cannot write the preset slot: {e}"));
        process::exit(1);
    }
    let playing = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0).ne(&0)
        && pid_alive(control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0) as i32);
    if playing {
        println!("preset set to {name} (swapped live)");
    } else {
        println!("preset set to {name} (takes effect on `zylaxion on`)");
    }
}

/// Handle `zylaxion volume <percent>` — one map write, live retune.
/// 100 percent is the compiled laptop-speaker default.
#[cfg(feature = "ebpf")]
pub fn cmd_volume(percent: u32) {
    init_logger();
    kernel_policy_gate();
    let mut control = match open_control() {
        Ok(c) => c,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    if let Err(e) = control.set(VOLUME_PCT_SLOT, percent, 0) {
        crate::output::error(format!("cannot write the volume slot: {e}"));
        process::exit(1);
    }
    println!("volume set to {percent}% (100 = the compiled default)");
}

/// Handle `zylaxion status [--verbose]` — the one-screen truth:
/// kernel verdict, tap residency, player liveness, and the active
/// posture. The verbose flag appends the kernel-side typing
/// telemetry (the merged `stats` surface, night-improve-24) — a
/// static print-once deep read, never a live refresh: the deleted
/// `--live` loop was the wrong shape for a status surface.
#[cfg(feature = "ebpf")]
pub fn cmd_status(verbose: bool) {
    let kernel = crate::session::kernel::probe();
    println!("kernel:   {} ({})", kernel.release, kernel.verdict());

    if !pin::any_pin_present() {
        println!("tap:      not attached (sudo zylaxion attach to plant it)");
        return;
    }
    if !pin::all_pins_present() {
        crate::output::warning(
            "the pin set is partial (a crashed attach?) — sudo zylaxion attach replants it",
        );
    }
    // The kernel-policy note (night-badas-4 round 5): a locked
    // unprivileged-bpf kernel refuses the map reads too — name why.
    if !crate::session::privilege::euid_is_root() && !kernel.unpriv_bpf_allowed() {
        crate::output::warning(kernel.unpriv_bpf_verdict());
    }
    // Open the control map once: the tap line below reads the chain
    // step from it, the posture block reads the rest.
    let control = open_control();

    // Which chain step owns the live tap (night-improve-25): the
    // TAP_KIND control slot records fentry:input_event /
    // kprobe:input_event / kprobe:input_handle_event at attach. A slot
    // this build cannot parse (a newer binary attached this tap?)
    // renders honestly as unknown instead of guessing.
    let tap_word = match control
        .as_ref()
        .ok()
        .and_then(|c| c.get(&TAP_KIND_SLOT, 0).ok())
        .and_then(crate::ebpf::loader::TapKind::from_slot)
    {
        Some(kind) => kind.label().to_string(),
        None => "an unknown chain step (a newer binary attached this tap?)".to_string(),
    };
    println!(
        "tap:      attached via {tap_word} ({} pins under {})",
        pin::ALL_PINS.len(),
        pin::PIN_DIR
    );

    let Ok(control) = control else {
        crate::output::warning("the pinned control map is not openable by this user");
        return;
    };
    let enabled = control.get(&ENABLED_SLOT, 0).unwrap_or(0) == 1;
    let preset_index = control.get(&PRESET_ID_SLOT, 0).unwrap_or(0);
    let preset_name = crate::profiles::registry_all()
        .get(preset_index as usize)
        .map(|e| e.name)
        .unwrap_or("<unknown>");
    let volume = control.get(&VOLUME_PCT_SLOT, 0).unwrap_or(100);
    let pid = control.get(&PLAYER_PID_SLOT, 0).unwrap_or(0);
    let state = control.get(&STATE_SLOT, 0).unwrap_or(0);

    println!("sound:    {}", if enabled { "on" } else { "off" });
    println!("preset:   {preset_name}");
    println!("volume:   {volume}% (100 = the compiled default)");
    match (pid, pid_alive(pid as i32)) {
        (0, _) => println!("player:   not running"),
        (p, true) => {
            let state_word = match state {
                STATE_RUNNING => "running",
                STATE_IDLE => "idle",
                1 => "starting",
                3 => "failed",
                _ => "unknown",
            };
            println!("player:   running (pid {p}, state: {state_word})");
        }
        (p, false) => println!("player:   stale pid {p} in the map (next `on` takes over)"),
    }
    if verbose {
        crate::commands::stats::print_telemetry();
    }
    println!();
    println!(
        "introspection: bpftool map dump pinned {pin}",
        pin = pin::PIN_MAP_CONTROL
    );
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
pub fn cmd_volume(_percent: u32) {
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
#[path = "../../test/commands/sound_tests.rs"]
mod tests;
