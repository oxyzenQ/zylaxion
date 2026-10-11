// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Information subcommands: `doctor`, `testconf`, `list-presets`, `list-backends`,
//! and the no-subcommand `overview`.

use std::process;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait};

use crate::config;

/// Print a system-health diagnostic report (build facts, input group,
/// XDG, audio).
///
/// The report leads with the BUILD facts block (night-improve-13, the
/// zelynic night-improve-66 doctor completeness): version, commit,
/// variant, build timestamp, rustc, profile — the binary answers "which
/// exact artifact am I?" before anything about the host, because on a
/// foreign build every host verdict below reads differently. Every fact
/// is compile-time truth assembled by [`crate::build_info`] (the
/// Hinnant clock stamps the build time, no time crate).
pub fn cmd_doctor() {
    println!(
        "{}\n",
        crate::error_format::section("=== zylaxion doctor ===")
    );

    let mut ok = true;

    // 0. Build facts (compile-time truth — cannot fail, never counts
    // against the ok verdict).
    let build = crate::build_info::BuildInfo::collect();
    println!("{}", crate::error_format::section("[1/4] Build facts..."));
    println!("      Version:    v{}", build.version);
    println!("      Commit:     {}", build.git_sha);
    println!("      Variant:    {}", build.variant);
    println!("      Built:      {}", build.build_time);
    println!("      Rustc:      {}", build.rustc_version);
    println!("      Profile:    {}", build.profile);

    // 1. Input group check.
    println!(
        "\n{}",
        crate::error_format::section("[2/4] Input group membership...")
    );
    if check_input_group() {
        println!(
            "      {}",
            crate::error_format::ok_line("PASS: user is in the 'input' group")
        );
    } else {
        println!(
            "      {}",
            crate::error_format::fail_line("FAIL: user is NOT in the 'input' group")
        );
        println!(
            "      {}",
            crate::error_format::tip_line("fix: sudo usermod -aG input $USER")
        );
        println!(
            "      {}",
            crate::error_format::tip_line("then log out and back in")
        );
        ok = false;
    }

    // 2. XDG_RUNTIME_DIR check.
    println!(
        "\n{}",
        crate::error_format::section("[3/4] XDG_RUNTIME_DIR...")
    );
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => {
            let path = std::path::Path::new(&dir);
            if path.is_dir() {
                println!(
                    "      {}",
                    crate::error_format::ok_line(&format!("PASS: $XDG_RUNTIME_DIR = {dir}"))
                );
            } else {
                println!(
                    "      {}",
                    crate::error_format::fail_line(&format!(
                        "FAIL: $XDG_RUNTIME_DIR = {dir} (not a directory)"
                    ))
                );
                ok = false;
            }
        }
        Err(_) => {
            println!(
                "      {}",
                crate::error_format::fail_line("FAIL: $XDG_RUNTIME_DIR is not set")
            );
            println!(
                "      {}",
                crate::error_format::tip_line("fix: export XDG_RUNTIME_DIR=/run/user/$(id -u)")
            );
            ok = false;
        }
    }

    // 3. Audio server check (best-effort).
    println!(
        "\n{}",
        crate::error_format::section("[4/4] Audio server...")
    );
    match cpal::default_host().default_output_device() {
        Some(device) => {
            let name = device.name().unwrap_or_else(|_| "<unknown>".into());
            println!(
                "      {}",
                crate::error_format::ok_line(&format!("PASS: default output device: {name}"))
            );
        }
        None => {
            println!(
                "      {}",
                crate::error_format::fail_line("FAIL: no audio output device found")
            );
            println!(
                "      {}",
                crate::error_format::tip_line("fix: start PipeWire or PulseAudio")
            );
            ok = false;
        }
    }

    println!();
    if ok {
        println!(
            "{}",
            crate::error_format::ok_line("All checks passed — zylaxion is ready to run.")
        );
    } else {
        println!(
            "{}",
            crate::error_format::fail_line("Some checks failed — see above for fixes.")
        );
        process::exit(1);
    }
}

