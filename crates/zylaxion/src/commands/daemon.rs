// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Foreground and daemon subcommands: `start`, `daemon`, `stop`.
//!
//! Both `cmd_start` and `cmd_daemon` follow the same lifecycle:
//!
//!   1. Acquire the single-instance flock (prevents two zylaxion
//!      processes from running concurrently and clashing on PipeWire).
//!   2. Resolve the central `config.toml` from the search path.
//!   3. Construct a `MechanicalClick` and wrap it in `Arc<ArcSwap<>>`
//!      for hot-reload support.
//!   4. Start the audio + input hardware.
//!   5. Spawn the config-watcher thread (polls `config.toml` mtime
//!      every 1 s; on change, re-reads, validates, and atomically
//!      swaps the model).
//!   6. (Daemon mode only) Spawn the IPC thread for `stop`/`status`.
//!   7. Run the orchestrator loop until stop_flag is set or the input
//!      channel disconnects.

use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use arc_swap::ArcSwap;
use zactrix_profiles::MechanicalClick;
use zylaxion_core::Orchestrator;
use zylaxion_input::{InputSource, LibinputSource};

use crate::config;
use crate::daemon;
use crate::instance_lock;

/// Polling interval for the config-file watcher.
///
/// 1 second is the sweet spot: short enough that users perceive config
/// edits as "instant" after save, long enough that the watcher thread
/// consumes negligible CPU (one `stat()` per second).
const CONFIG_WATCH_INTERVAL: Duration = Duration::from_secs(1);

/// Run zylaxion in the foreground (Ctrl+C to quit).
///
/// Loads `config.toml` from the search path, spawns the config-watcher
/// thread (auto-reload on file change), and runs the orchestrator loop
/// on the main thread until Ctrl+C disconnects the input channel.
pub fn cmd_start() {
    env_logger::init();

    // Acquire the single-instance lock BEFORE touching audio or input.
    // If another zylaxion process (start OR daemon) is already running,
    // this prints "error: Zylaxion is already running..." and exits 1.
    // The `_lock` guard is held for the entire function scope; when
    // cmd_start returns (or the process is killed), the kernel releases
    // the flock atomically.
    let _lock = instance_lock::acquire_or_exit();

    let (profiles, config_path) = config::resolve_config();
    log::info!("starting zylaxion in foreground mode");

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

    // 3. Wrap the model in Arc<ArcSwap<>> for hot-reload. The
    //    config-watcher thread (spawned below) will atomically swap
    //    this when config.toml changes on disk.
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_overrides(profiles),
    ));

    // 4. Spawn the config-watcher thread. It polls config.toml's mtime
    //    every CONFIG_WATCH_INTERVAL; on change, it re-reads, validates,
    //    and atomically swaps the model. On parse error, it logs a
    //    warn! and keeps the old model — the user can fix the TOML and
    //    save again to retry.
    let _watcher_handle = spawn_config_watcher(Arc::clone(&model), config_path);

    // 5. Run the main loop on the MAIN thread (blocks until Ctrl+C).
    let stop_flag = Arc::new(AtomicBool::new(false));
    log::info!("ready — press any key to hear it (Ctrl+C to quit)");

    orchestrator.run(&model, &event_rx, stop_flag);

    log::info!("shutdown complete");
}

/// Run zylaxion as a POSIX background daemon.
///
/// Forks FIRST (no audio/input before fork), then the child initialises
/// hardware, writes the PID file, installs signal handlers, spawns the
/// IPC thread (for `stop`/`status`) and the config-watcher thread (for
/// auto-reload), and runs the orchestrator loop.
pub fn cmd_daemon() {
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

    let (profiles, config_path) = config::resolve_config();
    log::info!("config loaded");

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

    // Wrap the acoustic model in an ArcSwap so the config-watcher
    // thread can swap it at runtime (auto-reload). The render loop
    // loads a snapshot per event batch — lock-free, no Mutex.
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_overrides(profiles),
    ));

    let _ipc_handle = daemon::spawn_ipc_thread(listener, Arc::clone(&stop_flag));
    let _watcher_handle = spawn_config_watcher(Arc::clone(&model), config_path);

    orchestrator.run(&model, &event_rx, stop_flag);

    log::info!("shutting down");
    daemon::cleanup();
}

