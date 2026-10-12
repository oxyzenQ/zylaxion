// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The tap loader — attach, init, pin; and detach (night-improve-23;
//! night-improve-25 restores the approved attach chain).
//!
//! This is the privileged one-shot that replaces daemon startup. It
//! loads the embedded tap object, walks the attach chain, initializes
//! the control plane, and pins EVERYTHING to bpffs — then the process
//! may exit, because the pinned link keeps the tap alive and the
//! pinned maps keep the state alive. After `attach` returns, nothing
//! of zylaxion is resident in userspace.
//!
//! ## The attach chain (locked, night-improve-25)
//!
//! Tried in order, the loader picks, no user-facing mode:
//!
//! 1. `fentry:input_event` — BPF_PROG_TYPE_TRACING on the exported
//!    `input_event()` function via the BTF trampoline. Zero overhead.
//!    Requires vmlinux BTF and the symbol in it (aya resolves the
//!    attach_btf_id at load time).
//! 2. `kprobe:input_event` — BPF_PROG_TYPE_KPROBE, pt_regs, no BTF
//!    needed. Requires the symbol off the kprobe blacklist (it is:
//!    plain-text EXPORT_SYMBOL on mainline).
//! 3. `kprobe:input_handle_event` — the alternate symbol, one step
//!    later in the input core (after the is_event_supported check);
//!    fewer events, still passive.
//! 4. Hard refusal — a classified [`AttachError`]; nothing
//!    half-attached, no silent fallback.
//!
//! The chain step that won is recorded in the control map's TAP_KIND
//! slot (0/1/2) so `status` and the doctor can report it, and the
//! chosen step's label leads the attach summary.
//!
//! Ownership model (the boring privilege story): attach runs as root
//! (`sudo zylaxion attach`), and when invoked through sudo the pins are
//! chowned to the invoking user — so the unprivileged player and CLI can
//! open the pinned maps afterwards (bpffs file permissions govern pinned
//! map access, the same posture systemd-tmpfiles-style boot units use).
//!
//! Idempotence and self-heal: a complete pin set is already-attached
//! (a success shape — running attach twice is fine); a partial pin set
//! (a crashed attach, a manual `rm`) is cleaned and replanted — and a
//! failed attach plants ONLY the failure marker (a small pinned map
//! carrying the classified error code) so the doctor can name the
//! failure afterwards. The stale-state forensics the old daemon needed
//! (PID files, /proc/exe identity checks) reduce to "pins exist and
//! are complete", because the kernel owns the state now.

use std::path::Path;

use aya::maps::{Array, HashMap};
use aya::programs::{FEntry, KProbe};
use aya::{Btf, Ebpf};

use super::attach_error::AttachError;
use super::bpf_syscall;
use super::embedded;
use super::pin;

/// The embedded pure-Rust tap object (night-improve-23): the aya-ebpf
/// ELF staged into OUT_DIR by build.rs — source-built on a git checkout,
/// registry-prebuilt on a crates.io extract; either way the same name,
/// either way it rides inside the binary.
static TAP_ELF: &[u8] =
    &embedded::AlignedElf::new(*include_bytes!(concat!(env!("OUT_DIR"), "/zylaxion-tap"))).bytes;

/// The program names in the tap object (the ELF symbol names — see
/// ebpf/src/main.rs: two attach programs over one body).
const FENTRY_PROGRAM: &str = "fentry_input_event";
const KPROBE_PROGRAM: &str = "kprobe_input_event";

/// The chain step that owns the live tap — the vocabulary of the
/// control map's TAP_KIND slot and of every surface that reports
/// which lane is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapKind {
    /// Step 1: `fentry:input_event` (BPF_PROG_TYPE_TRACING, BTF).
    Fentry,
    /// Step 2: `kprobe:input_event` (BPF_PROG_TYPE_KPROBE).
    Kprobe,
    /// Step 3: `kprobe:input_handle_event` (the alternate symbol).
    KprobeAlt,
}

