// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The `doctor` subcommand: the four-station system-health report.

use std::process;

use cpal::traits::{DeviceTrait, HostTrait};

use super::alsa_guard::QuietAlsa;

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
    //    floor): the whole station lives in doctor_kernel.rs now (the
    //    LOC-gate split, a pure move) - one probe feeds both this
    //    report and the attach path, so the two surfaces can never
    //    drift.
    if !super::doctor_kernel::kernel_station(root) {
        ok = false;
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
        // night-dragon-maintenance-4: the block's header rides the
        // warning tier with its rows — the caution block reads as one
        // yellow lane, scannable on a color terminal, byte-identical
        // plain otherwise.
        println!("{}", crate::output::warn_line("Warnings:"));
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
