// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The attach chain and the map planting: walk fentry → kprobe →
//! kprobe-alt, then initialize and pin the control plane, the keymap,
//! and the streaming maps.

use aya::maps::{Array, HashMap};
use aya::programs::{FEntry, KProbe};
use aya::{Btf, Ebpf};

use crate::ebpf::attach_error::AttachError;
use crate::ebpf::bpf_syscall;
use crate::ebpf::pin;

use super::{FENTRY_PROGRAM, KEYMAP_KEYCODE_DOMAIN, KPROBE_PROGRAM, TapKind, control};

/// Plant everything: walk the chain, then initialize and pin the
/// control plane. Any failure returns a classified [`AttachError`] —
/// the caller's [`super::attach`]'s fail protocol then cleans every
/// pin and plants the marker (this function pins, the caller decides
/// what survives).
pub(super) fn plant(bpf: &mut Ebpf) -> Result<TapKind, AttachError> {
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
        for code in KEYMAP_KEYCODE_DOMAIN {
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

/// Chown the pinned files to the invoking sudo user so the unprivileged
/// player and CLI can open them. Without sudo (a root shell), the pins
/// stay root-owned — honest about that in the summary.
pub(super) fn hand_pins_to_sudo_user() -> String {
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
        if std::path::Path::new(path).exists()
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
