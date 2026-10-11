// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Foreground and daemon subcommands: `start`, `daemon`, `stop`.
//!
//! Both `cmd_start` and `cmd_daemon` follow the same lifecycle:
//!
//!   1. Acquire the single-instance flock (prevents two zylaxion
//!      processes from running concurrently and clashing on PipeWire).
//!   2. Resolve the active preset from the registry via the session
//!      resolver (CLI `--preset` > `ZYLAXION_PRESET` env > compiled
//!      default; night-improve-22, the no-config law).
//!   3. Construct a `MechanicalClick` and wrap it in `Arc<ArcSwap<>>`
//!      (the swap point survives the config era; preset changes are a
//!      process restart, not a runtime swap).
//!   4. Start the audio + input hardware.
//!   5. (Daemon mode only) Spawn the IPC thread for `stop`/`status`.
//!   6. Run the orchestrator loop until stop_flag is set or the input
//!      channel disconnects.
//!
//! The former config-watcher thread (poll `config.toml` mtime every
//! 1 s, re-read, validate, hot-swap the model) died with the config
//! file in night-improve-22 — a sound switcher does not need a
//! filesystem poller.

use std::process;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::core::Orchestrator;
use crate::input::{InputSource, LibinputSource};
use crate::profiles::MechanicalClick;
use arc_swap::ArcSwap;

use crate::daemon;
use crate::session::instance_lock;
use crate::session::preset;

/// Shared logger wiring (night-improve-13): one builder for both the
/// foreground (`start`) and daemonized paths. The timestamp lane is a
/// hand-rolled UTC stamp ([`crate::info::clock`] — the Hinnant civil_from_days
/// core, std clock only) because env_logger's own timestamp lane (the
/// `humantime` feature) transitively pulls the jiff crate tree — the
/// exact dependency bloat the no-time-crate contract refuses. The
/// rendered shape keeps env_logger's bracketed contract so journal
/// consumers and log greps survive unchanged:
/// `[2026-10-11T07:41:44.284Z INFO  zylaxion::daemon] message`.
///
/// `default_filter`: `None` = plain `Env::default()` (RUST_LOG only —
/// `main.rs` seeds a default before parsing), `Some(f)` =
/// `default_filter_or(f)` (the daemonized path where the parent may have
/// died before seeding). Call exactly once per process, as before.
fn init_logger(default_filter: Option<&str>) {
    use std::io::Write as _;

    let env = match default_filter {
        Some(f) => env_logger::Env::default().default_filter_or(f),
        None => env_logger::Env::default(),
    };
    env_logger::Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {:<5} {}] {}",
                crate::info::clock::iso_utc_millis(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
}

