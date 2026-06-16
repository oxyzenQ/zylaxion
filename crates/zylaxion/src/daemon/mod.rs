// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Daemonization helpers: fork, PID file, signal handling, IPC thread.

pub mod ipc;

use std::fs;
use std::io::Write;
use std::os::unix::net::UnixListener;

use nix::sys::signal::{self, SigHandler, Signal};
use nix::unistd::{fork, getpid, setsid, ForkResult, Pid};

/// A flag set by the SIGTERM/SIGINT handler to tell the IPC
/// listener thread to stop.
pub static SHUTDOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Install signal handlers that set the SHUTDOWN flag.
pub fn install_signal_handlers() {
    extern "C" fn handle_signal(_: std::os::raw::c_int) {
        SHUTDOWN.store(true, std::sync::atomic::Ordering::Relaxed);
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

/// Spawn the IPC listener on a dedicated thread.
///
/// When a "stop" command is received, `SHUTDOWN` is set and the thread exits.
pub fn spawn_ipc_thread(listener: UnixListener) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("zylaxion-ipc".into())
        .spawn(move || loop {
            match ipc::handle_one_connection(&listener) {
                Some(cmd) if cmd == "stop" => {
                    log::info!("received 'stop' command via IPC");
                    SHUTDOWN.store(true, std::sync::atomic::Ordering::Relaxed);
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
