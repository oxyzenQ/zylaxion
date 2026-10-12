// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The tap loader — attach, init, pin; and detach (night-improve-23).
//!
//! This is the privileged one-shot that replaces daemon startup. It
//! loads the embedded tap object, initializes the control plane, attaches
//! the tracepoint, and pins EVERYTHING to bpffs — then the process may
//! exit, because the pinned link keeps the tap alive and the pinned maps
//! keep the state alive. After `attach` returns, nothing of zylaxion is
//! resident in userspace.
//!
//! Ownership model (the boring privilege story): attach runs as root
//! (`sudo zylaxion attach`), and when invoked through sudo the pins are
//! chowned to the invoking user — so the unprivileged player and CLI can
//! open the pinned maps afterwards (bpffs file permissions govern pinned
//! map access, the same posture systemd-tmpfiles-style boot units use).
//!
//! Idempotence and self-heal: a complete pin set reads as already
//! attached; a partial pin set (a crashed attach, a manual `rm`) is
//! cleaned and replanted — the stale-state forensics the old daemon
//! needed (PID files, /proc/exe identity checks) reduce to "pins exist
//? and are complete", because the kernel owns the state now.

use std::path::Path;

use aya::Ebpf;
use aya::maps::{Array, HashMap};
use aya::programs::TracePoint;

use super::bpf_syscall;
use super::embedded;
use super::pin;

/// The embedded pure-Rust tap object (night-improve-23): the aya-ebpf
/// ELF staged into OUT_DIR by build.rs — source-built on a git checkout,
/// registry-prebuilt on a crates.io extract; either way the same name,
/// either way it rides inside the binary.
static TAP_ELF: &[u8] =
    &embedded::AlignedElf::new(*include_bytes!(concat!(env!("OUT_DIR"), "/zylaxion-tap"))).bytes;

/// The control plane slot contract — the userspace mirror of
/// `control_slots` in ebpf/src/main.rs. Keep both sides in lockstep;
/// the ebpf crate's module doc carries the same table.
pub mod control {
    /// 1 while the sound is on. The kernel gate reads ONLY this slot.
    pub const ENABLED: u32 = 0;
    /// Registry index of the active preset (userspace-resolved).
    pub const PRESET_ID: u32 = 1;
    /// Master volume, percent of the compiled default gain.
    pub const VOLUME_PCT: u32 = 2;
    /// The player process pid (the "pidfile" lives in the kernel now).
    pub const PLAYER_PID: u32 = 3;
    /// Player lifecycle: 0 idle, 1 starting, 2 running, 3 failed.
    pub const STATE: u32 = 4;
}

/// The ring buffer record — layout contract with `KeyRingEvent` in
/// ebpf/src/main.rs (24 bytes: repr(C) puts ts_us at 0 and the three
/// u32s at 8/12/16, four trailing pad bytes — identical shape on both
/// sides is the contract). The player parses ring items into this
/// shape; until the player lane lands it exists as the pinned contract
/// the tests pin.
#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRingEvent {
    /// CLOCK_MONOTONIC microseconds (stamped by the tap).
    pub ts_us: u64,
    /// The evdev keycode (e.g. KEY_A = 30).
    pub keycode: u32,
    /// 1 = press, 0 = release (autorepeat is filtered in-kernel).
    pub value: u32,
    /// The keymap sound class.
    pub slot: u32,
}

