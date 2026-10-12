// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The sound verbs: `on`, `off`, `preset`, `volume`, `stats`, `status`
//! (night-improve-23, the daemonless control plane).
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
//! zylaxion preset tpt      one map write — the playing preset swaps live
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
/// the compiled default, or 100. A malformed value is a hard error —
/// the no-silent-fallback law.
fn context_volume_percent() -> Result<u32, String> {
    let gain = crate::session::preset::master_volume()?;
    let percent = (gain / VoicePool::DEFAULT_MASTER_VOLUME * 100.0).round();
    if percent < 0.0 || percent > 10_000.0 {
        return Err(format!(
            "volume gain {gain} maps outside the percent domain (0..=10000) — \
             check {ZYLAXION_VOLUME_HELP}",
            ZYLAXION_VOLUME_HELP = crate::session::preset::VOLUME_ENV_VAR
        ));
    }
    Ok(percent as u32)
}

/// Handle `zylaxion on [preset]` — enable the sound.
#[cfg(feature = "ebpf")]
pub fn cmd_on(cli_preset: Option<String>) {
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
        println!(
            "sound on: preset {preset_name}, volume {volume_percent}% (player pid {existing_pid}, updated live)"
        );
        return;
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
        .arg("_play")
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
                "the player exited during startup — run `zylaxion _play` in the \
                 foreground for the full error",
            );
            let _ = control.set(ENABLED_SLOT, 0, 0);
            process::exit(1);
        }
        if control.get(&STATE_SLOT, 0).unwrap_or(0) == STATE_RUNNING {
            break;
        }
        if started.elapsed() >= PLAYER_SETTLE_BUDGET {
            crate::output::error(
                "the player did not reach the running state in time — run \
                 `zylaxion _play` in the foreground for the full error",
            );
            let _ = child.kill();
            let _ = control.set(ENABLED_SLOT, 0, 0);
            process::exit(1);
        }
        std::thread::sleep(SETTLE_STEP);
    }
    drop(child);

    println!(
        "sound on: preset {preset_name}, volume {volume_percent}% (player detached; `zylaxion off` to stop)"
    );
}

