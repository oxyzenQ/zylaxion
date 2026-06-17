// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Foreground and daemon subcommands: `start`, `daemon`, `stop`, `reload`.

use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use arc_swap::ArcSwap;
use zactrix_profiles::MechanicalClick;
use zylaxion_core::Orchestrator;
use zylaxion_input::{InputSource, LibinputSource};

use crate::daemon;
use crate::instance_lock;
use crate::profile::resolve_profile;

/// Run zylaxion in the foreground (Ctrl+C to quit).
///
/// Mirrors the `zylaxion_live` example: LibinputSource on stack,
/// Orchestrator::run on main thread.
///
/// Note: hot-reload via IPC is only available in `daemon` mode (the
/// foreground `start` mode has no IPC listener). To reload profiles in
/// foreground mode, Ctrl+C and restart with the new profile.
pub fn cmd_start(profile_name: Option<String>) {
    env_logger::init();

    // Acquire the single-instance lock BEFORE touching audio or input.
    // If another zylaxion process (start OR daemon) is already running,
    // this prints "error: Zylaxion is already running..." and exits 1.
    // The `_lock` guard is held for the entire process lifetime; when
    // cmd_start returns (or the process is killed), the kernel releases
    // the flock atomically.
    let _lock = instance_lock::acquire_or_exit();

    let profiles = resolve_profile(&profile_name);
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
    let model = Arc::new(ArcSwap::from_pointee(MechanicalClick::with_overrides(
        profiles,
    )));
    log::info!("ready — press any key to hear it (Ctrl+C to quit)");

    orchestrator.run(&model, &event_rx, stop_flag);

    log::info!("shutdown complete");
}

/// Run zylaxion as a POSIX background daemon.
///
/// Forks FIRST (no audio/input before fork), then the child initialises
/// hardware, writes the PID file, installs signal handlers, and runs the
/// orchestrator loop. The IPC thread listens for "stop" and "reload"
/// commands; "reload" swaps the acoustic model behind the `ArcSwap`
/// without restarting the daemon.
pub fn cmd_daemon(profile_name: Option<String>) {
    // Acquire the single-instance lock BEFORE forking. The lock is
    // associated with the open file description (kernel struct file),
    // not the process — so fork() inherits it into the child via the
    // duplicated file descriptor. When the parent exits via
    // `daemonize()`, the kernel keeps the lock alive because the child
    // still holds a reference to the same file description.
    //
    // This is the correct order: acquire-then-fork guarantees that no
    // other zylaxion process can slip in between the fork and the child
    // calling `acquire()`.
    let _lock = instance_lock::acquire_or_exit();

    // Stash the profile name in an Arc so the IPC thread can re-resolve
    // it from disk on each "reload" command (without taking ownership
    // away from the child).
    let profile_name_arc = Arc::new(profile_name);

    // Check if already running (with /proc/<pid>/comm PID recycling check).
    // This is a soft check for nicer error messages — the flock above is
    // the authoritative single-instance guard.
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

    let profiles = resolve_profile(&profile_name_arc);
    log::info!(
        "using profile: {}",
        profile_name_arc.as_deref().unwrap_or("default")
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

    // Wrap the acoustic model in an ArcSwap so the IPC thread can swap
    // it at runtime (hot-reload). The render loop loads a snapshot per
    // event batch — lock-free, no Mutex.
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_overrides(profiles),
    ));

    let _ipc_handle = daemon::spawn_ipc_thread(
        listener,
        Arc::clone(&stop_flag),
        Arc::clone(&model),
        Arc::clone(&profile_name_arc),
    );

    orchestrator.run(&model, &event_rx, stop_flag);

    log::info!("shutting down");
    daemon::cleanup();
}

/// Send a "stop" command to a running daemon via IPC.
pub fn cmd_stop() {
    daemon::client_send_and_print("stop");
}

/// Send a "reload" command to a running daemon via IPC.
///
/// The daemon will re-read the current profile TOML from disk (using
/// the same `--profile <name>` it was started with), construct a new
/// `MechanicalClick`, and swap it in behind the `ArcSwap`. Active
/// voices finish naturally with their old profile; new keypresses pick
/// up the new model immediately.
pub fn cmd_reload() {
    println!("Reloading profiles...");
    daemon::client_send_and_print("reload");
}
