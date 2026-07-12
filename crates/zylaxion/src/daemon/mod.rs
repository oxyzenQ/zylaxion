// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Daemonization helpers: fork, PID file, signal handling, IPC thread.

pub mod ipc;

use std::fs;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nix::sys::signal::{self, SigHandler, Signal};
use nix::unistd::{dup2, fork, getpid, setsid, ForkResult, Pid};

/// Ignore `SIGHUP` and `SIGPIPE` so the daemon child does not die or
/// hang when the controlling terminal disappears or a socket write fails.
pub fn ignore_hup_pipe() {
    let _ = unsafe { signal::signal(Signal::SIGHUP, SigHandler::SigIgn) };
    let _ = unsafe { signal::signal(Signal::SIGPIPE, SigHandler::SigIgn) };
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

/// Fork into the background.  The parent prints the child PID and
/// exits with 0.  The child calls `setsid()` to become a session
/// leader and detach from the controlling terminal.
///
/// # Init synchronization (v10.2.0+ — dragonzen audit B9)
///
/// Previously the parent printed the child PID and `exit(0)` immediately
/// after `fork()`. If the child then failed during init (IPC bind failure,
/// audio device open failure, config parse error), the parent had already
/// exited 0 — the user assumed the daemon started successfully, but it
/// had actually died. The user saw no error output because the child had
/// already called `close_std_fds()` before any error path.
///
/// The fix uses a pipe between parent and child. The child returns a
/// [`DaemonChildSync`] handle. The child calls
/// [`DaemonChildSync::signal_success`] once init is complete, or
/// [`DaemonChildSync::signal_failure`] with an error message if init
/// fails. The parent blocks reading from the pipe:
///
/// - **Empty read (EOF)** = child closed the pipe = init succeeded.
///   Parent prints PID, exits 0.
/// - **Non-empty read** = child wrote an error message before closing
///   = init failed. Parent prints the error to stderr, exits 1.
/// - **Pipe read error** = child crashed without writing (e.g. SIGKILL
///   during init). Parent prints a generic error, exits 1.
///
/// This is the canonical daemonization pattern used by `systemd-run`,
/// `nohup`, and BSD's `daemon(3)`.
pub fn daemonize() -> Result<DaemonChildSync, String> {
    // Create a pipe for parent<->child init synchronization BEFORE fork.
    // Both ends are inherited by the child, but each side closes the end
    // it doesn't need.
    let (reader, writer) =
        nix::unistd::pipe().map_err(|e| format!("failed to create init-sync pipe: {e}"))?;

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            // Parent: close the write end (we only read), then block on
            // reading from the pipe until the child signals.
            drop(writer);
            let mut buf = Vec::with_capacity(256);
            // Convert the OwnedFd to a File for read_to_end. `File:
            // From<OwnedFd>` is stable since Rust 1.65.
            let mut reader_file: std::fs::File = reader.into();
            match std::io::Read::read_to_end(&mut reader_file, &mut buf) {
                Ok(_) => {
                    if buf.is_empty() {
                        // Child closed the pipe without writing —
                        // init succeeded.
                        println!("{}", child.as_raw());
                        std::process::exit(0);
                    } else {
                        // Child wrote an error message before closing.
                        let msg = String::from_utf8_lossy(&buf);
                        eprintln!("zylaxion: error: daemon init failed: {msg}");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("zylaxion: error: init-sync pipe read failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Ok(ForkResult::Child) => {
            // Child: close the read end (we only write), then setsid().
            drop(reader);
            if let Err(e) = setsid() {
                // We still hold the writer. Write the error before
                // returning Err — the caller will exit, Drop closes
                // the writer, parent reads the message.
                let msg = format!("setsid failed: {e}");
                let _ = nix::unistd::write(&writer, msg.as_bytes());
                return Err(msg);
            }
            Ok(DaemonChildSync { writer })
        }
        Err(e) => Err(format!("fork failed: {e}")),
    }
}

/// Handle held by the daemon child after [`daemonize`]. The child uses
/// this to signal init completion (or failure) to the parent, which is
/// blocking on the other end of the pipe.
///
/// Drop semantics: if the child crashes or exits without calling either
/// method, the writer is dropped, the pipe closes, and the parent reads
/// EOF (empty). The parent treats this as success — which is wrong if
/// the child actually crashed. To avoid this, the child MUST call
/// `signal_failure` explicitly on every error path before exiting.
/// Failure to do so will cause the parent to report success while the
/// daemon is actually dead.
pub struct DaemonChildSync {
    writer: std::os::fd::OwnedFd,
}

impl DaemonChildSync {
    /// Signal to the parent that init completed successfully. The parent
    /// unblocks, prints the child PID, and exits 0.
    ///
    /// Consumes `self` so the writer is dropped immediately, closing the
    /// pipe. The parent's `read_to_end` returns `Ok(0 bytes)` which is
    /// the success signal.
    pub fn signal_success(self) {
        drop(self.writer);
    }

    /// Signal to the parent that init failed with `error_message`. The
    /// parent unblocks, prints the error to stderr, and exits 1.
    ///
    /// Consumes `self` so the writer is dropped after the write completes,
    /// closing the pipe. The parent's `read_to_end` returns the message
    /// bytes which it prints.
    pub fn signal_failure(self, error_message: &str) {
        let _ = nix::unistd::write(&self.writer, error_message.as_bytes());
        drop(self.writer);
    }
}

/// Close standard file descriptors (stdin, stdout, stderr) and
/// redirect them to `/dev/null` so the daemon cannot read from or
/// write to the controlling terminal.
///
/// # Fd leak fix (v10.2.0+ — dragonzen audit B7)
///
/// Previously this function forgot the `File` handle (to prevent Drop
/// from closing it before `dup2` ran) but never closed the original
/// fd afterward. The fd was leaked for the daemon's lifetime — one
/// fd gone from the process's `ulimit -n` budget. On systems with
/// low fd limits this adds up across daemon restarts.
///
/// The fix explicitly closes the original fd via `nix::unistd::close`
/// after the three `dup2` calls succeed. If any `dup2` fails, we
/// still close the original — the worst case is that one of stdio
/// (0/1/2) is not redirected, which is logged elsewhere.
pub fn close_std_fds() {
    let null = std::fs::File::open("/dev/null").expect("failed to open /dev/null");
    let null_fd = null.as_raw_fd();
    // Prevent File's Drop from closing the fd before dup2 completes.
    std::mem::forget(null);

    let _ = dup2(null_fd, 0); // stdin
    let _ = dup2(null_fd, 1); // stdout
    let _ = dup2(null_fd, 2); // stderr

    // v10.2.0 (B7): explicitly close the original /dev/null fd. The
    // three dup2 calls above each created their own fd references
    // (0, 1, 2) that the kernel tracks independently — this original
    // fd is no longer needed.
    let _ = nix::unistd::close(null_fd);
}

/// Check if a daemon is already running by reading the PID file,
/// verifying the process exists via `kill(pid, None)`, and
/// confirming the process is actually `zylaxion` (not a recycled PID).
///
/// On stale detection (dead process or PID recycled by another program),
/// both the PID file and socket file are removed so the daemon can
/// start cleanly on the next attempt.
///
/// # Identity check (v10.2.0+ — dragonzen audit B4)
///
/// Previously this function read `/proc/<pid>/comm` and required it to
/// equal `"zylaxion"` exactly. That check fails in three real-world
/// scenarios:
///
/// 1. **Renamed binary**: a parallel install like `zylaxion-10.1.0`
///    has `comm = "zylaxion-10.1.0"` (truncated to 15 chars by the
///    kernel → `"zylaxion-10.1.0"` actually fits, but `zylaxion-nightly`
///    would become `"zylaxion-nightly"` → truncated to
///    `"zylaxion-nightl"`).
/// 2. **Symlinked binary**: `/usr/local/bin/zylaxion → /opt/zylaxion/zylaxion`
///    sets `comm` to the symlink name (`"zylaxion"`) — OK — but
///    `/usr/local/bin/zylaxion → /opt/zylaxion-10.1.0/zylaxion` still
///    works because the kernel uses the invoked name. However,
///    `zylaxion` invoked as `./zylaxion` from a build directory sets
///    `comm = "zylaxion"` — also OK. The real problem is rename.
/// 3. **Truncation**: any binary name > 15 chars is truncated by the
///    kernel when read from `/proc/<pid>/comm`.
///
/// The fix reads `/proc/<pid>/exe` (a symlink to the actual binary
/// path) and checks that the basename **starts with** `"zylaxion"`.
/// This handles rename (`zylaxion-10.1.0`), symlinks (`/opt/zylaxion/`),
/// and version suffixes (`zylaxion-nightly`). It also rejects
/// unrelated processes that happen to recycle the PID.
pub fn is_daemon_running() -> Result<Pid, String> {
    let path = ipc::pid_path();
    let content = fs::read_to_string(&path).map_err(|_| "daemon is not running".to_string())?;
    let pid: i32 = content
        .trim()
        .parse()
        .map_err(|_| "invalid PID file".to_string())?;
    let pid = Pid::from_raw(pid);

    // Step 1: Does a process with this PID exist at all?
    //         signal 0 is a no-op that checks liveness.
    if signal::kill(pid, None).is_err() {
        // Process is gone — stale PID file from a crash / kill -9.
        cleanup();
        return Err("daemon is not running (stale PID — cleaned up)".to_string());
    }

    // Step 2: The PID is live, but Linux recycles PIDs aggressively.
    //         A completely unrelated process may now own this PID.
    //         Verify it is actually zylaxion by reading /proc/<pid>/exe
    //         (a symlink to the real binary path) and checking the
    //         basename starts with "zylaxion". This is robust against
    //         renamed binaries, symlinks, and version suffixes.
    let exe_path = format!("/proc/{}/exe", pid.as_raw());
    match fs::read_link(&exe_path) {
        Ok(exe) => {
            let basename = exe.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !basename.starts_with("zylaxion") {
                cleanup();
                return Err(format!(
                    "daemon is not running (PID {pid} recycled by '{basename}' — cleaned up)"
                ));
            }
        }
        Err(_) => {
            // /proc/<pid>/exe unreadable. This can happen if:
            // - The process exited between the kill check and this read
            //   (race).
            // - The process is owned by a different user and we lack
            //   permission to readlink (shouldn't happen for a
            //   user-launched daemon, but defensive).
            // - The kernel doesn't expose /proc (non-Linux — not our
            //   target).
            // Treat as stale and clean up — safer than falsely claiming
            // a daemon is running.
            cleanup();
            return Err("daemon is not running (stale PID — cleaned up)".to_string());
        }
    }

    Ok(pid)
}

/// Spawn the IPC listener on a dedicated background thread.
///
/// Handles the `stop` command: sets `stop_flag` to `true` and exits the
/// thread. The main-thread orchestrator loop checks this flag and breaks
/// cleanly, dropping `CpalSink` and releasing audio.
///
/// As of v0.3.1, config reloading is handled by the separate
/// `config-watcher` thread (see `commands::daemon::spawn_config_watcher`)
/// which polls `config.toml`'s mtime. The `reload` IPC command has been
/// removed — users just edit and save the config file.
///
/// # Robustness (v10.2.0+ — dragonzen audit B6/B10/S2)
///
/// Three fixes landed here:
///
/// 1. **B6**: previously used `.expect("failed to spawn IPC thread")`,
///    which panicked the main thread if `Builder::spawn` failed (thread
///    limit, OOM). Now returns `Result`, and the caller logs the error
///    gracefully instead of crashing the daemon.
/// 2. **B10**: previously blocked forever in `listener.accept()`. When
///    `stop_flag` was set (SIGTERM / IPC `stop`), the orchestrator
///    exited cleanly but the IPC thread stayed blocked — leaking until
///    process exit. Now the listener is set to non-blocking and the
///    loop polls `stop_flag` between accept attempts with a 100 ms
///    backoff. The thread exits within 100 ms of `stop_flag` being set.
/// 3. **S2**: previously, a panic inside `handle_one_connection` (e.g.
///    from a future code change that introduced an `unwrap` on `None`)
///    killed the IPC thread silently — the daemon could no longer
///    receive `stop` commands and had to be killed with `SIGTERM`. Now
///    each connection is wrapped in `std::panic::catch_unwind`. On
///    `Err`, the panic is logged and the loop continues.
pub fn spawn_ipc_thread(
    listener: UnixListener,
    stop_flag: Arc<AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    // Set the listener to non-blocking so we can poll `stop_flag`
    // between accept attempts. Without this, `accept()` blocks forever
    // and the thread cannot exit until a new client connects — which
    // never happens during shutdown.
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("failed to set IPC listener non-blocking: {e}"))?;

    std::thread::Builder::new()
        .name("zylaxion-ipc".into())
        .spawn(move || loop {
            // Check stop flag at the top of every iteration so we exit
            // promptly when the orchestrator is shutting down.
            if stop_flag.load(Ordering::Relaxed) {
                log::info!("IPC thread exiting (stop flag set)");
                return;
            }

            // `accept()` on a non-blocking listener returns immediately.
            // `WouldBlock` means no client is connecting — sleep briefly
            // and retry. `Err(WouldBlock)` is the common case.
            match listener.accept() {
                Ok((stream, _addr)) => {
                    // Reset to blocking for the duration of the request
                    // handling so `read_line` / `write_all` behave
                    // normally. The stream is dropped at the end of
                    // `handle_one_connection`, restoring the fd.
                    let _ = stream.set_nonblocking(false);

                    // Wrap each connection in catch_unwind so a panic
                    // in handle_one_connection doesn't kill the IPC
                    // thread (S2). The daemon stays responsive to
                    // future `stop` commands.
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        ipc::handle_one_connection_on(&listener, &stream)
                    }));
                    match result {
                        Ok(Some(cmd)) if cmd == "stop" => {
                            log::info!("received 'stop' command via IPC");
                            stop_flag.store(true, Ordering::Relaxed);
                            return;
                        }
                        Ok(_) => continue,
                        Err(payload) => {
                            log::error!(
                                "IPC connection panicked: {:?} — thread continues",
                                payload
                            );
                            continue;
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No client connecting — poll stop_flag after a
                    // short sleep. 100 ms gives sub-100ms shutdown
                    // responsiveness while keeping idle CPU at ~10 Hz.
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                Err(e) => {
                    log::warn!("IPC accept() error: {e} — retrying in 100 ms");
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
            }
        })
        .map_err(|e| format!("failed to spawn IPC thread: {e}"))
}

/// Connect to the daemon, send a command, print the result.
pub fn client_send_and_print(cmd: &str) {
    match ipc::send_command(cmd) {
        Ok(resp) => {
            if resp.ok {
                println!("{}", resp.message);
            } else {
                crate::error_format::error(resp.message);
                std::process::exit(1);
            }
        }
        Err(e) => {
            crate::error_format::error(e);
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
                crate::error_format::error(resp.message);
                std::process::exit(1);
            }
        }
        Err(e) => {
            crate::error_format::error(e);
            std::process::exit(1);
        }
    }
}