/// Handle `zylaxion off` — silence. The player self-exits within its
/// 100 ms watcher cadence; the tap and the pins STAY attached (the next
/// `on` is instant and needs no sudo). `zylaxion detach` removes the
/// kernel side entirely.
#[cfg(feature = "ebpf")]
pub fn cmd_off() {
    init_logger();

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

/// Handle `zylaxion preset <name>` — one map write, live swap.
#[cfg(feature = "ebpf")]
pub fn cmd_preset(name: &str) {
    init_logger();
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

/// Handle `zylaxion stats` — the kernel-side typing telemetry.
///
/// The tap accumulated per-keycode press counters in the stats map at
/// zero userspace cost; this reads them and renders the top keys with
/// their evdev names.
#[cfg(feature = "ebpf")]
pub fn cmd_stats() {
    let data = match MapData::from_pin(pin::PIN_MAP_STATS) {
        Ok(d) => d,
        Err(e) => {
            crate::output::error(format!(
                "the tap is not attached (cannot open {}): {e}\n  \
                 tip: sudo zylaxion attach",
                pin::PIN_MAP_STATS
            ));
            process::exit(1);
        }
    };
    let owned = aya::maps::Map::Array(data);
    let stats: Array<MapData, u64> = match Array::try_from(owned) {
        Ok(s) => s,
        Err(e) => {
            crate::output::error(format!("the pinned stats map is not an array: {e}"));
            process::exit(1);
        }
    };

    let total_slot = 256u32;
    let total = stats.get(&total_slot, 0).unwrap_or(0);
    let mut rows: Vec<(u32, u64)> = Vec::new();
    for code in 0..255u32 {
        let count = stats.get(&code, 0).unwrap_or(0);
        if count > 0 {
            rows.push((code, count));
        }
    }
    rows.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    println!("Keyboard stats (accumulated in the kernel since attach):\n");
    println!("  total presses: {total}");
    if rows.is_empty() {
        println!("  no presses recorded yet — type something!");
        return;
    }
    println!("\n  top keys:");
    for (code, count) in rows.iter().take(10) {
        println!("  {:>8}  keycode {code:>3} ({})", count, key_name(*code));
    }
}

/// Handle `zylaxion status` — the one-screen truth: kernel verdict, tap
/// residency, player liveness, and the active posture.
#[cfg(feature = "ebpf")]
pub fn cmd_status() {
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
    println!(
        "tap:      attached ({} pins under {})",
        pin::ALL_PINS.len(),
        pin::PIN_DIR
    );

    let Ok(control) = open_control() else {
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
    println!();
    println!(
        "introspection: bpftool map dump pinned {pin}",
        pin = pin::PIN_MAP_CONTROL
    );
}

/// A compact evdev key name for the stats render. The common keyboard
/// keys carry real names; everything else renders as key<N> — the raw
/// count is the truth, the name is a courtesy.
fn key_name(code: u32) -> String {
    let common: &[(u32, &str)] = &[
        (1, "ESC"),
        (2, "1"),
        (3, "2"),
        (4, "3"),
        (5, "4"),
        (6, "5"),
        (7, "6"),
        (8, "7"),
        (9, "8"),
        (10, "9"),
        (11, "0"),
        (14, "BKSP"),
        (15, "TAB"),
        (16, "Q"),
        (17, "W"),
        (18, "E"),
        (19, "R"),
        (20, "T"),
        (21, "Y"),
        (22, "U"),
        (23, "I"),
        (24, "O"),
        (25, "P"),
        (26, "["),
        (27, "]"),
        (28, "ENTER"),
        (29, "LCTRL"),
        (30, "A"),
        (31, "S"),
        (32, "D"),
        (33, "F"),
        (34, "G"),
        (35, "H"),
        (36, "J"),
        (37, "K"),
        (38, "L"),
        (39, ";"),
        (40, "'"),
        (41, "`"),
        (42, "LSHIFT"),
        (43, "\\"),
        (44, "Z"),
        (45, "X"),
        (46, "C"),
        (47, "V"),
        (48, "B"),
        (49, "N"),
        (50, "M"),
        (51, ","),
        (52, "."),
        (53, "/"),
        (54, "RSHIFT"),
        (55, "KP*"),
        (56, "LALT"),
        (57, "SPACE"),
        (58, "CAPS"),
        (59, "F1"),
        (60, "F2"),
        (61, "F3"),
        (62, "F4"),
        (63, "F5"),
        (64, "F6"),
        (65, "F7"),
        (66, "F8"),
        (67, "F9"),
        (68, "F10"),
        (69, "NUM"),
        (70, "SCROLL"),
        (87, "F11"),
        (88, "F12"),
        (96, "KP-ENTER"),
        (97, "RCTRL"),
        (98, "KP/"),
        (100, "RALT"),
        (102, "HOME"),
        (103, "UP"),
        (104, "PGUP"),
        (105, "LEFT"),
        (106, "RIGHT"),
        (107, "END"),
        (108, "DOWN"),
        (109, "PGDN"),
        (110, "INS"),
        (111, "DEL"),
        (119, "PAUSE"),
    ];
    common
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| format!("key{code}"))
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

/// Dormant lane (cmd_stats): this binary was built without the eBPF
/// loader (--no-default-features) — the honest refusal, every time.
#[cfg(not(feature = "ebpf"))]
pub fn cmd_stats() {
    crate::output::error(crate::ebpf::dormant::DORMANT_REFUSAL);
    process::exit(1);
}

/// Dormant lane (cmd_status): this binary was built without the eBPF
/// loader (--no-default-features) — the honest refusal, every time.
#[cfg(not(feature = "ebpf"))]
pub fn cmd_status() {
    crate::output::error(crate::ebpf::dormant::DORMANT_REFUSAL);
    process::exit(1);
}
