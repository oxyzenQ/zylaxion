// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The attach orchestration: the one privileged one-shot that loads
//! the embedded tap, hands the work to the chain, and owns the
//! failure protocol (clean everything, plant the classified marker).

use aya::Ebpf;

use crate::ebpf::attach_error::AttachError;
use crate::ebpf::pin;

use super::chain::{hand_pins_to_desktop_user, plant};
use super::{TAP_ELF, last_attach_error};

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
    //    night-dinner-19: the early return still runs the hand-over
    //    (root only — a plain-user no-op has nothing to hand). The
    //    boot oneshot runs before anyone logs in, so the pins it
    //    plants are root-owned, and this lane is the one-command
    //    heal: `sudo zylaxion attach` after login hands them over
    //    without the detach cycle.
    if pin::all_pins_present() {
        let ownership = if crate::session::privilege::euid_is_root() {
            hand_pins_to_desktop_user()
        } else {
            String::new()
        };
        return Ok(format!(
            "tap is already attached — nothing to do (all pins present; `zylaxion detach` \
             removes it){ownership}"
        ));
    }
    // The pre-clean keeps the attach-failure marker planted (night-improve-26:
    // the evidence the doctor reads back is not swept with the rest).
    if pin::any_pin_present() {
        let (removed, _dir) = pin::clear_pins_keeping_marker();
        log::info!("cleaned partial pin set before re-attach: {removed:?}");
    }

    // The inherited evidence (night-improve-26): if a previous attempt
    // failed, its marker is still planted — name it in the log before
    // this attempt supersedes it. A successful plant re-pins a fresh
    // marker; a failure re-plants via the failure protocol. Then the
    // marker file alone goes: it lives in the pin directory, and a
    // non-empty directory is exactly what turned a stale state into
    // the EEXIST the live test surfaced on first-ever attach.
    if pin::error_marker_present() {
        match last_attach_error() {
            Some(prev) => {
                log::info!("inherited attach-failure marker from the previous attempt: {prev}");
            }
            None => {
                log::info!("a stale attach-failure marker is planted but unreadable");
            }
        }
        let _ = std::fs::remove_file(pin::PIN_MAP_ERROR);
    }

    // night-improve-26: a stale pin directory — a prior attempt mkdir'd
    // it and failed before planting anything — is removed here, the
    // same classified path detach uses. Attach is idempotent from any
    // prior state: fresh (absent), stale-empty (removed here),
    // half-attached (the pre-clean above), fully-attached (the early
    // return above). Foreign entries are NOT ours to remove — the
    // honest refusal names them instead of deleting unknown state.
    match pin::remove_pin_dir() {
        pin::PinDirRemoval::Removed => {
            log::info!("removed a stale pin directory left by a prior failed attach");
        }
        pin::PinDirRemoval::AlreadyAbsent => {}
        pin::PinDirRemoval::LeftForeignEntries => {
            return fail(AttachError::PinFailed {
                path: format!(
                    "{} holds entries zylaxion did not create — remove them by hand \
                     and re-run attach",
                    pin::PIN_DIR
                ),
            });
        }
        pin::PinDirRemoval::Unexpected(e) => {
            return fail(AttachError::PinFailed {
                path: format!(
                    "{} (a stale directory could not be removed): {e}",
                    pin::PIN_DIR
                ),
            });
        }
    }

    // 3. The pin directory must exist before anything pins into it.
    if let Err(e) = std::fs::create_dir(pin::PIN_DIR) {
        return fail(AttachError::PinFailed {
            path: format!("{} (is bpffs mounted?): {e}", pin::PIN_DIR),
        });
    }

    // 4. The alignment preflight (the AlignedElf contract): structurally
    //    impossible now, kept so a regression diagnoses in one line.
    if let Some(violation) = crate::ebpf::embedded::alignment_violation(TAP_ELF, "zylaxion-tap") {
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
            // 7. Hand the pins to the desktop user (the invoking sudo
            //    user, or — the boot lane — the first /run/user desktop
            //    session, night-dinner-19) so the unprivileged player
            //    and CLI can open them afterwards.
            let ownership = hand_pins_to_desktop_user();
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

/// The failure protocol: clean every pin this attempt planted (never
/// half-attached), plant ONLY the classified-error marker for the
/// doctor, and return the error. A marker that cannot be planted is
/// logged — the error it accompanies matters more.
fn fail(e: AttachError) -> Result<String, AttachError> {
    // The cleanup keeps the old marker planted (the evidence the doctor
    // reads back is not swept with the rest); the old marker file goes
    // right before the re-plant — a deliberate supersede, so the doctor
    // never reads a verdict older than this failure.
    let (removed, _) = pin::clear_pins_keeping_marker();
    let _ = std::fs::remove_file(pin::PIN_MAP_ERROR);
    log::info!(
        "attach failed ({e}); cleaned {} pin(s) — nothing of zylaxion remains half-attached",
        removed.len()
    );
    if let Err(marker_err) =
        crate::ebpf::bpf_syscall::plant_error_marker(e.code(), pin::PIN_MAP_ERROR)
    {
        log::warn!(
            "could not plant the attach-failure marker for the doctor ({}): {marker_err}",
            pin::PIN_MAP_ERROR
        );
    }
    Err(e)
}