impl TapKind {
    /// The chain label every surface prints — the attach summary, the
    /// status tap line, the doctor.
    pub fn label(self) -> &'static str {
        match self {
            TapKind::Fentry => "fentry:input_event",
            TapKind::Kprobe => "kprobe:input_event",
            TapKind::KprobeAlt => "kprobe:input_handle_event",
        }
    }

    /// The control-map slot value (the wire format, pinned by tests).
    pub fn slot(self) -> u32 {
        match self {
            TapKind::Fentry => 0,
            TapKind::Kprobe => 1,
            TapKind::KprobeAlt => 2,
        }
    }

    /// The inverse of [`Self::slot`] — the read side `status` uses.
    pub fn from_slot(slot: u32) -> Option<Self> {
        Some(match slot {
            0 => TapKind::Fentry,
            1 => TapKind::Kprobe,
            2 => TapKind::KprobeAlt,
            _ => return None,
        })
    }
}

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
    /// Which attach-chain step owns the live tap (night-improve-25):
    /// the [`super::TapKind`] slot value (0/1/2).
    pub const TAP_KIND: u32 = 5;
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
/// Returns a human-readable summary the CLI prints verbatim; every
/// failure is a classified [`AttachError`] and never leaves a partial
/// pin set (only the failure marker, for the doctor).
pub fn attach() -> Result<String, AttachError> {
    // 1. The kernel gate (night-improve-23, the eBPF-only floor): one
    //    probe feeds both the doctor and this refusal, so the verdicts
    //    can never drift.
    let kernel = crate::session::kernel::probe();
    if !kernel.supported() {
        return fail(AttachError::Unknown {
            detail: format!(
                "kernel {} is below the 5.15 minimum — zylaxion is eBPF-only, upgrade the \
                 kernel (the doctor's verdict: {})",
                if kernel.release.is_empty() {
                    "unknown release"
                } else {
                    &kernel.release
                },
                kernel.verdict()
            ),
        });
    }
    if kernel.lockdown_blocks_bpf {
        return fail(AttachError::LockdownActive);
    }
    if !kernel.bpffs {
        return fail(AttachError::BpffsUnavailable);
    }

    // 2. Idempotence + self-heal: a complete set IS attached — the
    //    second attach is a success shape, nothing to do. A partial
    //    set (crashed attach) is cleaned and replanted.
    if pin::all_pins_present() {
        return Ok(
            "tap is already attached — nothing to do (all pins present; `zylaxion detach` \
             removes it)"
                .to_string(),
        );
    }
    if pin::any_pin_present() {
        let removed = pin::clear_pins();
        log::info!("cleaned partial pin set before re-attach: {removed:?}");
    }

    // 3. The pin directory must exist before anything pins into it.
    if let Err(e) = std::fs::create_dir(pin::PIN_DIR) {
        return fail(AttachError::PinFailed {
            path: format!("{} (is bpffs mounted?): {e}", pin::PIN_DIR),
        });
    }

    // 4. The alignment preflight (the AlignedElf contract): structurally
    //    impossible now, kept so a regression diagnoses in one line.
    if let Some(violation) = embedded::alignment_violation(TAP_ELF, "zylaxion-tap") {
        return fail(AttachError::Unknown { detail: violation });
    }

    // 5. Load the object (map relocations happen here; programs load
    //    per-chain-step below).
    let mut bpf = match Ebpf::load(TAP_ELF) {
        Ok(b) => b,
        Err(e) => {
            return fail(AttachError::Unknown {
                detail: format!("failed to load the tap object: {e}"),
            });
        }
    };

    // 6. THE CHAIN (fentry -> kprobe -> kprobe-alt), then the control
    //    plane init and the map pins — all inside plant(), so any
    //    failure cleans every pin before returning.
    let planted = plant(&mut bpf);
    match planted {
        Ok(kind) => {
            // 7. Hand the pins to the invoking sudo user (if any) so the
            //    unprivileged player and CLI can open them afterwards.
            let ownership = hand_pins_to_sudo_user();
            Ok(format!(
                "attach: {}\ntap attached: program {} ({} bytes), link pinned, control plane \
                 live (preset: {}, volume: 100%, sound: off){ownership}",
                kind.label(),
                pin::PIN_PROG,
                TAP_ELF.len(),
                crate::profiles::DEFAULT_PRESET,
            ))
        }
        Err(e) => fail(e),
    }
}

