// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Information subcommands: `doctor`, `list-profiles`, `list-backends`.

use std::process;

use cpal::traits::{DeviceTrait, HostTrait};

/// Print a system-health diagnostic report (input group, XDG, audio).
pub fn cmd_doctor() {
    println!("=== Zylaxion Doctor ===\n");

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

/// List available acoustic profiles and show their search-path order.
pub fn cmd_list_profiles() {
    println!("Available acoustic profiles:\n");
    println!("  technical    Crisp, loud, punchy (default)");
    println!("              Cherry MX Blue click with bright spring ring.");
    println!("  classic      Deeper, more resonant");
    println!("              Old bucklespring warmth, long spring sustain.");
    println!("  studio       Softer attack, longer decay");
    println!("              Gentle click for quiet office environments.");
    println!("  elegant      Very soft, muffled, polite");
    println!("              Subtle click for low-profile keyboards.");
    println!("  whisper      Extremely quiet, short decay");
    println!("              Barely audible — for libraries and meetings.");
    println!();
    println!("  Usage: zylaxion start --profile <name>");
    println!("         zylaxion daemon --profile <name>");
    println!();
    println!("  Profiles are loaded from (first found wins):");
    println!("    1. ~/.config/zylaxion/profiles/<name>.toml");
    println!("    2. /etc/zylaxion/profiles/<name>.toml");
    println!("    3. /usr/local/share/zylaxion/profiles/<name>.toml");
    println!("    4. ./profiles/<name>.toml (relative to CWD, for dev)");
    println!("    5. Hardcoded default (always available)");
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
