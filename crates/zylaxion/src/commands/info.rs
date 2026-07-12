// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Information subcommands: `doctor`, `testconf`, `list-presets`, `list-backends`.

use std::process;

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
        // Validate a specific file.
        let path = std::path::Path::new(path_str);
        if !path.is_file() {
            crate::error_format::error(format!("file not found: {path_str}"));
            process::exit(1);
        }
        match config::validate_config_file(path) {
            Ok(()) => {
                println!("Config OK: {}", path.display());
            }
            Err(err) => {
                crate::error_format::error(format!("in {}: {err}", path.display()));
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
