// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// ipc_tests.rs — the daemon/ipc.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

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