/// Attach the tap: the privileged one-shot.
///
/// Returns a human-readable summary the CLI prints verbatim. Every
/// error names the missing piece and its fix — an attach that cannot
/// explain itself is a failed attach.
pub fn attach() -> Result<String, String> {
    // 1. The kernel gate (night-improve-23, the eBPF-only floor): one
    //    probe feeds both the doctor and this refusal, so the verdicts
    //    can never drift.
    let kernel = crate::session::kernel::probe();
    if !kernel.supported() {
        return Err(format!(
            "kernel eBPF verdict: {} ({} is below the 5.15 minimum) — zylaxion is \
             eBPF-only, upgrade the kernel",
            kernel.verdict(),
            if kernel.release.is_empty() {
                "unknown release".to_string()
            } else {
                kernel.release.clone()
            }
        ));
    }
    if kernel.lockdown_blocks_bpf {
        return Err(
            "kernel lockdown 'confidentiality' is active — the kernel withdraws BPF; \
             choose a lockdown mode that allows it (integrity or none)"
                .to_string(),
        );
    }
    if !kernel.bpffs {
        return Err(
            "bpffs is not mounted at /sys/fs/bpf — the pinned state store has nowhere \
             to live (fix: sudo mount -t bpf bpf /sys/fs/bpf, normally automatic)"
                .to_string(),
        );
    }

    // 2. Idempotence + self-heal: a complete set refuses politely; a
    //    partial set (crashed attach) is cleaned and replanted.
    if pin::all_pins_present() {
        return Err(
            "zylaxion tap is already attached (all pins present) — detach first or \
             check status"
                .to_string(),
        );
    }
    if pin::any_pin_present() {
        let removed = pin::clear_pins();
        log::info!("cleaned partial pin set before re-attach: {removed:?}");
    }

    // 3. The pin directory must exist before anything pins into it.
    std::fs::create_dir(pin::PIN_DIR)
        .map_err(|e| format!("failed to create {} (is bpffs mounted?): {e}", pin::PIN_DIR))?;

    // 4. The alignment preflight (the AlignedElf contract): structurally
    //    impossible now, kept so a regression diagnoses in one line.
    if let Some(violation) = embedded::alignment_violation(TAP_ELF, "zylaxion-tap") {
        return Err(violation);
    }

    // 5. Load the object.
    let mut bpf = Ebpf::load(TAP_ELF).map_err(|e| format!("failed to load the tap object: {e}"))?;

    // 6. Load and attach the program. The attach runs through the raw
    //    syscall lane (bpf_syscall): aya's typed TracePointLink wraps a
    //    pub(crate) inner, so its link cannot be pinned through the
    //    public API — and a pinned link is the whole point: it keeps
    //    the attachment alive after this process exits. The lane
    //    mirrors aya's own perf_attach shape (perf event handle +
    //    BPF_LINK_CREATE with BPF_PERF_EVENT) and pins the result.
    //    Map relocations need the map fds still registered in the
    //    object, so the program loads BEFORE the maps are taken out;
    //    the program's own pin lands in the same scope, and the borrow
    //    of the object ends before the maps are moved out below.
    {
        let prog: &mut TracePoint = bpf
            .program_mut("input_event")
            .ok_or("the tap object carries no 'input_event' program")?
            .try_into()
            .map_err(|e| format!("'input_event' is not a tracepoint program: {e}"))?;
        prog.load()
            .map_err(|e| format!("program load failed (verifier rejected the tap): {e}"))?;
        use std::os::fd::{AsFd as _, AsRawFd as _};
        let prog_fd = prog
            .fd()
            .map_err(|e| format!("failed to get the loaded program fd: {e}"))?
            .as_fd()
            .as_raw_fd();
        bpf_syscall::create_and_pin_tracepoint_link(prog_fd, "input", "input_event", pin::PIN_LINK)
            .map_err(|e| format!("tracepoint attach failed: {e}"))?;
        prog.pin(pin::PIN_PROG)
            .map_err(|e| format!("failed to pin the tap program: {e}"))?;
    }

    // 7. Initialize and pin the control plane. The control defaults put
    //    the tap in a defined idle posture: sound off, compiled default
    //    preset selected, volume 100 percent, no player registered.
    {
        let map = bpf
            .take_map("control")
            .ok_or("the tap object carries no 'control' map")?;
        let mut control_map: Array<_, u32> =
            Array::try_from(map).map_err(|e| format!("'control' is not an array map: {e}"))?;
        let default_preset_id = u32::try_from(
            crate::profiles::registry_all()
                .iter()
                .position(|e| e.name == crate::profiles::DEFAULT_PRESET)
                .unwrap_or(0),
        )
        .unwrap_or(0);
        let defaults = [
            (control::ENABLED, 0),
            (control::PRESET_ID, default_preset_id),
            (control::VOLUME_PCT, 100),
            (control::PLAYER_PID, 0),
            (control::STATE, 0),
        ];
        for (slot, value) in defaults {
            control_map
                .set(slot, value, 0)
                .map_err(|e| format!("control init failed at slot {slot}: {e}"))?;
        }
        control_map
            .pin(pin::PIN_MAP_CONTROL)
            .map_err(|e| format!("failed to pin the control map: {e}"))?;
    }

    // 8. The keymap: the sound intent. Keyboard keys 1..=255 map to the
    //    default sound class; everything else (mouse buttons, vendor
    //    keys) stays unmapped and dies in the kernel. The keymap is
    //    written ONCE here — preset switches ride the control map's
    //    preset slot, not this map.
    {
        let map = bpf
            .take_map("keymap")
            .ok_or("the tap object carries no 'keymap' map")?;
        let mut keymap: HashMap<_, u32, u32> =
            HashMap::try_from(map).map_err(|e| format!("'keymap' is not a hash map: {e}"))?;
        for code in 1..=255u32 {
            keymap
                .insert(code, 0, 0)
                .map_err(|e| format!("keymap init failed at keycode {code}: {e}"))?;
        }
        keymap
            .pin(pin::PIN_MAP_KEYMAP)
            .map_err(|e| format!("failed to pin the keymap: {e}"))?;
    }

    // 9. The event stream and the stats table pin as-is (fresh maps are
    //    zeroed by the kernel; no initialization needed).
    bpf.take_map("events")
        .ok_or("the tap object carries no 'events' map")?
        .pin(pin::PIN_MAP_EVENTS)
        .map_err(|e| format!("failed to pin the events ring buffer: {e}"))?;
    bpf.take_map("stats")
        .ok_or("the tap object carries no 'stats' map")?
        .pin(pin::PIN_MAP_STATS)
        .map_err(|e| format!("failed to pin the stats map: {e}"))?;

    // 10. Hand the pins to the invoking sudo user (if any) so the
    //     unprivileged player and CLI can open them afterwards.
    let ownership = hand_pins_to_sudo_user();

    Ok(format!(
        "tap attached: program {} ({} bytes), link pinned, control plane live \
         (preset: {}, volume: 100%, sound: off){ownership}",
        pin::PIN_PROG,
        TAP_ELF.len(),
        crate::profiles::DEFAULT_PRESET,
    ))
}

