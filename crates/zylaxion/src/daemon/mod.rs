// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Daemonization helpers: fork, PID file, signal handling, IPC thread.

pub mod ipc;

use std::fs;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

use nix::sys::signal::{self, SigHandler, Signal};
use nix::unistd::{fork, getpid, setsid, ForkResult, Pid};

/// Global pointer to the stop flag, bridging the Rust `Arc<AtomicBool>`
/// into the C signal handler (which cannot capture Rust variables).
/// Written once during `install_signal_handlers()`, read from the
/// signal handler.  Uses `AtomicPtr` to avoid `unsafe static mut`.
static STOP_FLAG_PTR: AtomicPtr<AtomicBool> = AtomicPtr::new(std::ptr::null_mut());

/// Ignore `SIGHUP` and `SIGPIPE` so the daemon child does not die or
/// hang when the controlling terminal disappears or a socket write fails.
pub fn ignore_hup_pipe() {
    let _ = unsafe { signal::signal(Signal::SIGHUP, SigHandler::SigIgn) };
    let _ = unsafe { signal::signal(Signal::SIGPIPE, SigHandler::SigIgn) };
}

/// Install SIGTERM / SIGINT handlers that atomically set `stop_flag` to
/// `true`.  The `Arc<AtomicBool>` is intentionally leaked via
/// `Arc::into_raw` because signal handlers cannot safely manage Rust
/// ownership — the flag lives for the entire daemon process lifetime.
pub fn install_signal_handlers(stop_flag: Arc<AtomicBool>) {
    // Leak the Arc so the C signal handler can reach the AtomicBool.
    // This is safe: `AtomicBool::store(Relaxed)` is async-signal-safe,
    // and the allocation outlives the process.
    let ptr = Arc::into_raw(stop_flag) as *mut AtomicBool;
    STOP_FLAG_PTR.store(ptr, Ordering::Release);

    extern "C" fn handle_signal(_: std::os::raw::c_int) {
        let ptr = STOP_FLAG_PTR.load(Ordering::Acquire);
        if !ptr.is_null() {
            // SAFETY: `ptr` is a leaked `Arc<AtomicBool>` that lives for
            // the entire process lifetime.  `AtomicBool::store(Relaxed)`
            // is an async-signal-safe atomic operation.
            unsafe {
                (*ptr).store(true, Ordering::Relaxed);
            }
        }
    }

    let handler = SigHandler::Handler(handle_signal);
    let _ = unsafe { signal::signal(Signal::SIGTERM, handler) };
    let _ = unsafe { signal::signal(Signal::SIGINT, handler) };
}

/// Write the current PID to the PID file.
pub fn write_pid_file() -> Result<(), String> {
    let path = ipc::pid_path();
    let pid = getpid().as_raw();
    let mut f = fs::File::create(&path).map_err(|e| format!("failed to create PID file: {e}"))?;
    write!(f, "{pid}").map_err(|e| format!("failed to write PID: {e}"))?;
    Ok(())
}

/// Remove the PID file and socket file.
pub fn cleanup() {
    let _ = fs::remove_file(ipc::pid_path());
    let _ = fs::remove_file(ipc::socket_path());
}

/// Fork into the background.  The child calls `setsid()` to become
/// a session leader, then returns.  The parent exits with 0.
pub fn daemonize() -> Result<(), String> {
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => std::process::exit(0),
        Ok(ForkResult::Child) => {
            if let Err(e) = setsid() {
                return Err(format!("setsid failed: {e}"));
            }
            // Second fork to guarantee we can never re-acquire a terminal.
            match unsafe { fork() } {
                Ok(ForkResult::Parent { .. }) => std::process::exit(0),
                Ok(ForkResult::Child) => Ok(()),
                Err(e) => Err(format!("second fork failed: {e}")),
            }
        }
        Err(e) => Err(format!("fork failed: {e}")),
    }
}

/// Check if a daemon is already running by reading the PID file
/// and checking if the process exists.
pub fn is_daemon_running() -> Result<Pid, String> {
    let path = ipc::pid_path();
    let content = fs::read_to_string(&path).map_err(|_| "daemon is not running".to_string())?;
    let pid: i32 = content
        .trim()
        .parse()
        .map_err(|_| "invalid PID file".to_string())?;
    let pid = Pid::from_raw(pid);

    // Send signal 0 (check existence) via nix.
    if signal::kill(pid, None).is_err() {
        let _ = fs::remove_file(&path);
        return Err("daemon is not running (stale PID file cleaned)".to_string());
    }

    Ok(pid)
}

/// Spawn the IPC listener on a dedicated background thread.
///
/// When a "stop" command is received, `stop_flag` is set to `true` and
/// the thread exits.  The main-thread orchestrator loop checks this
/// flag and breaks cleanly, dropping `CpalSink` and releasing audio.
pub fn spawn_ipc_thread(
    listener: UnixListener,
    stop_flag: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("zylaxion-ipc".into())
        .spawn(move || loop {
            match ipc::handle_one_connection(&listener) {
                Some(cmd) if cmd == "stop" => {
                    log::info!("received 'stop' command via IPC");
                    stop_flag.store(true, Ordering::Relaxed);
                    break;
                }
                Some(_) | None => continue,
            }
        })
        .expect("failed to spawn IPC thread")
}

/// Connect to the daemon, send a command, print the result.
pub fn client_send_and_print(cmd: &str) {
    match ipc::send_command(cmd) {
        Ok(resp) => {
            if resp.ok {
                println!("{}", resp.message);
            } else {
                eprintln!("error: {}", resp.message);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

/// Execute the `status` subcommand with extra detail.
pub fn cmd_status() {
    match ipc::send_command("status") {
        Ok(resp) => {
            if resp.ok {
                match is_daemon_running() {
                    Ok(pid) => {
                        println!(
                            "Running (PID: {}), Socket: {}",
                            pid.as_raw(),
                            ipc::socket_path().display()
                        );
                    }
                    Err(_) => {
                        println!("Running, Socket: {}", ipc::socket_path().display());
                    }
                }
            } else {
                eprintln!("error: {}", resp.message);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