/// Send a "stop" command to a running daemon via IPC.
pub fn cmd_stop() {
    daemon::client_send_and_print("stop");
}

/// Spawn a background thread that watches `config.toml` for changes
/// and atomically swaps the acoustic model when the file is modified.
///
/// # Behaviour
///
/// - Polls `config_path`'s modification time (mtime) every
///   [`CONFIG_WATCH_INTERVAL`] (1 second) via `std::fs::metadata`.
/// - When the mtime advances, the thread re-reads the file, parses it
///   via [`config::resolve_config`]-equivalent logic (validate + clamp),
///   constructs a new `MechanicalClick`, and calls
///   `model.store(Arc::new(new_model))` — an atomic swap.
/// - If the re-read or parse fails, the thread logs a `warn!` and keeps
///   the old model. The user can fix the TOML and save again to retry.
/// - If `config_path` is `None` (hardcoded default fallback, no file to
///   watch), the thread exits immediately after one log line.
///
/// # Why a separate thread (not in the orchestrator loop)
///
/// The orchestrator loop is audio-critical. Putting fs::metadata polls
/// there would add latency spikes. A dedicated watcher thread keeps
/// the audio path clean while still picking up config changes within
/// 1 second.
///
/// # No blocking of the audio callback
///
/// `ArcSwap::store()` is a single atomic pointer swap. The cpal audio
/// callback never touches the ArcSwap — it only reads the cached
/// `KeyProfile` snapshot that each voice captured at trigger time. So
/// even mid-callback swaps are safe.
fn spawn_config_watcher(
    model: Arc<ArcSwap<MechanicalClick>>,
    config_path: Option<std::path::PathBuf>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("zylaxion-config-watcher".into())
        .spawn(move || {
            let Some(path) = config_path else {
                log::info!(
                    "config-watcher: no config.toml on disk — using hardcoded default, nothing to watch"
                );
                return;
            };

            log::info!(
                "config-watcher: watching {} for changes (poll interval: {:?})",
                path.display(),
                CONFIG_WATCH_INTERVAL
            );

            // Snapshot the initial mtime so we don't immediately re-read
            // the file we just loaded. None means the file wasn't
            // statable at startup (shouldn't happen since resolve_config
            // just loaded it, but be defensive).
            let mut last_mtime: Option<SystemTime> = current_mtime(&path);

            loop {
                std::thread::sleep(CONFIG_WATCH_INTERVAL);

                let now_mtime = match current_mtime(&path) {
                    Some(t) => t,
                    None => {
                        // File disappeared (e.g. user deleted it). Skip
                        // this cycle; if the user recreates it, the next
                        // poll will pick up the new mtime.
                        continue;
                    }
                };

                // Skip if mtime hasn't advanced. The `last_mtime.is_none()`
                // branch handles the rare case where the initial stat
                // failed but later succeeded — treat that as a change.
                if let Some(prev) = last_mtime {
                    if now_mtime <= prev {
                        continue;
                    }
                }

                // mtime advanced — re-read and swap.
                log::info!("config-watcher: {} changed, reloading", path.display());
                last_mtime = Some(now_mtime);

                match zactrix_profiles::ProfileWithOverrides::from_file(&path) {
                    Ok(profiles) => {
                        let new_model = MechanicalClick::with_overrides(profiles);
                        model.store(Arc::new(new_model));
                        log::info!("config-watcher: reloaded successfully");
                    }
                    Err(e) => {
                        // Keep the old model. User can fix the TOML and
                        // save again — the next mtime advance will
                        // trigger another reload attempt.
                        log::warn!(
                            "config-watcher: failed to reload {} — keeping previous config. Error: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        })
        .expect("failed to spawn config-watcher thread")
}

/// Get the modification time of a path as a `SystemTime`, or `None` if
/// the file does not exist or its mtime cannot be read.
fn current_mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}
