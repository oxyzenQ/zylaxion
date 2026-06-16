// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unix Domain Socket IPC for the Zylaxion daemon.
//!
//! Uses only `std::os::unix::net` — no nix socket API, no trait import
//! headaches.  The daemon listens on `$XDG_RUNTIME_DIR/zylaxion.sock`.
//!
//! Wire protocol: one JSON object per connection, newline-delimited.

use std::io::{BufRead, Write};
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

/// Create a Unix domain socket listener.
pub fn create_listener() -> Result<UnixListener, String> {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    UnixListener::bind(&path).map_err(|e| format!("failed to bind socket: {e}"))
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
    let mut json = serde_json::to_string(&request).unwrap();
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
            let _ = writeln!(writer, "{}\n", serde_json::to_string(&resp).unwrap());
            return None;
        }
    };

    let (response, should_stop) = match request.cmd.as_str() {
        "stop" => (
            IpcResponse {
                ok: true,
                message: "shutting down".into(),
            },
            true,
        ),
        "status" => (
            IpcResponse {
                ok: true,
                message: "running".into(),
            },
            false,
        ),
        other => (
            IpcResponse {
                ok: false,
                message: format!("unknown command: {other}"),
            },
            false,
        ),
    };

    let _ = writeln!(writer, "{}\n", serde_json::to_string(&response).unwrap());
    if should_stop {
        Some(request.cmd)
    } else {
        None
    }
}
