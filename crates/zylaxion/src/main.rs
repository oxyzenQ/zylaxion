// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! **Zylaxion** — real-time mechanical keyboard acoustic synthesizer for Linux.
//!
//! Transforms every keystroke into a spatially-accurate click sound
//! through your speakers, using the kernel's evdev interface and
//! low-latency audio via cpal / PipeWire.
//!
//! # Subcommands
//!
//! - `start`   — Run in the foreground (Ctrl+C to quit).
//! - `daemon`  — Fork into the background, controlled via Unix socket.
//! - `stop`    — Tell a running daemon to shut down.
//! - `status`  — Query whether a daemon is running.
//! - `doctor`  — Print a system-health diagnostic report.
//! - `list-profiles`   — Show available acoustic profiles.
//! - `list-backends`   — Show available audio backends (via cpal).

mod daemon;

use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait};
use zactrix_profiles::{load_profile_from_file, KeyProfile, MechanicalClick};
use zylaxion_core::Orchestrator;
use zylaxion_input::{InputSource, LibinputSource};

/// Build hash placeholder — replaced at release time by CI.
const BUILD_HASH: &str = "a1b2c3d";

// ── CLI ─────────────────────────────────────────────────────────────────

/// Zylaxion — mechanical keyboard acoustic synthesizer
#[derive(Parser)]
#[command(name = "zylaxion", version, about)]
#[command(after_help = "License: GPL-3.0-or-later | https://github.com/oxyzenQ/zylaxion")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run in the foreground (press Ctrl+C to quit)
    Start {
        /// Acoustic profile name (e.g. technical, classic, studio, elegant, whisper)
        #[arg(long, global = true)]
        profile: Option<String>,
    },

    /// Run as a background daemon (controlled via Unix socket)
    Daemon {
        /// Acoustic profile name (e.g. technical, classic, studio, elegant, whisper)
        #[arg(long, global = true)]
        profile: Option<String>,
    },

    /// Stop a running daemon
    Stop,

    /// Show daemon status
    Status,

    /// Print system health diagnostic
    Doctor,

    /// List available acoustic profiles
    ListProfiles,

    /// List available audio backends
    ListBackends,
}

// ── Entrypoint ──────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    // Custom -V / --version handler.
    if let Some("version") = std::env::args().nth(1).as_deref() {
        print_version();
        return;
    }

    match cli.command {
        Commands::Start { profile } => cmd_start(profile),
        Commands::Daemon { profile } => cmd_daemon(profile),
        Commands::Stop => cmd_stop(),
        Commands::Status => daemon::cmd_status(),
        Commands::Doctor => cmd_doctor(),
        Commands::ListProfiles => cmd_list_profiles(),
        Commands::ListBackends => cmd_list_backends(),
    }
}

// ── Version ─────────────────────────────────────────────────────────────

fn print_version() {
    println!("Version: v{}", env!("CARGO_PKG_VERSION"));
    println!("Build: linux-x86_64 ({BUILD_HASH})");
    println!("Copyright: (c) 2026 rezky_nightky (oxyzenQ)");
    println!("License: GPL-3.0-or-later");
    println!("Source: https://github.com/oxyzenQ/zylaxion");
}

// ── Profile resolution ─────────────────────────────────────────────────

/// Resolve an acoustic profile by name.
///
/// Search order (first found wins):
///   1. `~/.config/zylaxion/profiles/<name>.toml`  (user-local)
///   2. `/etc/zylaxion/profiles/<name>.toml`       (system-wide)
///   3. Hardcoded default                          (always available)
///
/// If `name` is `None`, returns the hardcoded default immediately.
fn resolve_profile(name: &Option<String>) -> KeyProfile {
    let name = match name.as_deref() {
        Some(n) => n,
        None => return KeyProfile::default(),
    };

    let toml_name = format!("{name}.toml");

    // 1. User-local: ~/.config/zylaxion/profiles/<name>.toml
    if let Some(home) = std::env::var_os("HOME") {
        let user_path = std::path::PathBuf::from(home)
            .join(".config/zylaxion/profiles")
            .join(&toml_name);
        if user_path.is_file() {
            match load_profile_from_file(&user_path) {
                Ok(p) => {
                    log::info!("loaded profile '{}' from {}", name, user_path.display());
                    return p;
                }
                Err(e) => {
                    eprintln!("[zylaxion] warning: {e}");
                }
            }
        }
    }

    // 2. System-wide: /etc/zylaxion/profiles/<name>.toml
    let system_path = std::path::PathBuf::from("/etc/zylaxion/profiles").join(&toml_name);
    if system_path.is_file() {
        match load_profile_from_file(&system_path) {
            Ok(p) => {
                log::info!("loaded profile '{}' from {}", name, system_path.display());
                return p;
            }
            Err(e) => {
                eprintln!("[zylaxion] warning: {e}");
            }
        }
    }

    // 3. Fallback: hardcoded default.
    eprintln!(
        "[zylaxion] profile '{}' not found — using default profile",
        name
    );
    KeyProfile::default()
}

// ── Subcommands ─────────────────────────────────────────────────────────

