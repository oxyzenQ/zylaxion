// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Unix Domain Socket IPC for the Zylaxion daemon.
//!
//! Uses only `std::os::unix::net` — no nix socket API, no trait import
//! headaches.  The daemon listens on `$XDG_RUNTIME_DIR/zylaxion.sock`.
//!
//! # Security
//!
//! The socket file is created with permission `0o600` (read/write owner
//! only) so that only the user who started the daemon can send IPC
//! commands (`stop`, `status`). Without this, any user with read access
//! to `$XDG_RUNTIME_DIR` could shut down or probe the daemon. The umask
//! is also temporarily dropped to `0o077` during `bind()` to guarantee
//! the file is created with the strict mode even if the parent shell
//! inherits a permissive umask like `0o022`.

use std::io::{BufRead, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// JSON command sent by the CLI to the daemon.
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcRequest {
    pub cmd: String,
}

/// JSON response sent by the daemon to the CLI.
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    pub message: String,
}

/// Return the path to the daemon socket.
pub fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join("zylaxion.sock")
}

/// Return the path to the daemon PID file.
pub fn pid_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime).join("zylaxion.pid")
}

/// Strict permission applied to the IPC socket file: `0o600`
/// (read+write for owner only). This blocks other users on the system
/// from issuing `stop` / `status` commands to the daemon.
const SOCKET_FILE_MODE: u32 = 0o600;

/// Create a Unix domain socket listener.
///
/// After binding, the socket file's permission bits are explicitly
/// tightened to `0o600` (owner-only read/write). This is a defense-
/// in-depth measure:
///
/// 1. The `umask` is temporarily set to `0o077` during `bind()` so the
///    file is created with the strict mode even if the parent shell
///    inherits a permissive umask like `0o022`.
/// 2. After `bind()` returns, we `chmod` the path again to `0o600` in
///    case the filesystem or a race condition loosened the mode.
///
/// # Errors
///
/// Returns a human-readable error string if the socket cannot be
/// removed, bound, or chmod'd.
pub fn create_listener() -> Result<UnixListener, String> {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);

    // Save the current umask so we can restore it after bind().
    // SAFETY: `umask(2)` is process-wide and async-signal-safe. We are
    // single-threaded here (the IPC thread hasn't spawned yet), so
    // there is no race with other code that might depend on the umask.
    let saved_umask = unsafe { libc::umask(0o077) };

    let listener = UnixListener::bind(&path);

    // Restore the umask unconditionally — failing to do so would
    // silently tighten file modes for the rest of the process lifetime.
    unsafe {
        libc::umask(saved_umask);
    }

    let listener = listener.map_err(|e| format!("failed to bind socket: {e}"))?;

    // Defense in depth: explicitly chmod the socket file to 0o600.
    // The umask set above SHOULD already produce this mode, but a
    // concurrent rename or a permissive filesystem could loosen it.
    // We chmod unconditionally so the mode is correct regardless of
    // what `bind()` produced.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(SOCKET_FILE_MODE))
        .map_err(|e| format!("failed to chmod socket to 0o600: {e}"))?;

    log::info!(
        "IPC socket bound at {} with mode 0o600 (owner-only)",
        path.display()
    );

    Ok(listener)
}

/// Connect to the daemon socket and send a JSON command.
pub fn send_command(cmd: &str) -> Result<IpcResponse, String> {
    let path = socket_path();
    if !path.exists() {
        return Err("zylaxion daemon is not running".into());
    }

    let stream = std::os::unix::net::UnixStream::connect(&path)
        .map_err(|e| format!("failed to connect to daemon: {e}"))?;

    let mut reader = std::io::BufReader::new(&stream);
    let mut writer = std::io::BufWriter::new(&stream);

    let request = IpcRequest {
        cmd: cmd.to_string(),
    };
    let mut json = serde_json::to_string(&request).unwrap_or_default();
    json.push('\n');

    writer
        .write_all(json.as_bytes())
        .map_err(|e| format!("failed to send command: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("failed to flush: {e}"))?;

    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .map_err(|e| format!("failed to read response: {e}"))?;

    serde_json::from_str::<IpcResponse>(&response_line)
        .map_err(|e| format!("invalid response from daemon: {e}"))
}

/// Accept one connection, read a command, dispatch it.
///
/// Returns `Some("stop")` if the daemon should shut down, `None` otherwise.
pub fn handle_one_connection(listener: &UnixListener) -> Option<String> {
    let (stream, _) = match listener.accept() {
        Ok(s) => s,
        Err(_) => return None,
    };

    stream.set_nonblocking(false).ok();
    let mut reader = std::io::BufReader::new(&stream);
    let mut writer = std::io::BufWriter::new(&stream);

    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return None;
    }

    let request: IpcRequest = match serde_json::from_str(&line) {
        Ok(r) => r,
        Err(e) => {
            let resp = IpcResponse {
                ok: false,
                message: format!("invalid JSON: {e}"),
            };
            let _ = writeln!(
                writer,
                "{}\n",
                serde_json::to_string(&resp).unwrap_or_default()
            );
            return None;
        }
    };

    let response = match request.cmd.as_str() {
        "stop" => IpcResponse {
            ok: true,
            message: "shutting down".into(),
        },
        "status" => IpcResponse {
            ok: true,
            message: "running".into(),
        },
        other => IpcResponse {
            ok: false,
            message: format!("unknown command: {other}"),
        },
    };

    let _ = writeln!(
        writer,
        "{}\n",
        serde_json::to_string(&response).unwrap_or_default()
    );
    // Return the command string so the IPC thread can dispatch on it
    // (stop, reload, etc.).
    Some(request.cmd)
}