/// Run zylaxion in the foreground (Ctrl+C to quit).
///
/// Resolves the active preset (from `--preset` CLI flag or the
/// `ZYLAXION_PRESET` environment variable against the compiled
/// registry) and runs the orchestrator loop until Ctrl+C.
///
/// If the resolved preset does not exist in the registry, prints a
/// clear error and exits — there is NO silent fallback.
pub fn cmd_start(cli_preset: Option<String>) {
    // Privilege guard first (night-improve-16): refuse euid 0 before
    // ANY state touch — `acquire_or_exit` below creates the lock file,
    // and acquiring it as root IS the pollution event (a root-owned
    // lock inside the user's runtime dir breaks the next user-mode
    // run). Full verdict table: `crate::session::privilege` module docs.
    if let Some(refusal) =
        crate::session::privilege::session_refusal(crate::session::privilege::euid_is_root())
    {
        crate::output::error(refusal);
        process::exit(1);
    }

    init_logger(Some("info"));

    let _lock = instance_lock::acquire_or_exit();

    let (profile, active_preset, master_volume) = match preset::resolve(cli_preset.as_deref()) {
        Ok(resolved) => resolved,
        Err(e) => {
            crate::output::error(e);
            process::exit(1);
        }
    };
    log::info!("starting zylaxion in foreground mode (preset: {active_preset})");
    log::info!("master volume: {master_volume}×");

    // 1. Start input capture (background thread).
    let mut input_source = LibinputSource::new();
    let event_rx = match input_source.listen() {
        Ok(rx) => rx,
        Err(e) => {
            crate::output::error(format!("input error: {e}"));
            process::exit(1);
        }
    };

    // 2. Create the orchestrator (audio + engine). night-improve-22:
    //    pass the resolved master volume (ZYLAXION_VOLUME env or the
    //    compiled default) instead of a config-file value.
    let mut orchestrator = match Orchestrator::with_master_volume(master_volume) {
        Ok(o) => o,
        Err(e) => {
            crate::output::error(format!("audio error: {e}"));
            crate::output::warning("make sure PipeWire or PulseAudio is running");
            process::exit(1);
        }
    };

    // Get the device's actual sample rate for DSP coefficient calculation.
    let sample_rate = orchestrator.sample_rate();
    log::info!("audio device sample rate: {sample_rate} Hz");

    // 3. Wrap the model in Arc<ArcSwap<>> — the swap point that once
    //    served the config-era hot reload. The active preset's profile
    //    comes straight from the compiled registry.
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_profile(profile, sample_rate),
    ));

    // 4. Run the main loop. No watcher thread: night-improve-22
    //    retired the config file, so there is nothing to poll — the
    //    preset is chosen once at process start.
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Install signal handlers so Ctrl+C / SIGTERM / SIGQUIT set the
    // stop_flag, letting the orchestrator exit gracefully (fade-out,
    // CpalSink drop, Flock drop). Without this, `kill -9` would skip
    // Drop guards and leave the terminal / PipeWire in a broken state.
    if let Err(e) =
        crate::session::signals::install_graceful_shutdown_handlers(Arc::clone(&stop_flag))
    {
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
    // Privilege guard first (night-improve-16): same contract as
    // `cmd_start`, and here it must also precede the fork — the parent
    // refuses, so no root-owned child ever materializes. Full verdict
    // table: `crate::session::privilege` module docs.
    if let Some(refusal) =
        crate::session::privilege::session_refusal(crate::session::privilege::euid_is_root())
    {
        crate::output::error(refusal);
        process::exit(1);
    }

    let _lock = instance_lock::acquire_or_exit();

    if daemon::is_daemon_running().is_ok() {
        crate::output::error("zylaxion daemon is already running");
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
                crate::output::error(format!("daemonize failed: {e}"));
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

    init_logger(Some("info"));

    let (profile, active_preset, master_volume) = match preset::resolve(cli_preset.as_deref()) {
        Ok(resolved) => resolved,
        Err(e) => {
            fail_init!(child_sync, e);
        }
    };

    log::info!(
        "daemon started (PID: {}, preset: {active_preset}, foreground: {foreground})",
        nix::unistd::getpid().as_raw()
    );
    log::info!("master volume: {master_volume}×");

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
    if let Err(e) =
        crate::session::signals::install_graceful_shutdown_handlers(Arc::clone(&stop_flag))
    {
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

    // v10.2.0 (P1): pass the resolved master volume instead of using
    // the hardcoded default.
    let mut orchestrator = match Orchestrator::with_master_volume(master_volume) {
        Ok(o) => o,
        Err(e) => {
            fail_init!(child_sync, format!("audio error: {e}"));
        }
    };

    let sample_rate = orchestrator.sample_rate();
    log::info!("audio device sample rate: {sample_rate} Hz");

    // night-improve-22: the active preset's profile comes straight
    // from the compiled registry.
    let model: Arc<ArcSwap<MechanicalClick>> = Arc::new(ArcSwap::from_pointee(
        MechanicalClick::with_profile(profile, sample_rate),
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
    // v10.2.0 (dragonzen audit S1): the config-watcher stop_flag clone
    // died with the watcher in night-improve-22 — the stop_flag now
    // serves the signal handlers and the IPC thread only.

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
