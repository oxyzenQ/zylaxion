// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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

    let (profiles, config_path, active_preset, master_params) =
        match config::resolve_config(cli_preset.as_deref()) {
            Ok(tuple) => tuple,
            Err(e) => {
                crate::error_format::error(e);
                process::exit(1);
            }
        };
    log::info!("starting zylaxion in foreground mode (preset: {active_preset})");
    log::info!("master volume: {}×", master_params.volume);

    // 1. Start input capture (background thread).
    let mut input_source = LibinputSource::new();
    let event_rx = match input_source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            crate::error_format::error(format!("input error: {e}"));
            process::exit(1);
        }
    };

    // 2. Create the orchestrator (audio + engine). v10.2.0 (P1): pass
    //    the configured master volume instead of using the hardcoded
    //    default.
    let mut orchestrator = match Orchestrator::with_master_volume(master_params.volume) {
        Ok(o) => o,
        Err(e) => {
            crate::error_format::error(format!("audio error: {e}"));
            crate::error_format::warning("make sure PipeWire or PulseAudio is running");
            process::exit(1);
        }
    };

    // Get the device's actual sample rate for DSP coefficient calculation.
    let sample_rate = orchestrator.sample_rate();
    log::info!("audio device sample rate: {sample_rate} Hz");

    // 3. Wrap the model in Arc<ArcSwap<>> for hot-reload.
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_overrides(profiles, sample_rate),
    ));

    // 5. Run the main loop.
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Install signal handlers so Ctrl+C / SIGTERM / SIGQUIT set the
    // stop_flag, letting the orchestrator exit gracefully (fade-out,
    // CpalSink drop, Flock drop). Without this, `kill -9` would skip
    // Drop guards and leave the terminal / PipeWire in a broken state.
    if let Err(e) = crate::signals::install_graceful_shutdown_handlers(Arc::clone(&stop_flag)) {
        log::warn!("failed to install signal handlers: {e}");
    }

    // 4. Spawn the config-watcher thread. The watcher always reads
    //    preset.tuning from the file on change — the --preset CLI flag
    //    is for initial load only. We pass active_preset here just for
    //    the startup log message. sample_rate is shared so the watcher
    //    can construct new MechanicalClick instances on reload.
    //
    //    v10.2.0 (S1): the watcher now takes a clone of stop_flag so
    //    it can exit cleanly when the orchestrator is shutting down.
    //    Must be spawned AFTER stop_flag is created.
    let _watcher_handle = spawn_config_watcher(
        Arc::clone(&model),
        config_path,
        active_preset.clone(),
        sample_rate,
        Arc::clone(&stop_flag),
    );

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
///
/// # `foreground` mode (for process supervisors like systemd)
///
/// When `foreground` is `true`, the function skips `daemonize()` and
/// `close_std_fds()` and runs the full daemon logic inline on the
/// calling thread. This is the correct mode for systemd's `Type=simple`
/// supervision: the launched process must stay alive in the foreground
/// so systemd can track its PID. The `zylaxion.service` unit uses this
/// via `ExecStart=/usr/bin/zylaxion daemon --foreground`.
///
/// Without `--foreground`, the function forks to the background (the
/// parent prints the child PID and exits 0). This is the right
/// behaviour for manual CLI usage but causes systemd to think the
/// service died immediately (parent exit → service inactive).
pub fn cmd_daemon(cli_preset: Option<String>, foreground: bool) {
    let _lock = instance_lock::acquire_or_exit();

    // Stash the CLI preset in an Arc so the config-watcher thread can
    // share it. If None, the watcher re-reads preset.tuning on each
    // file change.
    let cli_preset_arc = Arc::new(cli_preset);

    if daemon::is_daemon_running().is_ok() {
        crate::error_format::error("zylaxion daemon is already running");
        process::exit(1);
    }

    // v10.2.0 (dragonzen audit B9): in background mode, daemonize()
    // now returns a `DaemonChildSync` handle. We hold it through all
    // init steps and call `signal_success`/`signal_failure` at the
    // appropriate point so the parent doesn't report success before
    // the child has actually initialized. In foreground mode there's
    // no parent to sync with — `child_sync` stays `None`.
    let mut child_sync: Option<daemon::DaemonChildSync> = if foreground {
        // ── Foreground mode: skip fork, keep std streams for journald.
        // systemd wires stdout/stderr to journald automatically when
        // the unit has `StandardOutput=journal` (the default). Logging
        // via `eprintln!` / `log::info!` therefore lands in
        // `journalctl --user -u zylaxion` without any extra config.
        eprintln!(
            "[zylaxion] starting in foreground mode for process supervisor (PID: {})",
            nix::unistd::getpid().as_raw()
        );
        None
    } else {
        // ── Background mode: classic POSIX double-fork daemonization.
        // daemonize() blocks the parent (which exits with the child's
        // status). On the child side it returns Ok(DaemonChildSync).
        match daemon::daemonize() {
            Ok(sync) => {
                // We are now the daemon child.
                daemon::close_std_fds();
                Some(sync)
            }
            Err(e) => {
                // daemonize() failed (fork or setsid). The error was
                // already written to the pipe if we got far enough;
                // the parent will print it. We still need to exit.
                // If daemonize() failed before fork, there's no parent
                // to sync with — print to stderr directly.
                crate::error_format::error(format!("daemonize failed: {e}"));
                process::exit(1);
            }
        }
    };

    // Helper macro: in background mode, signal failure to the parent
    // before exiting. In foreground mode, just exit (error already
    // logged via `log::error!` or `error_format::error`).
    macro_rules! fail_init {
        ($sync:expr, $msg:expr) => {{
            let msg = $msg;
            if let Some(sync) = $sync.take() {
                sync.signal_failure(&msg);
            }
            daemon::cleanup();
            process::exit(1);
        }};
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    let (profiles, config_path, active_preset, master_params) =
        match config::resolve_config(cli_preset_arc.as_deref()) {
            Ok(tuple) => tuple,
            Err(e) => {
                fail_init!(child_sync, e);
            }
        };

    log::info!(
        "daemon started (PID: {}, preset: {active_preset}, foreground: {foreground})",
        nix::unistd::getpid().as_raw()
    );
    log::info!("master volume: {}×", master_params.volume);

    if let Err(e) = daemon::write_pid_file() {
        fail_init!(child_sync, format!("write_pid_file failed: {e}"));
    }

    // In foreground mode we keep the controlling terminal's HUP/PIPE
    // defaults (systemd does not send HUP unless the user session ends,
    // and our signal hook handles SIGTERM cleanly). In background mode
    // we explicitly ignore HUP/PIPE so the daemon survives logout.
    if !foreground {
        daemon::ignore_hup_pipe();
    }

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
            fail_init!(child_sync, format!("IPC setup failed: {e}"));
        }
    };
    log::info!("IPC socket ready: {}", daemon::ipc::socket_path().display());

    let mut input_source = LibinputSource::new();
    let event_rx = match input_source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            fail_init!(child_sync, format!("input error: {e}"));
        }
    };

    // v10.2.0 (P1): pass the configured master volume instead of using
    // the hardcoded default.
    let mut orchestrator = match Orchestrator::with_master_volume(master_params.volume) {
        Ok(o) => o,
        Err(e) => {
            fail_init!(child_sync, format!("audio error: {e}"));
        }
    };

    let sample_rate = orchestrator.sample_rate();
    log::info!("audio device sample rate: {sample_rate} Hz");

    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_overrides(profiles, sample_rate),
    ));

    // v10.2.0 (dragonzen audit B6): spawn_ipc_thread now returns Result.
    // A failure here is non-fatal — the daemon can still run without
    // IPC (the user just can't `zylaxion stop` via socket; they must
    // use SIGTERM/SIGINT instead). Log the error and continue.
    let _ipc_handle = match daemon::spawn_ipc_thread(listener, Arc::clone(&stop_flag)) {
        Ok(handle) => Some(handle),
        Err(e) => {
            log::warn!("IPC thread spawn failed — daemon will run without IPC. Error: {e}");
            None
        }
    };
    // v10.2.0 (dragonzen audit S1): pass stop_flag to the config-watcher
    // so it can exit cleanly when the orchestrator is shutting down.
    // Previously the watcher thread leaked at shutdown — it kept polling
    // every 1 s until process exit, holding a clone of the ArcSwap and
    // preventing clean Drop of the model.
    let _watcher_handle = spawn_config_watcher(
        Arc::clone(&model),
        config_path,
        active_preset.clone(),
        sample_rate,
        Arc::clone(&stop_flag),
    );

    // v10.2.0 (dragonzen audit B9): all init is complete. Signal
    // success to the parent (if any) so the parent can print the PID
    // and exit 0. In foreground mode this is a no-op (child_sync is
    // None). After this point, the daemon is fully running — any
    // further errors are runtime errors, not init errors, and are
    // handled by the orchestrator's graceful shutdown path.
    if let Some(sync) = child_sync.take() {
        sync.signal_success();
    }

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
/// - Polls every [`CONFIG_WATCH_INTERVAL`] (1 second).
/// - On every poll, re-runs the search-path resolution via
///   [`crate::config::find_config_path`]. This catches the case where
///   a higher-priority config file appears while the daemon is
///   running (e.g. user creates `~/.config/zylaxion/config.toml`
///   while the daemon was watching `/usr/local/share/zylaxion/...`).
///   When the resolved path changes, the watcher logs the switch and
///   loads the new file immediately.
/// - For the currently watched path, polls its modification time
///   (mtime) via `std::fs::metadata`. When the mtime advances, the
///   thread re-reads the file via [`config::reload_preset`].
///   **The `--preset` CLI flag is ignored on reload** — the watcher
///   always reads `preset.tuning` from the freshly-saved file.
/// - On any error (parse failure, preset not found), the thread logs
///   a `warn!` and keeps the old model. The user can fix the TOML and
///   save again to retry.
/// - If `config_path` is `None` at startup (hardcoded default, no
///   file found anywhere), the thread does NOT exit — it keeps
///   polling so it picks up a config file the moment one appears in
///   any search path.
///
/// # `initial_preset` is for logging only
///
/// The `initial_preset` parameter is used solely for the startup log
/// message ("watching ... initial preset: X"). It is NOT passed to
/// `reload_preset` — the watcher always defers to `preset.tuning` on
/// file change.
///
/// # No blocking of the audio callback
///
/// `ArcSwap::store()` is a single atomic pointer swap. The cpal audio
/// callback never touches the ArcSwap — it only reads the cached
/// `KeyProfile` snapshot that each voice captured at trigger time.
///
/// # Graceful shutdown (v10.2.0+ — dragonzen audit S1)
///
/// The watcher accepts a clone of the orchestrator's `stop_flag`. It
/// checks the flag at the top of every poll iteration and exits
/// cleanly when the flag is set. Previously the watcher had no
/// shutdown signal — it leaked at process exit, holding a clone of
/// the `Arc<ArcSwap<MechanicalClick>>` and preventing clean Drop of
/// the model.
fn spawn_config_watcher(
    model: Arc<ArcSwap<MechanicalClick>>,
    config_path: Option<std::path::PathBuf>,
    initial_preset: String,
    sample_rate: u32,
    stop_flag: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("zylaxion-config-watcher".into())
        .spawn(move || {
            // `current_path` is the file the watcher is currently
            // watching. It can change mid-run if a higher-priority
            // file appears in the search path (see `find_config_path`
            // for the resolution order).
            let mut current_path: Option<std::path::PathBuf> = config_path.clone();

            // `last_mtime` is the mtime of `current_path` as observed
            // on the previous poll. Used to detect in-place edits to
            // the currently watched file. Reset to `None` whenever
            // `current_path` changes so the new file is loaded on the
            // next iteration regardless of its mtime.
            let mut last_mtime: Option<SystemTime> = current_mtime_of(&current_path);

            log::info!(
                "config-watcher: initial path = {} (initial preset: {}, poll interval: {:?})",
                path_display(&current_path),
                initial_preset,
                CONFIG_WATCH_INTERVAL
            );

            loop {
                // v10.2.0 (S1): check stop_flag at the top of every
                // iteration so the watcher exits cleanly when the
                // orchestrator is shutting down.
                if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    log::info!("config-watcher exiting (stop flag set)");
                    return;
                }

                std::thread::sleep(CONFIG_WATCH_INTERVAL);

                // ── 1. Re-evaluate the search path ────────────────────
                // This is the v4.1.0 addition: detect when a higher-
                // priority config file appears while the daemon is
                // already running. Without this, the watcher would be
                // locked to whatever path was resolved at startup.
                let resolved = crate::config::find_config_path();

                if resolved != current_path {
                    log::info!(
                        "config-watcher: config path changed: switching from {} to {}",
                        path_display(&current_path),
                        path_display(&resolved)
                    );
                    current_path = resolved;
                    // Force a reload on the next step by clearing the
                    // mtime baseline. The new file might have any
                    // mtime, so we cannot rely on the previous value.
                    last_mtime = None;

                    // If the new path is None (all config files
                    // vanished), keep the old model and continue
                    // polling — the user might be in the middle of
                    // moving files around.
                    if current_path.is_none() {
                        log::warn!(
                            "config-watcher: no config.toml found in any search path — \
                             keeping previous model, will retry next poll"
                        );
                        continue;
                    }
                }

                // ── 2. Check the currently-watched file's mtime ──────
                let Some(path) = current_path.as_ref() else {
                    // No file to watch yet (initial hardcoded-default
                    // case). The search-path re-evaluation above will
                    // pick up a file the moment one appears.
                    continue;
                };

                let now_mtime = match current_mtime(path) {
                    Some(t) => t,
                    None => {
                        // The file disappeared between the search-path
                        // check and now (race). The next poll's
                        // search-path re-evaluation will handle the
                        // fallback. Just reset the baseline so we don't
                        // miss a recreation.
                        last_mtime = None;
                        continue;
                    }
                };

                if let Some(prev) = last_mtime {
                    if now_mtime <= prev {
                        // mtime hasn't advanced — nothing to do.
                        continue;
                    }
                }

                log::info!("config-watcher: {} changed, reloading", path.display());
                last_mtime = Some(now_mtime);

                // reload_preset reads preset.tuning from the file — the
                // initial --preset CLI flag is intentionally NOT passed
                // here so that file edits always take precedence.
                match crate::config::reload_preset(path, None) {
                    Ok((profiles, active, master)) => {
                        let new_model =
                            MechanicalClick::with_overrides(profiles, sample_rate);
                        model.store(Arc::new(new_model));
                        log::info!("config-watcher: reloaded preset '{active}' successfully");
                        // v10.2.0 (P1): master volume is part of the
                        // config but lives in the VoicePool, not the
                        // AcousticModel. Hot-reload of master volume is
                        // not yet wired — the user must restart the
                        // daemon for [master].volume changes to take
                        // effect. Log the loaded value for visibility.
                        log::debug!(
                            "config-watcher: master volume from reload = {}× (hot-reload not yet supported — restart to apply)",
                            master.volume
                        );
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

/// Get the modification time of a path (wrapped in `Option`) as a
/// `SystemTime`. Returns `None` if the path is `None` (no file
/// configured) or if the metadata cannot be read.
fn current_mtime_of(path: &Option<std::path::PathBuf>) -> Option<SystemTime> {
    path.as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
}

/// Format an `Option<PathBuf>` for display in log messages.
/// `None` becomes `"<hardcoded default>"` so the log line is
/// unambiguous: it means no config file was found anywhere and the
/// daemon is running with the built-in `KeyProfile::default()`.
fn path_display(path: &Option<std::path::PathBuf>) -> String {
    match path {
        Some(p) => p.display().to_string(),
        None => "<hardcoded default>".to_string(),
    }
}

/// Get the modification time of a path as a `SystemTime`, or `None` if
/// the file does not exist or its mtime cannot be read.
fn current_mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}