/// Plant everything: walk the chain, then initialize and pin the
/// control plane. Any failure returns a classified [`AttachError`] —
/// the caller's [`fail`] protocol then cleans every pin and plants
/// the marker (this function pins, the caller decides what survives).
fn plant(bpf: &mut Ebpf) -> Result<TapKind, AttachError> {
    // ── The chain ────────────────────────────────────────────────────
    let mut chosen: Option<TapKind> = None;
    // The last step's failure — the verdict the operator reads when
    // the chain is exhausted (the per-step INFO log carries every
    // reason along the way).
    let mut last_step_error: Option<AttachError> = None;

    // Step 1: fentry:input_event (BTF trampoline, zero overhead).
    match try_fentry(bpf) {
        Ok(()) => chosen = Some(TapKind::Fentry),
        Err(e) => {
            log::info!("attach chain: fentry:input_event unavailable: {e}");
            last_step_error = Some(e);
        }
    }

    // Steps 2+3: kprobe on input_event, then the alternate symbol.
    // The program loads once; the symbol is an attach-time parameter.
    if chosen.is_none() {
        match load_kprobe_program(bpf) {
            Ok(prog_fd) => {
                for (kind, symbol) in [
                    (TapKind::Kprobe, "input_event"),
                    (TapKind::KprobeAlt, "input_handle_event"),
                ] {
                    match bpf_syscall::create_and_pin_kprobe_link(prog_fd, symbol, pin::PIN_LINK) {
                        Ok(()) => {
                            pin_kprobe_program(bpf)?;
                            chosen = Some(kind);
                            break;
                        }
                        Err(e) => {
                            log::info!("attach chain: kprobe:{symbol} unavailable: {e}");
                            last_step_error = Some(e);
                        }
                    }
                }
            }
            Err(e) => {
                log::info!("attach chain: the kprobe program did not load: {e}");
                last_step_error = Some(e);
            }
        }
    }

    let kind = chosen.ok_or_else(|| last_step_error.unwrap_or(AttachError::SymbolMissing))?;

    // ── The control plane (unchanged discipline, one new slot) ─────
    //
    // Initialize and pin. The control defaults put the tap in a
    // defined idle posture: sound off, compiled default preset
    // selected, volume 100 percent, no player registered, and the
    // TAP_KIND slot naming the chain step that owns this attachment.
    {
        let map = bpf
            .take_map("control")
            .ok_or_else(|| AttachError::Unknown {
                detail: "the tap object carries no 'control' map".to_string(),
            })?;
        let mut control_map: Array<_, u32> =
            Array::try_from(map).map_err(|e| AttachError::Unknown {
                detail: format!("'control' is not an array map: {e}"),
            })?;
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
            (control::TAP_KIND, kind.slot()),
        ];
        for (slot, value) in defaults {
            control_map
                .set(slot, value, 0)
                .map_err(|e| AttachError::Unknown {
                    detail: format!("control init failed at slot {slot}: {e}"),
                })?;
        }
        control_map
            .pin(pin::PIN_MAP_CONTROL)
            .map_err(|e| AttachError::PinFailed {
                path: format!("{} ({e})", pin::PIN_MAP_CONTROL),
            })?;
    }

    // The keymap: the sound intent. Keyboard keys 1..=255 map to the
    // default sound class; everything else (mouse buttons, vendor
    // keys) stays unmapped and dies in the kernel. The keymap is
    // written ONCE here — preset switches ride the control map's
    // preset slot, not this map.
    {
        let map = bpf.take_map("keymap").ok_or_else(|| AttachError::Unknown {
            detail: "the tap object carries no 'keymap' map".to_string(),
        })?;
        let mut keymap: HashMap<_, u32, u32> =
            HashMap::try_from(map).map_err(|e| AttachError::Unknown {
                detail: format!("'keymap' is not a hash map: {e}"),
            })?;
        for code in 1..=255u32 {
            keymap
                .insert(code, 0, 0)
                .map_err(|e| AttachError::Unknown {
                    detail: format!("keymap init failed at keycode {code}: {e}"),
                })?;
        }
        keymap
            .pin(pin::PIN_MAP_KEYMAP)
            .map_err(|e| AttachError::PinFailed {
                path: format!("{} ({e})", pin::PIN_MAP_KEYMAP),
            })?;
    }

    // The event stream and the stats table pin as-is (fresh maps are
    // zeroed by the kernel; no initialization needed).
    bpf.take_map("events")
        .ok_or_else(|| AttachError::Unknown {
            detail: "the tap object carries no 'events' map".to_string(),
        })?
        .pin(pin::PIN_MAP_EVENTS)
        .map_err(|e| AttachError::PinFailed {
            path: format!("{} ({e})", pin::PIN_MAP_EVENTS),
        })?;
    bpf.take_map("stats")
        .ok_or_else(|| AttachError::Unknown {
            detail: "the tap object carries no 'stats' map".to_string(),
        })?
        .pin(pin::PIN_MAP_STATS)
        .map_err(|e| AttachError::PinFailed {
            path: format!("{} ({e})", pin::PIN_MAP_STATS),
        })?;

    Ok(kind)
}

