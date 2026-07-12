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
///
/// Uses `pathguard::resolve_runtime_dir()` to validate `$XDG_RUNTIME_DIR`
/// — falls back to `/tmp` if the env var points to a dangerous system
/// path (e.g. `/etc`, `/usr`, `~/.ssh`).
pub fn socket_path() -> PathBuf {
    crate::pathguard::resolve_runtime_dir().join("zylaxion.sock")
}

/// Return the path to the daemon PID file.
///
/// Same pathguard validation as `socket_path()`.
pub fn pid_path() -> PathBuf {
    crate::pathguard::resolve_runtime_dir().join("zylaxion.pid")
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

/// Handle a single already-accepted IPC connection.
///
/// Reads one JSON request line, dispatches the command, writes one JSON
/// response line, and returns the command string so the caller (the IPC
/// thread) can act on it (e.g. set `stop_flag` for `"stop"`).
///
/// The `listener` parameter is accepted for future use (e.g. inspecting
/// the listener's local address) but currently unused. The IPC thread
/// (see `daemon::spawn_ipc_thread`) performs `accept()` itself with a
/// non-blocking listener and a `stop_flag` poll, then passes the
/// accepted stream here.
pub fn handle_one_connection_on(
    _listener: &UnixListener,
    stream: &std::os::unix::net::UnixStream,
) -> Option<String> {
    stream.set_nonblocking(false).ok();
    let mut reader = std::io::BufReader::new(stream);
    let mut writer = std::io::BufWriter::new(stream);

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

// ── Tests ───────────────────────────────────────────────────────────────
//
// v10.2.0 (dragonzen audit P7): IPC layer unit tests. These exercise
// the JSON request/response protocol without needing a running daemon
// or a real audio/input stack. We create a UnixStream pair (one end
// acts as the "client", the other as the "daemon's accepted stream")
// and a dummy UnixListener (unused by handle_one_connection_on but
// required by the signature for future extensibility).

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    /// Create a dummy UnixListener on a unique temp path. Used only to
    /// satisfy the `&UnixListener` parameter of `handle_one_connection_on`
    /// (which is currently unused but reserved for future use).
    fn dummy_listener() -> UnixListener {
        let path = std::env::temp_dir().join(format!(
            "zylaxion-ipc-test-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).expect("bind dummy listener");
        // Clean up after the test.
        let path_clone = path.clone();
        // defer removal by spawning a thread that sleeps briefly then removes
        // (can't use Drop on UnixListener because we don't own the path)
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = std::fs::remove_file(&path_clone);
        });
        listener
    }

    /// Send a JSON line through the client end of a UnixStream pair,
    /// then read the response line back.
    fn send_and_receive(client: &UnixStream, json_line: &str) -> String {
        use std::io::Write;
        client.set_nonblocking(false).ok();
        let mut writer = std::io::BufWriter::new(client);
        writer
            .write_all(json_line.as_bytes())
            .expect("write request");
        writer.flush().expect("flush request");
        // Read the response. We need a timeout so the test doesn't hang
        // forever if the daemon side doesn't respond. Use a 1-second
        // deadline via set_read_timeout.
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .expect("set_read_timeout");
        let mut reader = std::io::BufReader::new(client);
        let mut response = String::new();
        reader.read_line(&mut response).expect("read response");
        response
    }

    #[test]
    fn handle_stop_command_returns_stop_and_ok_response() {
        let listener = dummy_listener();
        let (client, server) = UnixStream::pair().expect("pair");
        // Handle the "daemon" side in a thread — handle_one_connection_on
        // blocks on read_line until the client writes.
        let handle = std::thread::spawn(move || handle_one_connection_on(&listener, &server));
        let request = serde_json::to_string(&IpcRequest {
            cmd: "stop".to_string(),
        })
        .unwrap();
        let response = send_and_receive(&client, &format!("{request}\n"));
        let result = handle.join().expect("thread join");

        assert_eq!(result, Some("stop".to_string()));
        let resp: IpcResponse = serde_json::from_str(response.trim()).expect("parse response");
        assert!(resp.ok, "stop should return ok=true");
        assert_eq!(resp.message, "shutting down");
    }

    #[test]
    fn handle_status_command_returns_status_and_ok_response() {
        let listener = dummy_listener();
        let (client, server) = UnixStream::pair().expect("pair");
        let handle = std::thread::spawn(move || handle_one_connection_on(&listener, &server));
        let request = serde_json::to_string(&IpcRequest {
            cmd: "status".to_string(),
        })
        .unwrap();
        let response = send_and_receive(&client, &format!("{request}\n"));
        let result = handle.join().expect("thread join");

        assert_eq!(result, Some("status".to_string()));
        let resp: IpcResponse = serde_json::from_str(response.trim()).expect("parse response");
        assert!(resp.ok, "status should return ok=true");
        assert_eq!(resp.message, "running");
    }

    #[test]
    fn handle_unknown_command_returns_cmd_and_error_response() {
        let listener = dummy_listener();
        let (client, server) = UnixStream::pair().expect("pair");
        let handle = std::thread::spawn(move || handle_one_connection_on(&listener, &server));
        let request = serde_json::to_string(&IpcRequest {
            cmd: "frobnicate".to_string(),
        })
        .unwrap();
        let response = send_and_receive(&client, &format!("{request}\n"));
        let result = handle.join().expect("thread join");

        assert_eq!(result, Some("frobnicate".to_string()));
        let resp: IpcResponse = serde_json::from_str(response.trim()).expect("parse response");
        assert!(!resp.ok, "unknown command should return ok=false");
        assert!(resp.message.contains("unknown command: frobnicate"));
    }

    #[test]
    fn handle_malformed_json_returns_none_and_error_response() {
        let listener = dummy_listener();
        let (client, server) = UnixStream::pair().expect("pair");
        let handle = std::thread::spawn(move || handle_one_connection_on(&listener, &server));
        // Send invalid JSON.
        let response = send_and_receive(&client, "this is not json\n");
        let result = handle.join().expect("thread join");

        assert_eq!(result, None, "malformed JSON should return None");
        let resp: IpcResponse = serde_json::from_str(response.trim()).expect("parse response");
        assert!(!resp.ok, "malformed JSON should return ok=false");
        assert!(resp.message.contains("invalid JSON"));
    }

    #[test]
    fn handle_empty_input_returns_none() {
        let listener = dummy_listener();
        let (client, server) = UnixStream::pair().expect("pair");
        let handle = std::thread::spawn(move || handle_one_connection_on(&listener, &server));
        // Send just a newline — empty line. serde_json will fail to parse
        // an empty string, producing an "invalid JSON" response.
        let response = send_and_receive(&client, "\n");
        let result = handle.join().expect("thread join");

        assert_eq!(result, None, "empty input should return None");
        // The response should still be valid JSON with ok=false.
        let trimmed = response.trim();
        if !trimmed.is_empty() {
            let resp: IpcResponse = serde_json::from_str(trimmed).expect("parse response");
            assert!(!resp.ok);
        }
    }

    #[test]
    fn ipc_request_serializes_cmd_field() {
        let req = IpcRequest {
            cmd: "stop".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"cmd\":\"stop\""));
        let back: IpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.cmd, "stop");
    }

    #[test]
    fn ipc_response_serializes_ok_and_message() {
        let resp = IpcResponse {
            ok: true,
            message: "shutting down".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"message\":\"shutting down\""));
        let back: IpcResponse = serde_json::from_str(&json).unwrap();
        assert!(back.ok);
        assert_eq!(back.message, "shutting down");
    }
}