/// Validate the central `config.toml`: find it via the search path,
/// parse, and run all DSP parameter clamping checks.
///
/// If `file` is `Some(path)`, validates that specific file instead of
/// searching the standard config paths.
///
/// Exits 0 with "Config OK: <path>" if everything parses and validates.
/// Exits 1 with "Config Error in <path>: <error>" otherwise.
///
/// Equivalent in spirit to `nginx -t` or `sshd -t` — lets users catch
/// TOML typos and out-of-bounds DSP values before restarting the daemon.
pub fn cmd_testconf(file: Option<&str>) {
    if let Some(path_str) = file {
        let path = std::path::Path::new(path_str);

        // v10.2.0: reject files without .toml extension.
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            crate::error_format::error(format!("file must have .toml extension: {path_str}"));
            process::exit(1);
        }

        // v10.2.0: only allow reading from the standard config
        // directories — not arbitrary paths like /etc/unbound/.
        let canonical = match path.canonicalize() {
            Ok(c) => c,
            Err(_) => {
                crate::error_format::error(format!("file not found: {path_str}"));
                process::exit(1);
            }
        };

        let allowed_dirs: Vec<std::path::PathBuf> = {
            let mut dirs = Vec::new();
            // $XDG_CONFIG_HOME/zylaxion or ~/.config/zylaxion
            if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME")
                .filter(|v| !v.is_empty())
                .map(std::path::PathBuf::from)
                .filter(|p| p.is_absolute())
            {
                dirs.push(xdg.join("zylaxion"));
            } else if let Some(home) = std::env::var_os("HOME") {
                dirs.push(std::path::PathBuf::from(home).join(".config/zylaxion"));
            }
            dirs.push(std::path::PathBuf::from("/etc/zylaxion"));
            dirs.push(std::path::PathBuf::from("/usr/local/share/zylaxion"));
            // Also allow the current directory (for development).
            if let Ok(cwd) = std::env::current_dir() {
                dirs.push(cwd);
            }
            dirs
        };

        let is_allowed = allowed_dirs.iter().any(|dir| {
            if let Ok(dir_canon) = dir.canonicalize() {
                canonical.starts_with(&dir_canon)
            } else {
                canonical.starts_with(dir)
            }
        });

        if !is_allowed {
            crate::error_format::error(format!(
                "file must be within a zylaxion config directory \
                 (~/.config/zylaxion/, /etc/zylaxion/, /usr/local/share/zylaxion/, or CWD): {path_str}"
            ));
            process::exit(1);
        }

        match config::validate_config_file(&canonical) {
            Ok(()) => {
                println!(
                    "{}",
                    crate::error_format::ok_line(&format!("Config OK: {}", canonical.display()))
                );
            }
            Err(err) => {
                crate::error_format::error(format!("in {}: {err}", canonical.display()));
                process::exit(1);
            }
        }
    } else {
        // Search the standard paths.
        match config::validate_config() {
            Ok(path) => {
                println!(
                    "{}",
                    crate::error_format::ok_line(&format!("Config OK: {}", path.display()))
                );
            }
            Err((Some(path), err)) => {
                crate::error_format::error(format!("in {}: {err}", path.display()));
                process::exit(1);
            }
            Err((None, err)) => {
                crate::error_format::error(format!(
                    "{err}\n\nSearched:\n  1. $XDG_CONFIG_HOME/zylaxion/config.toml\n     (or ~/.config/zylaxion/config.toml if unset)\n  2. /etc/zylaxion/config.toml\n  3. /usr/local/share/zylaxion/config.toml\n  4. ./config.toml (current directory)"
                ));
                process::exit(1);
            }
        }
    }
}

/// List all acoustic presets defined in `config.toml` and mark the
/// active one (based on the `preset.tuning` value in the file).
///
/// Exits 0 on success, 1 if no config file is found or parsing fails.
pub fn cmd_list_presets() {
    match config::list_presets() {
        Ok((path, active, names)) => {
            println!("Available presets (from {}):\n", path.display());
            for name in &names {
                let marker = if name == &active {
                    crate::error_format::ok_marker(" <- active")
                } else {
                    String::new()
                };
                println!("  {name}{marker}");
            }
            println!();
            println!("Active preset: {active}");
            println!();
            println!("Switch preset:");
            println!("  Edit `tuning = \"<name>\"` in config.toml, OR");
            println!("  Run: zylaxion start --preset <name>");
        }
        Err(e) => {
            crate::error_format::error(e);
            process::exit(1);
        }
    }
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
/// - Active preset (from config.toml)
/// - Audio device (name, sample rate, channels)
/// - Config file path
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
    match config::list_presets() {
        Ok((path, active, _names)) => {
            println!("  preset:  {active}");
            println!("  config:  {}", path.display());
        }
        Err(_) => {
            println!("  preset:  <no config found>");
        }
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