fn cmd_start(profile_name: Option<String>) {
    env_logger::init();

    // Resolve the acoustic profile (TOML or hardcoded default).
    let profile = resolve_profile(&profile_name);
    log::info!(
        "starting zylaxion in foreground mode (profile: {})",
        profile_name.as_deref().unwrap_or("default")
    );

    // Mirror the zylaxion_live example exactly.
    //
    // 1. Start input capture (background thread).
    let mut input_source = LibinputSource::new();
    let event_rx = match input_source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            eprintln!("[zylaxion] input error: {e}");
            process::exit(1);
        }
    };

    // 2. Create the orchestrator (audio + engine).
    let mut orchestrator = match Orchestrator::new() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[zylaxion] audio error: {e}");
            eprintln!("[zylaxion] make sure PipeWire or PulseAudio is running");
            process::exit(1);
        }
    };

    // 3. Run the main loop on the MAIN thread (blocks until Ctrl+C).
    let stop_flag = Arc::new(AtomicBool::new(false));
    let model = MechanicalClick::with_profile(profile);
    log::info!("ready — press any key to hear it (Ctrl+C to quit)");

    orchestrator.run(&model, &event_rx, stop_flag);

    log::info!("shutdown complete");
}

fn cmd_daemon(profile_name: Option<String>) {
    // Check if already running (with /proc/<pid>/comm PID recycling check).
    if daemon::is_daemon_running().is_ok() {
        eprintln!("error: zylaxion daemon is already running");
        process::exit(1);
    }

    // ── Fork FIRST — do NOT touch audio/input before forking. ──
    // daemonize() prints the child PID and the parent exits with 0.
    if let Err(e) = daemon::daemonize() {
        eprintln!("error: daemonize failed: {e}");
        process::exit(1);
    }

    // We are now the daemon child.  Close inherited std fds so
    // we cannot read from or write to the controlling terminal.
    daemon::close_std_fds();

    // Initialise logging (stderr is now /dev/null — messages are
    // silently discarded; wire to syslog in a future iteration).
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    log::info!("daemon started (PID: {})", nix::unistd::getpid().as_raw());

    // Write PID file.
    if let Err(_e) = daemon::write_pid_file() {
        // Can't log — stderr is /dev/null.  Just exit.
        process::exit(1);
    }

    // Ignore SIGHUP / SIGPIPE so the daemon survives terminal closure
    // and broken socket writes.
    daemon::ignore_hup_pipe();

    // Shared stop flag: IPC thread and signal handlers both set this.
    // The orchestrator main loop checks it every iteration.
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Install signal handlers (SIGTERM / SIGINT → set stop_flag).
    daemon::install_signal_handlers(Arc::clone(&stop_flag));

    // Create IPC socket.
    let listener = match daemon::ipc::create_listener() {
        Ok(fd) => fd,
        Err(e) => {
            log::error!("IPC setup failed: {e}");
            daemon::cleanup();
            process::exit(1);
        }
    };
    log::info!("IPC socket ready: {}", daemon::ipc::socket_path().display());

    // Resolve the acoustic profile (TOML or hardcoded default).
    let profile = resolve_profile(&profile_name);
    log::info!(
        "using profile: {}",
        profile_name.as_deref().unwrap_or("default")
    );

    // ── NOW initialize audio (mirror zylaxion_live exactly) ──
    // Everything below runs ONLY in the detached child process.

    // 1. Start input capture (background thread).
    let mut input_source = LibinputSource::new();
    let event_rx = match input_source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            log::error!("input error: {e}");
            daemon::cleanup();
            process::exit(1);
        }
    };

    // 2. Create the orchestrator (audio + engine).
    let mut orchestrator = match Orchestrator::new() {
        Ok(o) => o,
        Err(e) => {
            log::error!("audio error: {e}");
            daemon::cleanup();
            process::exit(1);
        }
    };

    // Spawn IPC listener on a BACKGROUND thread.
    // The orchestrator runs on the MAIN thread — same as zylaxion_live.
    let _ipc_handle = daemon::spawn_ipc_thread(listener, Arc::clone(&stop_flag));

    // 3. Run the audio loop on the MAIN thread (blocks until stop
    //    or channel disconnect).  When this returns, CpalSink is
    //    dropped, releasing the PipeWire audio device.
    let model = MechanicalClick::with_profile(profile);
    orchestrator.run(&model, &event_rx, stop_flag);

    // Clean shutdown.
    log::info!("shutting down");
    daemon::cleanup();
}

fn cmd_stop() {
    daemon::client_send_and_print("stop");
}

fn cmd_doctor() {
    println!("=== Zylaxion Doctor ===\n");

    let mut ok = true;

    // 1. Input group check — read /proc/self/status groups and compare
    // against the 'input' group GID from /etc/group.
    println!("[1/3] Input group membership...");
    let in_input_group = check_input_group();

    if in_input_group {
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

fn cmd_list_profiles() {
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
    println!("    3. Hardcoded default (always available)");
}

fn cmd_list_backends() {
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

// ── Doctor helpers ──────────────────────────────────────────────────────

/// Check if the current user is in the 'input' group by parsing
/// /etc/group (no unsafe FFI needed).
fn check_input_group() -> bool {
    // Get current user's groups via nix.
    let user_groups: Vec<u32> = nix::unistd::getgroups()
        .map(|g| g.iter().map(|gid| gid.as_raw()).collect())
        .unwrap_or_default();

    // Parse /etc/group to find the 'input' group GID.
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

// ── Custom version ──────────────────────────────────────────────────────

// Override clap's built-in version to match the exact format.
#[allow(dead_code)]
impl Cli {
    fn version(&self) -> String {
        format!("v{}", env!("CARGO_PKG_VERSION"))
    }
}