/// Chain step 1: fentry on the exported input_event() function.
///
/// Aya resolves the target's BTF id at LOAD time (Btf::from_sys_fs +
/// the section name), so every BTF fact — no BTF, unreadable BTF, or
/// the symbol absent from it — classifies as
/// [`AttachError::FentryUnavailable`] before the kernel ever sees the
/// program. Map relocations need the map fds still registered in the
/// object, so the program loads BEFORE the maps are taken out.
fn try_fentry(bpf: &mut Ebpf) -> Result<(), AttachError> {
    let btf = Btf::from_sys_fs().map_err(|e| AttachError::FentryUnavailable {
        reason: format!("no usable vmlinux BTF: {e}"),
    })?;
    let prog: &mut FEntry = bpf
        .program_mut(FENTRY_PROGRAM)
        .ok_or_else(|| AttachError::Unknown {
            detail: format!("the tap object carries no '{FENTRY_PROGRAM}' program"),
        })?
        .try_into()
        .map_err(|e| AttachError::Unknown {
            detail: format!("'{FENTRY_PROGRAM}' is not an fentry program: {e}"),
        })?;
    prog.load("input_event", &btf)
        .map_err(AttachError::from_program_load)?;
    use std::os::fd::{AsFd as _, AsRawFd as _};
    let prog_fd = prog
        .fd()
        .map_err(|e| AttachError::Unknown {
            detail: format!("failed to get the loaded program fd: {e}"),
        })?
        .as_fd()
        .as_raw_fd();
    bpf_syscall::create_and_pin_fentry_link(prog_fd, pin::PIN_LINK)?;
    prog.pin(pin::PIN_PROG)
        .map_err(|e| AttachError::PinFailed {
            path: format!("{} ({e})", pin::PIN_PROG),
        })?;
    Ok(())
}

