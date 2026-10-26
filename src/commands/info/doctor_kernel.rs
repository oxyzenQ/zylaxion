// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The doctor's kernel/eBPF station, extracted from the report
//! orchestrator (night-dragon-maintenance-4: the sibling LOC gate
//! prescribes the split - a pure move into modules, the gate green in
//! between). One probe feeds both this report and the attach path,
//! so the two surfaces can never drift; the station owns the whole
//! kernel story: the floor verdict, BTF, bpffs, the pin reach and
//! ownership walk, the tap-chain preflight, the last-attach marker,
//! lockdown, and the unprivileged-bpf posture. Pure move - the
//! printed bytes are unchanged; the station returns its verdict and
//! the orchestrator folds it into the report's exit verdict.

/// The identity the reach verdict describes (night-dinner-3): the
/// user who will RUN the verbs - the euid for a plain run, the sudo
/// user under sudo, and root for a plain root shell (the Root fact
/// row and the report-level Warnings block name that bias).
fn reach_identity() -> (u32, u32) {
    if !crate::session::privilege::euid_is_root() {
        return (
            nix::unistd::geteuid().as_raw(),
            nix::unistd::getegid().as_raw(),
        );
    }
    match (
        std::env::var("SUDO_UID")
            .ok()
            .and_then(|v| v.parse::<u32>().ok()),
        std::env::var("SUDO_GID")
            .ok()
            .and_then(|v| v.parse::<u32>().ok()),
    ) {
        (Some(uid), Some(gid)) => (uid, gid),
        _ => (0, 0),
    }
}

