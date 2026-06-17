// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Foreground and daemon subcommands: `start`, `daemon`, `stop`.

use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use zactrix_profiles::MechanicalClick;
use zylaxion_core::Orchestrator;
use zylaxion_input::{InputSource, LibinputSource};

use crate::daemon;
use crate::profile::resolve_profile;

/// Run zylaxion in the foreground (Ctrl+C to quit).
///
/// Mirrors the `zylaxion_live` example: LibinputSource on stack,
/// Orchestrator::run on main thread.
pub fn cmd_start(profile_name: Option<String>) {
    env_logger::init();

    let profile = resolve_profile(&profile_name);
    log::info!(
        "starting zylaxion in foreground mode (profile: {})",
        profile_name.as_deref().unwrap_or("default")
    );

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

/// Run zylaxion as a POSIX background daemon.
///
/// Forks FIRST (no audio/input before fork), then the child initialises
/// hardware, writes the PID file, installs signal handlers, and runs the
/// orchestrator loop.
pub fn cmd_daemon(profile_name: Option<String>) {
    // Check if already running (with /proc/<pid>/comm PID recycling check).
    if daemon::is_daemon_running().is_ok() {
        eprintln!("error: zylaxion daemon is already running");
        process::exit(1);
    }

    // Fork FIRST — do NOT touch audio/input before forking.
    if let Err(e) = daemon::daemonize() {
        eprintln!("error: daemonize failed: {e}");
        process::exit(1);
    }

    // We are now the daemon child.
    daemon::close_std_fds();

    // Respect RUST_LOG if the parent process set it (e.g. via `zylaxion -v
    // daemon`), otherwise default to `info`. The parent's env vars survive
    // `daemonize()`'s fork, so `--verbose` propagates into the daemon child.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    log::info!("daemon started (PID: {})", nix::unistd::getpid().as_raw());

    if let Err(_e) = daemon::write_pid_file() {
        process::exit(1);
    }

    daemon::ignore_hup_pipe();

    let stop_flag = Arc::new(AtomicBool::new(false));
    daemon::install_signal_handlers(Arc::clone(&stop_flag));

    let listener = match daemon::ipc::create_listener() {
        Ok(fd) => fd,
        Err(e) => {
            log::error!("IPC setup failed: {e}");
            daemon::cleanup();
            process::exit(1);
        }
    };
    log::info!("IPC socket ready: {}", daemon::ipc::socket_path().display());

    let profile = resolve_profile(&profile_name);
    log::info!(
        "using profile: {}",
        profile_name.as_deref().unwrap_or("default")
    );

    // Initialise audio (mirror zylaxion_live exactly).
    let mut input_source = LibinputSource::new();
    let event_rx = match input_source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            log::error!("input error: {e}");
            daemon::cleanup();
            process::exit(1);
        }
    };

    let mut orchestrator = match Orchestrator::new() {
        Ok(o) => o,
        Err(e) => {
            log::error!("audio error: {e}");
            daemon::cleanup();
            process::exit(1);
        }
    };

    let _ipc_handle = daemon::spawn_ipc_thread(listener, Arc::clone(&stop_flag));

    let model = MechanicalClick::with_profile(profile);
    orchestrator.run(&model, &event_rx, stop_flag);

    log::info!("shutting down");
    daemon::cleanup();
}

/// Send a "stop" command to a running daemon via IPC.
pub fn cmd_stop() {
    daemon::client_send_and_print("stop");
}
