// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Information subcommands: `doctor`, `testconf`, `list-presets`, `list-backends`,
//! and the no-subcommand `overview`.

use std::process;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait};

use crate::config;

/// Print a system-health diagnostic report (input group, XDG, audio).
pub fn cmd_doctor() {
    println!("=== zylaxion doctor ===\n");

    let mut ok = true;

    // 1. Input group check.
    println!("[1/3] Input group membership...");
    if check_input_group() {
        println!("      ✓ user is in the 'input' group");
    } else {
        println!("      ✗ user is NOT in the 'input' group");
        println!("        fix: sudo usermod -aG input $USER");
        println!("        then log out and back in");
        ok = false;
    }

    // 2. XDG_RUNTIME_DIR check.
    println!("\n[2/3] XDG_RUNTIME_DIR...");
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => {
            let path = std::path::Path::new(&dir);
            if path.is_dir() {
                println!("      ✓ $XDG_RUNTIME_DIR = {dir}");
            } else {
                println!("      ✗ $XDG_RUNTIME_DIR = {dir} (not a directory)");
                ok = false;
            }
        }
        Err(_) => {
            println!("      ✗ $XDG_RUNTIME_DIR is not set");
            println!("        fix: export XDG_RUNTIME_DIR=/run/user/$(id -u)");
            ok = false;
        }
    }

    // 3. Audio server check (best-effort).
    println!("\n[3/3] Audio server...");
    match cpal::default_host().default_output_device() {
        Some(device) => {
            let name = device.name().unwrap_or_else(|_| "<unknown>".into());
            println!("      ✓ default output device: {name}");
        }
        None => {
            println!("      ✗ no audio output device found");
            println!("        fix: start PipeWire or PulseAudio");
            ok = false;
        }
    }

    println!();
    if ok {
        println!("All checks passed — zylaxion is ready to run.");
    } else {
        println!("Some checks failed — see above for fixes.");
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
                println!("Config OK: {}", canonical.display());
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
                println!("Config OK: {}", path.display());
            }
            Err((Some(path), err)) => {
                crate::error_format::error(format!("in {}: {err}", path.display()));
                process::exit(1);
            }
            Err((None, err)) => {
                crate::error_format::error(err);
                eprintln!();
                eprintln!("Searched:");
                eprintln!("  1. $XDG_CONFIG_HOME/zylaxion/config.toml");
                eprintln!("     (or ~/.config/zylaxion/config.toml if unset)");
                eprintln!("  2. /etc/zylaxion/config.toml");
                eprintln!("  3. /usr/local/share/zylaxion/config.toml");
                eprintln!("  4. ./config.toml (current directory)");
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
                let marker = if name == &active { " <- active" } else { "" };
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

    // Install a simple SIGINT handler so Ctrl+C exits cleanly
    // instead of panicking.
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, stop_clone);

    loop {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        // Clear screen + move cursor to top-left.
        print!("\x1b[2J\x1b[H");
        let _ = io::stdout().flush();

        // Print the overview.
        cmd_overview();

        // Print a footer with refresh hint.
        println!("  (refreshing every 2s — Ctrl+C to exit)");
        let _ = io::stdout().flush();

        // Sleep 2s, but check stop flag every 200ms for responsive exit.
        for _ in 0..10 {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    // Clear screen on exit for clean terminal.
    print!("\x1b[2J\x1b[H");
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
            if let Some(rest) = line.strip_prefix("input:x:") {
                if let Some(gid_str) = rest.split(':').next() {
                    if let Ok(input_gid) = gid_str.parse::<u32>() {
                        return user_groups.contains(&input_gid);
                    }
                }
            }
        }
    }
    false
}