/// Render the kernel/eBPF station and return its verdict: false when
/// any row of the station failed (the floor, the mount, the reach,
/// the ownership, lockdown, or the unprivileged-bpf posture).
pub(super) fn kernel_station(root: bool) -> bool {
    let mut ok = true;
    // 1. Kernel / eBPF capability (night-improve-23, the eBPF-only
    //    floor): the input tap is a passive kernel-side probe on the
    //    exported input_event() function and the state store is pinned
    //    maps on bpffs — the kernel IS the platform now. The owner's
    //    minimum is 5.15; below it the verdict is "verified
    //    unsupported" and nothing else in this report can save the
    //    run. One probe feeds both this report and the attach path,
    //    so the two surfaces can never drift.
    let kernel = crate::session::kernel::probe();
    println!();
    if kernel.release.is_empty() {
        println!(
            "{}",
            crate::output::kv(
                "Kernel:",
                &crate::output::fail_line("unreadable (/proc/sys/kernel/osrelease)")
            )
        );
        ok = false;
    } else {
        println!("{}", crate::output::kv("Kernel:", &kernel.release));
        println!(
            "{}",
            crate::output::kv(
                "Root:",
                &if root {
                    crate::output::warn_line("YES — the session the verdicts describe")
                } else {
                    "NO".to_string()
                }
            )
        );
        if kernel.supported() {
            println!(
                "{}",
                crate::output::kv(
                    "eBPF:",
                    &crate::output::ok_line(&format!(
                        "{} ({} meets the 5.15 minimum)",
                        kernel.verdict(),
                        kernel.release
                    ))
                )
            );
        } else {
            println!(
                "{}",
                crate::output::kv(
                    "eBPF:",
                    &crate::output::fail_line(&format!(
                        "{} ({} is below the 5.15 minimum — zylaxion is eBPF-only, upgrade the kernel)",
                        kernel.verdict(),
                        kernel.release
                    ))
                )
            );
            ok = false;
        }
        // BTF: the fentry lane's enabler (night-improve-25). The chain
        // does not NEED it — the kprobe steps run without BTF — so it
        // is reported for completeness, never fatal.
        println!(
            "{}",
            crate::output::kv(
                "BTF:",
                &format!(
                    "{} ({})",
                    if kernel.btf {
                        crate::output::ok_line("present")
                    } else {
                        crate::output::tip_line("absent")
                    },
                    if kernel.btf {
                        "enables the fentry lane"
                    } else {
                        "the chain falls back to the kprobe lanes"
                    }
                )
            )
        );
        if kernel.bpffs {
            println!(
                "{}",
                crate::output::kv("bpffs:", &crate::output::ok_line("mounted at /sys/fs/bpf"))
            );
            // The reach verdict (night-dinner-3): owning the pins is
            // not REACHING the pins — a bpffs mount in mode 0700 (the
            // Debian 13 default) lets attach plant user-owned pins
            // and still blocks every walk to them, so "All checks
            // passed" used to be a lie the first `zylaxion on`
            // refuted. Doctor stats the walk AS the identity that
            // runs the verbs.
            let (uid, gid) = reach_identity();
            let reach = crate::session::kernel::bpffs_reach(uid, gid);
            if !reach.root_traversable {
                println!(
                    "{}",
                    crate::output::kv(
                        "pin reach:",
                        &crate::output::fail_line(&format!(
                            "uid {uid} cannot walk to the pins (mount mode {:o}) — a \
                             mode-0700 bpffs mount, the Debian 13 default",
                            reach.root_mode
                        ))
                    )
                );
                println!(
                    "{}",
                    crate::output::kv(
                        "fix:",
                        &crate::output::tip_line(
                            "sudo chmod o+x /sys/fs/bpf (one command; attach does this \
                             automatically whenever it has a desktop user to hand the pins to)"
                        )
                    )
                );
                ok = false;
            } else if reach.pin_dir_present && !reach.pin_dir_traversable {
                println!(
                    "{}",
                    crate::output::kv(
                        "pin reach:",
                        &crate::output::fail_line(&format!(
                            "uid {uid} walks the mount but /sys/fs/bpf/zylaxion denies the \
                             walk (mode {:o})",
                            reach.root_mode
                        ))
                    )
                );
                println!(
                    "{}",
                    crate::output::kv(
                        "fix:",
                        &crate::output::tip_line(
                            "sudo chmod o+x /sys/fs/bpf/zylaxion (attach plants it 0755; \
                             this one was tightened by hand)"
                        )
                    )
                );
                ok = false;
            } else {
                println!(
                    "{}",
                    crate::output::kv(
                        "pin reach:",
                        &crate::output::ok_line(&format!(
                            "uid {uid} walks to the pins (mount mode {:o})",
                            reach.root_mode
                        ))
                    )
                );
                // The ownership verdict (night-dinner-19): reaching is
                // not OPENING — the boot oneshot ran before the first
                // login, so a walkable pin set can still be root-owned
                // and the first `zylaxion on` refuses at BPF_OBJ_GET
                // while every reach row reads green. Root is skipped:
                // the capability law opens everything for root (the
                // Root row already names that posture).
                if !root
                    && let Some(owner) = std::fs::metadata(crate::ebpf::pin::PIN_MAP_CONTROL)
                        .ok()
                        .map(|m| std::os::unix::fs::MetadataExt::uid(&m))
                {
                    if owner == uid {
                        println!(
                            "{}",
                            crate::output::kv(
                                "ownership:",
                                &crate::output::ok_line(&format!(
                                    "the control map is yours (uid {uid})"
                                ))
                            )
                        );
                    } else {
                        println!(
                            "{}",
                            crate::output::kv(
                                "ownership:",
                                &crate::output::fail_line(&format!(
                                    "the control map is owned by uid {owner}, not you \
                                     (uid {uid}) — the boot attach ran before your login"
                                ))
                            )
                        );
                        println!(
                            "{}",
                            crate::output::kv(
                                "fix:",
                                &crate::output::tip_line(
                                    "sudo zylaxion attach (heals in place — it hands the \
                                     pins over without a detach cycle)"
                                )
                            )
                        );
                        ok = false;
                    }
                }
            }
        } else {
            println!(
                "{}",
                crate::output::kv(
                    "bpffs:",
                    &crate::output::fail_line(
                        "not mounted at /sys/fs/bpf — the pinned state store has nowhere to live"
                    )
                )
            );
            println!(
                "{}",
                crate::output::kv(
                    "fix:",
                    &crate::output::tip_line(
                        "sudo mount -t bpf bpf /sys/fs/bpf (normally automatic)"
                    )
                )
            );
            ok = false;
        }
        // The input-tap verdict (night-improve-25): the probe's
        // preflight of the fentry → kprobe chain — one of the four
        // canonical strings, self-explaining, and the doctor adds
        // nothing to the substance (the attach attempt stays the
        // authoritative verdict). The canonical string's own
        // "input tap: " lead yields to the row key, the same law the
        // unprivileged-bpf row applies to its verdict string.
        let raw_tap: &str = &kernel.input_tap;
        let tap_value = raw_tap.strip_prefix("input tap: ").unwrap_or(raw_tap);
        // night-dragon-maintenance-4: the chain preflight is a verdict,
        // not a fact row — a verified step (fentry or either kprobe
        // lane) rides the status-green tier, an exhausted chain
        // (UNAVAILABLE) rides the error-red tier. Plain bytes on a
        // non-terminal stdout, the same gate every other tier rides.
        let tap_colored = if tap_value.starts_with("UNAVAILABLE") {
            crate::output::fail_line(tap_value)
        } else {
            crate::output::ok_line(tap_value)
        };
        println!("{}", crate::output::kv("tap chain:", &tap_colored));
        // The last attach failure, if one left its marker
        // (night-improve-25): a failed attach plants a small pinned map
        // carrying the classified error code; naming it here is the
        // doctor's honesty duty — the tap is not attached, and this
        // line says exactly which step of the chain failed and how it
        // was classified.
        if let Some(err) = crate::ebpf::loader::last_attach_error() {
            println!(
                "{}",
                crate::output::kv(
                    "last attach:",
                    &crate::output::tip_line(&format!(
                        "failed: {err} — `sudo zylaxion attach` prints the per-step log"
                    ))
                )
            );
        }
        if kernel.lockdown_readable && kernel.lockdown_blocks_bpf {
            println!(
                "{}",
                crate::output::kv(
                    "lockdown:",
                    &crate::output::fail_line(
                        "'confidentiality' active — the kernel withdraws BPF"
                    )
                )
            );
            println!(
                "{}",
                crate::output::kv(
                    "fix:",
                    &crate::output::tip_line(
                        "choose a kernel lockdown mode that allows BPF (integrity or none)"
                    )
                )
            );
            ok = false;
        }
        // The unprivileged-bpf posture (night-badas-4 round 5): the
        // control-plane verbs run as a plain user only when the kernel
        // allows unprivileged bpf(). A distro that builds the gate in
        // (Ubuntu does, CONFIG_BPF_UNPRIV_DEFAULT_OFF=y) locks the
        // verbs out for the whole boot — the doctor's duty is to name
        // it, the same way lockdown is named. The canonical verdict
        // string carries its own "unprivileged bpf:" lead; the row key
        // replaces it (the strip keeps the row geometry, the fallback
        // keeps the truth if the canonical string ever changes shape).
        let unpriv_verdict = kernel.unpriv_bpf_verdict();
        let unpriv_value = unpriv_verdict
            .strip_prefix("unprivileged bpf: ")
            .unwrap_or(&unpriv_verdict)
            .to_string();
        if kernel.unpriv_bpf_allowed() {
            println!(
                "{}",
                crate::output::kv("unpriv bpf:", &crate::output::ok_line(&unpriv_value))
            );
        } else {
            println!(
                "{}",
                crate::output::kv("unpriv bpf:", &crate::output::fail_line(&unpriv_value))
            );
            ok = false;
        }
    }
    ok
}
