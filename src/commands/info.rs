// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Information subcommands: `doctor`, `list-presets`, `list-backends`,
//! and the no-subcommand `overview`.
//!
//! night-improve-22: the `testconf` subcommand died with `config.toml`
//! — there is no file left to validate. The compiled preset registry
//! is type-checked Rust data, proven in-range by the registry test
//! tree; a validator command for it would validate nothing.

use std::process;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait};

use crate::session::preset;

/// Print a system-health diagnostic report (build facts, input group,
/// XDG, audio).
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
    println!("      Built:      {}", build.build_time);
    println!("      Rustc:      {}", build.rustc_version);
    println!("      Profile:    {}", build.profile);

    // 1. Kernel / eBPF capability (night-improve-23, the eBPF-only
    //    floor): the input tap is a BPF tracepoint program and the
    //    state store is pinned maps on bpffs — the kernel IS the
    //    platform now. The owner's minimum is 5.15; below it the
    //    verdict is "verified unsupported" and nothing else in this
    //    report can save the run. One probe feeds both this report
    //    and the attach path, so the two surfaces can never drift.
    let kernel = crate::session::kernel::probe();
    println!(
        "\n{}",
        crate::output::section("[2/5] Kernel / eBPF capability...")
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
        // BTF is optional for the tracepoint tap (no CO-RE
        // introspection needed) — reported for completeness, never
        // fatal.
        println!(
            "      BTF:      {} (not required for the tracepoint tap)",
            if kernel.btf { "present" } else { "absent" }
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
        if kernel.tracepoint_visible {
            println!(
                "      {}",
                crate::output::ok_line("PASS: tap: input_event tracepoint visible")
            );
        } else {
            println!(
                "      note: input_event tracepoint not visible from this session (tracefs may be restricted) — the attach attempt is the authoritative verdict"
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
    }

    // 2. Input group check.
    println!(
        "\n{}",
        crate::output::section("[3/5] Input group membership...")
    );
    if check_input_group() {
        println!(
            "      {}",
            crate::output::ok_line("PASS: user is in the 'input' group")
        );
    } else {
        println!(
            "      {}",
            crate::output::fail_line("FAIL: user is NOT in the 'input' group")
        );
        println!(
            "      {}",
            crate::output::tip_line("fix: sudo usermod -aG input $USER")
        );
        println!(
            "      {}",
            crate::output::tip_line("then log out and back in")
        );
        ok = false;
    }

    // 3. XDG_RUNTIME_DIR check.
    println!("\n{}", crate::output::section("[4/5] XDG_RUNTIME_DIR..."));
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

    // 4. Audio server check (best-effort).
    println!("\n{}", crate::output::section("[5/5] Audio server..."));
    match cpal::default_host().default_output_device() {
        Some(device) => {
            let name = device.name().unwrap_or_else(|_| "<unknown>".into());
            println!(
                "      {}",
                crate::output::ok_line(&format!("PASS: default output device: {name}"))
            );
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

/// List every acoustic preset in the compiled registry and mark the
/// one the session context would activate right now (the
/// `ZYLAXION_PRESET` selection when it names a registered preset,
/// the compiled default otherwise).
///
/// Exits 0 always — the registry is compile-time data; there is no
/// file to fail finding and no parse to fail.
pub fn cmd_list_presets() {
    let active = preset::context_preset_name();
    println!("Available presets (compiled into this binary):\n");
    for entry in crate::profiles::registry_all() {
        let marker = if entry.name == active {
            crate::output::ok_marker(" <- active")
        } else {
            String::new()
        };
        println!("  {}{marker}", entry.name);
        println!("      {}", entry.description);
    }
    println!();
    println!("Active preset: {active}");
    println!();
    println!("Switch preset:");
    println!("  export {}=\"<name>\", OR", preset::PRESET_ENV_VAR);
    println!("  Run: zylaxion start --preset <name>");
    println!();
    println!(
        "Volume (headphones want less): export {}=1.5",
        preset::VOLUME_ENV_VAR
    );
}

/// List available audio backends via cpal.
pub fn cmd_list_backends() {
    let host = cpal::default_host();
    println!("Audio host: {}", host.id().name());

    if let Some(device) = host.default_output_device() {
        let name = device.name().unwrap_or_else(|_| "<unknown>".into());
        println!("Default output: {name}");

        if let Ok(config) = device.default_output_config() {
            println!("  Sample rate: {} Hz", config.sample_rate().0);
            println!("  Channels: {}", config.channels());
            println!("  Format: {:?}", config.sample_format());
        }
    } else {
        println!("No default output device.");
    }
}

/// Quick status overview shown when `zylaxion` is run with no
/// subcommand (v10.2.0+ — user feedback: "should zylaxion can use
/// verbose because user can see the daemon running, info, other
/// metrics").
///
/// Prints a one-screen summary:
/// - Daemon status (running/stopped, PID)
/// - Active preset (the session context selection against the
///   compiled registry — no config file since night-improve-22)
/// - Audio device (name, sample rate, channels)
/// - Quick-start hint
pub fn cmd_overview() {
    println!("=== zylaxion ===\n");

    // 1. Daemon status.
    let daemon_running = match crate::daemon::is_daemon_running() {
        Ok(pid) => {
            println!("  daemon:  running (PID: {})", pid.as_raw());
            true
        }
        Err(_) => {
            println!("  daemon:  not running");
            false
        }
    };
    println!(
        "  preset:  {} (compiled registry)",
        preset::context_preset_name()
    );
    match preset::master_volume() {
        Ok(volume) => {
            let source = std::env::var(preset::VOLUME_ENV_VAR)
                .map(|_| "ZYLAXION_VOLUME")
                .unwrap_or_else(|_| "default");
            println!("  volume:  {volume} ({source})");
        }
        Err(_) => println!("  volume:  invalid ZYLAXION_VOLUME (start will refuse)"),
    }

    // 3. Audio device.
    let host = cpal::default_host();
    match host.default_output_device() {
        Some(device) => {
            let name = device.name().unwrap_or_else(|_| "<unknown>".into());
            if let Ok(dev_config) = device.default_output_config() {
                println!(
                    "  audio:   {name} ({} Hz, {}ch, {:?})",
                    dev_config.sample_rate().0,
                    dev_config.channels(),
                    dev_config.sample_format()
                );
            } else {
                println!("  audio:   {name} (config unavailable)");
            }
        }
        None => {
            println!("  audio:   no device found");
        }
    }

    // 4. Input group.
    if check_input_group() {
        println!("  input:   in 'input' group");
    } else {
        println!("  input:   NOT in 'input' group (fix: sudo usermod -aG input $USER)");
    }

    println!();
    if daemon_running {
        println!("  zylaxion stop          # stop daemon");
        println!("  zylaxion start         # foreground mode");
        println!("  zylaxion list-presets  # switch sound");
    } else {
        println!("  zylaxion daemon        # start background daemon");
        println!("  zylaxion start         # foreground mode");
        println!("  zylaxion doctor        # system check");
    }
    println!();
}

/// Live overview: refresh the status every 2 seconds (like `watch`).
/// Clears the screen + reprints the overview. Press Ctrl+C to exit.
/// (v10.2.0+ — user feedback: `--live` flag)
pub fn cmd_live_overview() {
    use std::io::{self, Write};

    // Install SIGINT + SIGTERM handler so Ctrl+C exits cleanly.
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop_clone));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, stop_clone);

    // Enter alternate screen — like vim/less. On exit we restore the
    // original screen, so the terminal looks clean (no scrollback
    // pollution from repeated overwrites).
    // ESC[?1049h = save cursor + switch to alternate screen
    // ESC[?1049l = restore cursor + switch back to main screen
    print!("\x1b[?1049h");
    // Hide cursor while in live mode.
    print!("\x1b[?25l");
    let _ = io::stdout().flush();

    loop {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        // Move cursor to top-left + clear from cursor down.
        // (Don't clear the whole screen — that causes flicker.)
        print!("\x1b[H\x1b[J");
        let _ = io::stdout().flush();

        // Print the overview.
        cmd_overview();

        // Footer with refresh hint.
        println!("  (refreshing every 2s — Ctrl+C to exit)");
        let _ = io::stdout().flush();

        // Sleep 2s, checking stop flag every 100ms for fast exit.
        for _ in 0..20 {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    // Restore: show cursor + leave alternate screen.
    print!("\x1b[?25h\x1b[?1049l");
    let _ = io::stdout().flush();
}

/// Check if the current user is in the 'input' group by parsing
/// `/etc/group` (no unsafe FFI needed).
fn check_input_group() -> bool {
    let user_groups: Vec<u32> = nix::unistd::getgroups()
        .map(|g| g.iter().map(|gid| gid.as_raw()).collect())
        .unwrap_or_default();

    if let Ok(content) = std::fs::read_to_string("/etc/group") {
        for line in content.lines() {
            // night-improve-4 (edition 2024): let-chains collapse the
            // nested guards clippy flagged as collapsible_if.
            if let Some(rest) = line.strip_prefix("input:x:")
                && let Some(gid_str) = rest.split(':').next()
                && let Ok(input_gid) = gid_str.parse::<u32>()
            {
                return user_groups.contains(&input_gid);
            }
        }
    }
    false
}