/// Detach the tap: unpin everything (idempotent, best-effort). Returns
/// a human-readable summary. After a successful detach NOTHING of
/// zylaxion exists in the kernel.
pub fn detach() -> Result<String, String> {
    if !pin::any_pin_present() {
        return Ok("tap is not attached — nothing to detach".to_string());
    }
    let removed = pin::clear_pins();
    let missing: Vec<_> = pin::ALL_PINS
        .iter()
        .filter(|p| Path::new(p).exists())
        .collect();
    if missing.is_empty() {
        Ok(format!(
            "tap detached: {} pin(s) removed — nothing of zylaxion remains in the kernel",
            removed.len()
        ))
    } else {
        Err(format!(
            "tap detach incomplete: {removed:?} removed, but {} could not be removed \
             (owned by another user? remove with sudo)",
            missing
                .iter()
                .map(|p| p.as_ref())
                .collect::<Vec<&str>>()
                .join(", ")
        ))
    }
}

/// Chown the pinned files to the invoking sudo user so the unprivileged
/// player and CLI can open them. Without sudo (a root shell), the pins
/// stay root-owned — honest about that in the summary.
fn hand_pins_to_sudo_user() -> String {
    let Ok(user) = std::env::var("SUDO_USER") else {
        return String::new();
    };
    if user.is_empty() || user == "root" {
        return String::new();
    }
    let Some(entry) = nix::unistd::User::from_name(&user).ok().flatten() else {
        return format!(" (sudo user '{user}' not resolvable — pins stay root-owned)");
    };
    let mut handed = 0usize;
    for path in pin::ALL_PINS {
        if Path::new(path).exists()
            && std::os::unix::fs::chown(
                path,
                Some(u32::from(entry.uid)),
                Some(u32::from(entry.gid)),
            )
            .is_ok()
        {
            handed += 1;
        }
    }
    if handed > 0 {
        format!(" (pins owned by '{user}' — unprivileged surfaces can open them)")
    } else {
        " (pin ownership unchanged — unprivileged surfaces may not open them)".to_string()
    }
}

#[cfg(test)]
#[path = "../../test/ebpf/loader_tests.rs"]
mod tests;