/// Chain steps 2+3 (load half): load the kprobe program once and hand
/// back its raw fd. The symbol is a link-time parameter, not baked
/// into the program — the same binary probes `input_event` and
/// `input_handle_event`.
fn load_kprobe_program(bpf: &mut Ebpf) -> Result<i32, AttachError> {
    let prog: &mut KProbe = bpf
        .program_mut(KPROBE_PROGRAM)
        .ok_or_else(|| AttachError::Unknown {
            detail: format!("the tap object carries no '{KPROBE_PROGRAM}' program"),
        })?
        .try_into()
        .map_err(|e| AttachError::Unknown {
            detail: format!("'{KPROBE_PROGRAM}' is not a kprobe program: {e}"),
        })?;
    prog.load().map_err(AttachError::from_program_load)?;
    use std::os::fd::{AsFd as _, AsRawFd as _};
    let prog_fd = prog
        .fd()
        .map_err(|e| AttachError::Unknown {
            detail: format!("failed to get the loaded program fd: {e}"),
        })?
        .as_fd()
        .as_raw_fd();
    Ok(prog_fd)
}

/// Chain steps 2+3 (pin half): the winning kprobe link is pinned, so
/// the program itself follows it to bpffs.
fn pin_kprobe_program(bpf: &mut Ebpf) -> Result<(), AttachError> {
    let prog: &mut KProbe = bpf
        .program_mut(KPROBE_PROGRAM)
        .ok_or_else(|| AttachError::Unknown {
            detail: format!("the tap object carries no '{KPROBE_PROGRAM}' program"),
        })?
        .try_into()
        .map_err(|e| AttachError::Unknown {
            detail: format!("'{KPROBE_PROGRAM}' is not a kprobe program: {e}"),
        })?;
    prog.pin(pin::PIN_PROG)
        .map_err(|e| AttachError::PinFailed {
            path: format!("{} ({e})", pin::PIN_PROG),
        })?;
    Ok(())
}

/// The failure protocol: clean every pin this attempt planted (never
/// half-attached), plant ONLY the classified-error marker for the
/// doctor, and return the error. A marker that cannot be planted is
/// logged — the error it accompanies matters more.
fn fail(e: AttachError) -> Result<String, AttachError> {
    let removed = pin::clear_pins();
    log::info!(
        "attach failed ({e}); cleaned {} pin(s) — nothing of zylaxion remains half-attached",
        removed.len()
    );
    if let Err(marker_err) = bpf_syscall::plant_error_marker(e.code(), pin::PIN_MAP_ERROR) {
        log::warn!(
            "could not plant the attach-failure marker for the doctor ({}): {marker_err}",
            pin::PIN_MAP_ERROR
        );
    }
    Err(e)
}

/// Detach the tap: unpin everything (idempotent, best-effort). Returns
/// a human-readable summary. After a successful detach NOTHING of
/// zylaxion exists in the kernel.
pub fn detach() -> Result<String, AttachError> {
    if !pin::any_pin_present() {
        return Ok("tap is not attached — nothing to detach".to_string());
    }
    let removed = pin::clear_pins();
    let missing: Vec<_> = pin::ALL_PINS
        .iter()
        .chain(std::iter::once(&pin::PIN_MAP_ERROR))
        .filter(|p| Path::new(p).exists())
        .collect();
    if missing.is_empty() {
        Ok(format!(
            "tap detached: {} pin(s) removed — nothing of zylaxion remains in the kernel",
            removed.len()
        ))
    } else {
        Err(AttachError::Unknown {
            detail: format!(
                "tap detach incomplete: {removed:?} removed, but {} could not be removed \
                 (owned by another user? remove with sudo)",
                missing
                    .iter()
                    .map(|p| p.as_ref())
                    .collect::<Vec<&str>>()
                    .join(", ")
            ),
        })
    }
}

/// The last attach failure recorded in the marker map, if one is
/// planted and readable — the doctor's read side of the marker lane.
pub fn last_attach_error() -> Option<AttachError> {
    use aya::maps::MapData;
    let data = MapData::from_pin(pin::PIN_MAP_ERROR).ok()?;
    let owned = aya::maps::Map::Array(data);
    let marker: Array<MapData, u32> = Array::try_from(owned).ok()?;
    let code = marker.get(&0u32, 0).ok()?;
    AttachError::from_code(code)
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
