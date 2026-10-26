// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The `doctor` subcommand: the four-station system-health report.

use std::process;

use cpal::traits::{DeviceTrait, HostTrait};

use super::alsa_guard::QuietAlsa;

/// The identity the reach verdict describes (night-dinner-3): the
/// user who will RUN the verbs — the euid for a plain run, the sudo
/// user under sudo, and root for a plain root shell (who walks
/// everywhere; the Warnings block already names that bias).
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

/// Print a system-health diagnostic report (build facts, kernel/eBPF
/// verdict, audio).
///
/// The report leads with the BUILD facts block (night-improve-13, the
/// zelynic night-improve-66 doctor completeness) — the binary answers
/// "which exact artifact am I?" before anything about the host,
/// because on a foreign build every host verdict below reads
/// differently. Every fact is compile-time truth assembled by
/// [`crate::info::build`] (the Hinnant clock stamps the build time,
/// no time crate). The output geometry (banner, key/value rows,
/// Warnings block) is the family law [`crate::output`] documents.
pub fn cmd_doctor() {
    println!("{}\n", crate::output::section("zylaxion doctor"));

    let mut ok = true;
    // Privilege awareness (night-improve-16, the audit's warn lane):
    // doctor stays reachable as root (a diagnostic must diagnose), but
    // every verdict below would describe ROOT's session — root is
    // always "in" the input group, root's audio server is a different
    // instance, root's XDG_RUNTIME_DIR may not even be the user's.
    // The distortion is named so the report cannot mislead: the Root
    // fact row in the session group states the posture where the
    // zelynic reference puts it, and the report-level Warnings block
    // carries the consequence.
    let mut warnings: Vec<&'static str> = Vec::new();
    let root = crate::session::privilege::euid_is_root();
    if root {
        warnings.push(
            "running as root — the verdicts above describe root's session, not yours; re-run without sudo",
        );
    }

    // 0. Build facts (compile-time truth — cannot fail, never counts
    //    against the ok verdict). The zelynic doctor reference order:
    //    Build, Version, Commit, Variant, Built, Rustc, Profile.
    let build = crate::info::build::BuildInfo::collect();
    // night-hunter-7: the binary's own build-flavor verdict, the
    // zelynic doctor lineage. Since night-dragon-maintenance-0 (the
    // unkillable-ebpf law) every build is FULL-LIFE: there is no
    // feature gate left to hide the tap behind, and the lane detail
    // is the ZYLAXION_EBPF_LANE stamp build.rs emits (source-built on
    // a git checkout, registry-prebuilt on a registry tarball). The
    // verdict stays an honest statement of that fact, not a branch.
    let lane = option_env!("ZYLAXION_EBPF_LANE").unwrap_or("unknown");
    println!(
        "{}",
        crate::output::kv(
            "Build:",
            &crate::output::ok_line(&format!("FULL-LIFE (eBPF objects: {lane})"))
        )
    );
    println!(
        "{}",
        crate::output::kv("Version:", &format!("v{}", build.version))
    );
    println!("{}", crate::output::kv("Commit:", &build.git_sha));
    println!("{}", crate::output::kv("Variant:", &build.variant));
    println!("{}", crate::output::kv("Built:", &build.build_time));
    println!("{}", crate::output::kv("Rustc:", &build.rustc_version));
    println!("{}", crate::output::kv("Profile:", &build.profile));

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
        println!("{}", crate::output::kv("tap chain:", tap_value));
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

    // 2. XDG_RUNTIME_DIR check.
    // The audio-session posture (night-total-lts-2): the user's audio
    // server exposes its sockets under $XDG_RUNTIME_DIR, so a missing
    // or bogus value breaks the player's audio-server connection even
    // though zylaxion itself writes no files there (the kernel control
    // map is the pidfile since night-improve-23 — RULES.md's file
    // inventory names it).
    println!();
    if root {
        // The root-session note (night-dinner-3): a root doctor used
        // to FAIL this station on root's own missing XDG_RUNTIME_DIR —
        // a fact about root's shell that describes nobody's desktop
        // and pushed the report to a false "Some checks failed".
        // Root never plays (the session refusal owns that verdict),
        // so the station downgrades to what it can honestly say:
        // when sudo handed us the desktop user's uid, THEIR runtime
        // dir is the one the player would open — a note, never a
        // verdict counted into the summary.
        match (
            std::env::var("SUDO_UID")
                .ok()
                .and_then(|v| v.parse::<u32>().ok()),
            std::env::var("SUDO_USER").ok(),
        ) {
            (Some(uid), Some(user)) if !user.is_empty() => {
                let dir = format!("/run/user/{uid}");
                if std::path::Path::new(&dir).is_dir() {
                    println!(
                        "{}",
                        crate::output::kv(
                            "XDG dir:",
                            &crate::output::ok_line(&format!(
                                "sudo user '{user}' has {dir} — the desktop session \
                                 checks out (root's shell has none of its own)"
                            ))
                        )
                    );
                } else {
                    println!(
                        "{}",
                        crate::output::kv(
                            "XDG dir:",
                            &crate::output::warn_line(&format!(
                                "sudo user '{user}' has no {dir} — their desktop session \
                                 is missing the runtime dir the player opens"
                            ))
                        )
                    );
                }
            }
            _ => {
                println!(
                    "{}",
                    crate::output::kv(
                        "XDG dir:",
                        &crate::output::warn_line(
                            "a plain root shell — the audio-session facts belong to a \
                             desktop user; re-run without sudo"
                        )
                    )
                );
            }
        }
    } else {
        match std::env::var("XDG_RUNTIME_DIR") {
            Ok(dir) => {
                let path = std::path::Path::new(&dir);
                if path.is_dir() {
                    println!(
                        "{}",
                        crate::output::kv(
                            "XDG dir:",
                            &crate::output::ok_line(&format!("$XDG_RUNTIME_DIR = {dir}"))
                        )
                    );
                } else {
                    println!(
                        "{}",
                        crate::output::kv(
                            "XDG dir:",
                            &crate::output::fail_line(&format!(
                                "$XDG_RUNTIME_DIR = {dir} (not a directory)"
                            ))
                        )
                    );
                    ok = false;
                }
            }
            Err(_) => {
                println!(
                    "{}",
                    crate::output::kv(
                        "XDG dir:",
                        &crate::output::fail_line("$XDG_RUNTIME_DIR is not set")
                    )
                );
                println!(
                    "{}",
                    crate::output::kv(
                        "fix:",
                        &crate::output::tip_line("export XDG_RUNTIME_DIR=/run/user/$(id -u)")
                    )
                );
                ok = false;
            }
        }
    }

    // 3. Audio server check (night-improve-29: the probe now OPENS the
    //    device — a name without a working config was a false PASS on
    //    hosts whose `default` PCM cannot open — and the probe rides
    //    the quiet-ALSA guard so libasound's C-level error log cannot
    //    spew across the report. The compact render matches the
    //    overview's audio line, one shape family-wide).
    println!();
    #[cfg(target_os = "linux")]
    let _quiet = QuietAlsa::new();
    match cpal::default_host().default_output_device().map(|d| {
        // night-dinner-10: DeviceTrait::name() became description() in
        // cpal 0.18 (a DeviceDescription with a stable id); render the
        // same bare name the CLI contract always shown, same fallback.
        let name = d
            .description()
            .map(|desc| desc.name().to_owned())
            .unwrap_or_else(|_| "<unknown>".into());
        (name, d.default_output_config())
    }) {
        Some((name, Ok(config))) => {
            println!(
                "{}",
                crate::output::kv(
                    "audio:",
                    &crate::output::ok_line(&format!(
                        "default output device: {name} ({} Hz, {}ch, {:?})",
                        config.sample_rate(),
                        config.channels(),
                        config.sample_format()
                    ))
                )
            );
        }
        Some((name, Err(e))) => {
            println!(
                "{}",
                crate::output::kv(
                    "audio:",
                    &crate::output::fail_line(&format!(
                        "default output device ({name}) present but not openable: {e}"
                    ))
                )
            );
            println!(
                "{}",
                crate::output::kv(
                    "fix:",
                    &crate::output::tip_line("start PipeWire or PulseAudio")
                )
            );
            ok = false;
        }
        None => {
            println!(
                "{}",
                crate::output::kv(
                    "audio:",
                    &crate::output::fail_line("no audio output device found")
                )
            );
            println!(
                "{}",
                crate::output::kv(
                    "fix:",
                    &crate::output::tip_line("start PipeWire or PulseAudio")
                )
            );
            ok = false;
        }
    }

    // The report-level Warnings block (the zelynic shape): cautions
    // that belong to the whole report, rendered between the facts and
    // the exit verdict.
    if !warnings.is_empty() {
        println!();
        println!("Warnings:");
        for w in &warnings {
            println!("{}", crate::output::warn_bullet(w));
        }
    }

    println!();
    if ok {
        println!(
            "{}",
            crate::output::ok_line("All checks passed — zylaxion is ready to run.")
        );
    } else {
        println!(
            "{}",
            crate::output::fail_line("Some checks failed — see above for fixes.")
        );
        process::exit(1);
    }
}
