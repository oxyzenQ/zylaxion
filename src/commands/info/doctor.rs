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
/// zelynic night-improve-66 doctor completeness): version, commit,
/// variant, build timestamp, rustc, profile — the binary answers "which
/// exact artifact am I?" before anything about the host, because on a
/// foreign build every host verdict below reads differently. Every fact
/// is compile-time truth assembled by [`crate::info::build`] (the
/// Hinnant clock stamps the build time, no time crate).
pub fn cmd_doctor() {
    println!("{}\n", crate::output::section("=== zylaxion doctor ==="));

    // Privilege awareness (night-improve-16, the audit's warn lane):
    // doctor stays reachable as root (a diagnostic must diagnose), but
    // every verdict below would describe ROOT's session — root is
    // always "in" the input group, root's audio server is a different
    // instance, root's XDG_RUNTIME_DIR may not even be the user's.
    // Name the distortion up front so the report cannot mislead.
    if crate::session::privilege::euid_is_root() {
        crate::output::warning(
            "running as root — the verdicts below describe root's session, not yours",
        );
        println!(
            "      {}",
            crate::output::tip_line("tip: re-run without sudo")
        );
        println!();
    }

    let mut ok = true;

    // 0. Build facts (compile-time truth — cannot fail, never counts
    // against the ok verdict).
    let build = crate::info::build::BuildInfo::collect();
    println!("{}", crate::output::section("[1/4] Build facts..."));
    println!("      Version:    v{}", build.version);
    println!("      Commit:     {}", build.git_sha);
    println!("      Variant:    {}", build.variant);
    // night-hunter-7: the binary's own build-flavor verdict, the
    // zelynic doctor lineage — full-life (the eBPF objects are inside)
    // or half-life (a --no-default-features build; every verdict below
    // still prints, but the tap surfaces they describe are absent).
    // cfg! is evaluated at compile time: the answer is baked into the
    // binary next to the objects it describes, and the lane detail is
    // the ZYLAXION_EBPF_LANE stamp build.rs emits (source-built,
    // registry-prebuilt, or the dormant refusal).
    let lane = option_env!("ZYLAXION_EBPF_LANE").unwrap_or("unknown");
    if cfg!(feature = "ebpf") {
        println!(
            "      Build:      {} (eBPF objects: {lane})",
            crate::output::ok_line("FULL-LIFE")
        );
    } else {
        println!(
            "      Build:      {} (eBPF objects: {lane})",
            crate::output::warn_line("HALF-LIFE")
        );
        println!(
            "      {}",
            crate::output::tip_line(
                "this build carries no tap — install a full-life binary \
                 (default features or a release tarball)"
            )
        );
    }
    println!("      Built:      {}", build.build_time);
    println!("      Rustc:      {}", build.rustc_version);
    println!("      Profile:    {}", build.profile);

    // 1. Kernel / eBPF capability (night-improve-23, the eBPF-only
    //    floor): the input tap is a passive kernel-side probe on the
    //    exported input_event() function and the state store is pinned
    //    maps on bpffs — the kernel IS the
    //    platform now. The owner's minimum is 5.15; below it the
    //    verdict is "verified unsupported" and nothing else in this
    //    report can save the run. One probe feeds both this report
    //    and the attach path, so the two surfaces can never drift.
    let kernel = crate::session::kernel::probe();
    println!(
        "\n{}",
        crate::output::section("[2/4] Kernel / eBPF capability...")
    );
    if kernel.release.is_empty() {
        println!(
            "      {}",
            crate::output::fail_line(
                "FAIL: kernel release unreadable (/proc/sys/kernel/osrelease)"
            )
        );
        ok = false;
    } else {
        println!("      Kernel:   {}", kernel.release);
        if kernel.supported() {
            println!(
                "      {}",
                crate::output::ok_line(&format!(
                    "PASS: eBPF: {} ({} meets the 5.15 minimum)",
                    kernel.verdict(),
                    kernel.release
                ))
            );
        } else {
            println!(
                "      {}",
                crate::output::fail_line(&format!(
                    "FAIL: eBPF: {} ({} is below the 5.15 minimum — zylaxion is eBPF-only, upgrade the kernel)",
                    kernel.verdict(),
                    kernel.release
                ))
            );
            ok = false;
        }
        // BTF: the fentry lane's enabler (night-improve-25). The chain
        // does not NEED it — the kprobe steps run without BTF — so it
        // is reported for completeness, never fatal.
        println!(
            "      BTF:      {} ({})",
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
        );
        if kernel.bpffs {
            println!(
                "      {}",
                crate::output::ok_line("PASS: bpffs: mounted at /sys/fs/bpf")
            );
        } else {
            println!(
                "      {}",
                crate::output::fail_line(
                    "FAIL: bpffs: not mounted at /sys/fs/bpf — the pinned state store has nowhere to live"
                )
            );
            println!(
                "      {}",
                crate::output::tip_line(
                    "fix: sudo mount -t bpf bpf /sys/fs/bpf (normally automatic)"
                )
            );
            ok = false;
        }
        // The input-tap verdict, rendered VERBATIM (night-improve-25):
        // the probe's preflight of the fentry → kprobe chain — one of
        // the four canonical strings, self-explaining, and the doctor
        // adds nothing to it (the attach attempt stays the
        // authoritative verdict).
        println!("      {}", kernel.input_tap);
        // The last attach failure, if one left its marker
        // (night-improve-25): a failed attach plants a small pinned map
        // carrying the classified error code; naming it here is the
        // doctor's honesty duty — the tap is not attached, and this
        // line says exactly which step of the chain failed and how it
        // was classified.
        #[cfg(feature = "ebpf")]
        if let Some(err) = crate::ebpf::loader::last_attach_error() {
            println!(
                "      {}",
                crate::output::tip_line(&format!(
                    "note: last attach failed: {err} — `sudo zylaxion attach` prints the \
                     per-step log"
                ))
            );
        }
        if kernel.lockdown_readable && kernel.lockdown_blocks_bpf {
            println!(
                "      {}",
                crate::output::fail_line(
                    "FAIL: lockdown: 'confidentiality' active — the kernel withdraws BPF"
                )
            );
            println!(
                "      {}",
                crate::output::tip_line(
                    "fix: choose a kernel lockdown mode that allows BPF (integrity or none)"
                )
            );
            ok = false;
        }
        // The unprivileged-bpf posture (night-badas-4 round 5): the
        // control-plane verbs run as a plain user only when the kernel
        // allows unprivileged bpf(). A distro that builds the gate in
        // (Ubuntu does, CONFIG_BPF_UNPRIV_DEFAULT_OFF=y) locks the
        // verbs out for the whole boot — the doctor's duty is to name
        // it, the same way lockdown is named.
        if kernel.unpriv_bpf_allowed() {
            println!(
                "      {}",
                crate::output::ok_line(
                    "PASS: unprivileged bpf: allowed — the verbs run as a plain user"
                )
            );
        } else {
            println!(
                "      {}",
                crate::output::fail_line(&format!("FAIL: {}", kernel.unpriv_bpf_verdict()))
            );
            ok = false;
        }
    }

    // 3. XDG_RUNTIME_DIR check.
    // The audio-session posture (night-total-lts-2): the user's audio
    // server exposes its sockets under $XDG_RUNTIME_DIR, so a missing
    // or bogus value breaks the player's audio-server connection even
    // though zylaxion itself writes no files there (the kernel control
    // map is the pidfile since night-improve-23 — RULES.md's file
    // inventory names it).
    println!("\n{}", crate::output::section("[3/4] XDG_RUNTIME_DIR..."));
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => {
            let path = std::path::Path::new(&dir);
            if path.is_dir() {
                println!(
                    "      {}",
                    crate::output::ok_line(&format!("PASS: $XDG_RUNTIME_DIR = {dir}"))
                );
            } else {
                println!(
                    "      {}",
                    crate::output::fail_line(&format!(
                        "FAIL: $XDG_RUNTIME_DIR = {dir} (not a directory)"
                    ))
                );
                ok = false;
            }
        }
        Err(_) => {
            println!(
                "      {}",
                crate::output::fail_line("FAIL: $XDG_RUNTIME_DIR is not set")
            );
            println!(
                "      {}",
                crate::output::tip_line("fix: export XDG_RUNTIME_DIR=/run/user/$(id -u)")
            );
            ok = false;
        }
    }

    // 4. Audio server check (night-improve-29: the probe now OPENS the
    //    device — a name without a working config was a false PASS on
    //    hosts whose `default` PCM cannot open — and the probe rides
    //    the quiet-ALSA guard so libasound's C-level error log cannot
    //    spew across the report. The compact render matches the
    //    overview's audio line, one shape family-wide).
    println!("\n{}", crate::output::section("[4/4] Audio server..."));
    #[cfg(target_os = "linux")]
    let _quiet = QuietAlsa::new();
    match cpal::default_host().default_output_device().map(|d| {
        let name = d.name().unwrap_or_else(|_| "<unknown>".into());
        (name, d.default_output_config())
    }) {
        Some((name, Ok(config))) => {
            println!(
                "      {}",
                crate::output::ok_line(&format!(
                    "PASS: default output device: {name} ({} Hz, {}ch, {:?})",
                    config.sample_rate().0,
                    config.channels(),
                    config.sample_format()
                ))
            );
        }
        Some((name, Err(e))) => {
            println!(
                "      {}",
                crate::output::fail_line(&format!(
                    "FAIL: default output device ({name}) present but not openable: {e}"
                ))
            );
            println!(
                "      {}",
                crate::output::tip_line("fix: start PipeWire or PulseAudio")
            );
            ok = false;
        }
        None => {
            println!(
                "      {}",
                crate::output::fail_line("FAIL: no audio output device found")
            );
            println!(
                "      {}",
                crate::output::tip_line("fix: start PipeWire or PulseAudio")
            );
            ok = false;
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
