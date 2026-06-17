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
/// Loads the active preset (from `--preset` CLI flag or `preset.tuning`
/// in config.toml), spawns the config-watcher thread (auto-reload on
/// file change), and runs the orchestrator loop until Ctrl+C.
///
/// If the resolved preset does not exist in config.toml, prints a clear
/// error and exits — there is NO silent fallback.
pub fn cmd_start(cli_preset: Option<String>) {
    env_logger::init();

    let _lock = instance_lock::acquire_or_exit();

    let (profiles, config_path, active_preset) = match config::resolve_config(cli_preset.as_deref())
    {
        Ok(tuple) => tuple,
        Err(e) => {
            crate::error_format::error(e);
            process::exit(1);
        }
    };
    log::info!("starting zylaxion in foreground mode (preset: {active_preset})");

    // 1. Start input capture (background thread).
    let mut input_source = LibinputSource::new();
    let event_rx = match input_source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            crate::error_format::error(format!("input error: {e}"));
            process::exit(1);
        }
    };

    // 2. Create the orchestrator (audio + engine).
    let mut orchestrator = match Orchestrator::new() {
        Ok(o) => o,
        Err(e) => {
            crate::error_format::error(format!("audio error: {e}"));
            crate::error_format::warning("make sure PipeWire or PulseAudio is running");
            process::exit(1);
        }
    };

    // 3. Wrap the model in Arc<ArcSwap<>> for hot-reload.
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_overrides(profiles),
    ));

    // 4. Spawn the config-watcher thread. If cli_preset is None, the
    //    watcher re-reads preset.tuning on each file change — so
    //    changing the tuning value and saving causes an immediate
    //    swap to the new preset.
    let _watcher_handle =
        spawn_config_watcher(Arc::clone(&model), config_path, Arc::new(cli_preset));

    // 5. Run the main loop.
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Install signal handlers so Ctrl+C / SIGTERM / SIGQUIT set the
    // stop_flag, letting the orchestrator exit gracefully (fade-out,
    // CpalSink drop, Flock drop). Without this, `kill -9` would skip
    // Drop guards and leave the terminal / PipeWire in a broken state.
    if let Err(e) = crate::signals::install_graceful_shutdown_handlers(Arc::clone(&stop_flag)) {
        log::warn!("failed to install signal handlers: {e}");
    }

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
///
/// If the resolved preset does not exist in config.toml, prints a clear
/// error and exits — there is NO silent fallback.
pub fn cmd_daemon(cli_preset: Option<String>) {
    let _lock = instance_lock::acquire_or_exit();

    // Stash the CLI preset in an Arc so the config-watcher thread can
    // share it. If None, the watcher re-reads preset.tuning on each
    // file change.
    let cli_preset_arc = Arc::new(cli_preset);

    if daemon::is_daemon_running().is_ok() {
        crate::error_format::error("zylaxion daemon is already running");
        process::exit(1);
    }

    if let Err(e) = daemon::daemonize() {
        crate::error_format::error(format!("daemonize failed: {e}"));
        process::exit(1);
    }

    // We are now the daemon child.
    daemon::close_std_fds();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    let (profiles, config_path, active_preset) =
        match config::resolve_config(cli_preset_arc.as_deref()) {
            Ok(tuple) => tuple,
            Err(e) => {
                crate::error_format::error(e);
                process::exit(1);
            }
        };

    log::info!(
        "daemon started (PID: {}, preset: {active_preset})",
        nix::unistd::getpid().as_raw()
    );

    if let Err(_e) = daemon::write_pid_file() {
        process::exit(1);
    }

    daemon::ignore_hup_pipe();

    let stop_flag = Arc::new(AtomicBool::new(false));

    // Install signal handlers so SIGTERM / SIGINT / SIGQUIT set the
    // stop_flag, letting the orchestrator exit gracefully (fade-out,
    // CpalSink drop, Flock drop, PID file cleanup). Without this,
    // `pkill -f zylaxion` would skip Drop guards and leave stale lock
    // files + a broken PipeWire graph.
    if let Err(e) = crate::signals::install_graceful_shutdown_handlers(Arc::clone(&stop_flag)) {
        log::warn!("failed to install signal handlers: {e}");
    }

    let listener = match daemon::ipc::create_listener() {
        Ok(fd) => fd,
        Err(e) => {
            log::error!("IPC setup failed: {e}");
            daemon::cleanup();
            process::exit(1);
        }
    };
    log::info!("IPC socket ready: {}", daemon::ipc::socket_path().display());

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

    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_overrides(profiles),
    ));

    let _ipc_handle = daemon::spawn_ipc_thread(listener, Arc::clone(&stop_flag));
    let _watcher_handle =
        spawn_config_watcher(Arc::clone(&model), config_path, Arc::clone(&cli_preset_arc));

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
/// - When the mtime advances, the thread re-reads the file via
///   [`config::reload_preset`], which determines the active preset:
///   - If `cli_preset` is `Some(name)`, that preset is always loaded
///     (CLI override).
///   - If `cli_preset` is `None`, the `preset.tuning` value from the
///     freshly-read file is used — so changing `tuning = "cherryMX"`
///     and saving causes an immediate swap to cherryMX.
/// - On any error (parse failure, preset not found), the thread logs
///   a `warn!` and keeps the old model. The user can fix the TOML and
///   save again to retry.
/// - If `config_path` is `None` (hardcoded default fallback, no file to
///   watch), the thread exits immediately.
///
/// # No blocking of the audio callback
///
/// `ArcSwap::store()` is a single atomic pointer swap. The cpal audio
/// callback never touches the ArcSwap — it only reads the cached
/// `KeyProfile` snapshot that each voice captured at trigger time.
fn spawn_config_watcher(
    model: Arc<ArcSwap<MechanicalClick>>,
    config_path: Option<std::path::PathBuf>,
    cli_preset: Arc<Option<String>>,
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
                "config-watcher: watching {} for changes (cli_preset: {}, poll interval: {:?})",
                path.display(),
                cli_preset.as_deref().unwrap_or("<from tuning>"),
                CONFIG_WATCH_INTERVAL
            );

            let mut last_mtime: Option<SystemTime> = current_mtime(&path);

            loop {
                std::thread::sleep(CONFIG_WATCH_INTERVAL);

                let now_mtime = match current_mtime(&path) {
                    Some(t) => t,
                    None => continue,
                };

                if let Some(prev) = last_mtime {
                    if now_mtime <= prev {
                        continue;
                    }
                }

                log::info!("config-watcher: {} changed, reloading", path.display());
                last_mtime = Some(now_mtime);

                match crate::config::reload_preset(&path, cli_preset.as_deref()) {
                    Ok((profiles, active)) => {
                        let new_model = MechanicalClick::with_overrides(profiles);
                        model.store(Arc::new(new_model));
                        log::info!("config-watcher: reloaded preset '{active}' successfully");
                    }
                    Err(e) => {
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
