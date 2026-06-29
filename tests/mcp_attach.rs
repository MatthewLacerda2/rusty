//! MCP attach-mode bridge end-to-end (#307).
//! Gated on `dev`. Drives `rusty::dev::mcp::run` with the `SocketClient` backend against
//! a *fake engine* — a listener speaking the #282 framed line protocol — asserting that
//! tool calls are forwarded over the socket and the framed replies are mapped to MCP
//! results. No real world is booted; the fake engine stands in for a running window.
#![cfg(feature = "dev")]

use std::io::{BufRead, BufReader, Write};
use std::thread;

use serde_json::Value;

use rusty::dev::mcp::{self, attach::SocketClient};

/// A connection splittable into reader + writer halves — both fake transports
/// (unix socket, loopback TCP) provide `try_clone`.
trait Dup: std::io::Read + Write + Sized {
    fn dup(&self) -> std::io::Result<Self>;
}
#[cfg(unix)]
impl Dup for std::os::unix::net::UnixStream {
    fn dup(&self) -> std::io::Result<Self> {
        self.try_clone()
    }
}
#[cfg(windows)]
impl Dup for std::net::TcpStream {
    fn dup(&self) -> std::io::Result<Self> {
        self.try_clone()
    }
}

/// Per-connection: read each command line and reply with a framed JSON line that
/// echoes the received line back as the `result`, so a test can assert the exact line
/// crossed the socket.
fn serve<S: Dup>(stream: S) {
    let mut writer = match stream.dup() {
        Ok(w) => w,
        Err(_) => return,
    };
    let reader = BufReader::new(stream);
    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let reply = format!(r#"{{"ok":true,"result":{}}}"#, Value::String(line));
        if writeln!(writer, "{reply}").is_err() {
            break;
        }
        let _ = writer.flush();
    }
}

/// Spawn a fake engine on a fresh address and return that address for
/// `SocketClient::new`. The accept loop handles one connection at a time — all the
/// per-call-connecting client needs.
#[cfg(unix)]
fn fake_engine() -> String {
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU32, Ordering};
    // Cargo runs the tests in one process, so a process-id path would collide between
    // tests; a per-bind counter keeps each fake engine's socket distinct.
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("rusty-attach-it-{}-{n}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).expect("bind fake engine");
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            serve(stream);
        }
    });
    path.display().to_string()
}
#[cfg(windows)]
fn fake_engine() -> String {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake engine");
    let addr = listener.local_addr().unwrap().to_string();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            serve(stream);
        }
    });
    addr
}

/// Drive JSON-RPC lines through the attach backend against `addr` and parse the
/// response lines.
fn drive(addr: String, lines: &[&str]) -> Vec<Value> {
    let backend = SocketClient::new(Some(addr));
    let input = lines.join("\n").into_bytes();
    let mut out = Vec::new();
    mcp::run(&backend, &input[..], &mut out).expect("runs to EOF");
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("each response line is JSON"))
        .collect()
}

#[test]
fn eval_is_forwarded_over_the_socket_and_the_reply_is_mapped() {
    let out = drive(
        fake_engine(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"eval","arguments":{"line":"Scene.CreateEntity(\"Widget\",\"Box\")"}}}"#,
        ],
    );
    assert_eq!(out[0]["result"]["isError"], false);
    // The fake engine echoes the forwarded line back, proving the exact Lua line
    // crossed the socket.
    assert_eq!(
        out[0]["result"]["content"][0]["text"],
        "Scene.CreateEntity(\"Widget\",\"Box\")"
    );
}

#[test]
fn multiple_calls_each_reconnect_and_succeed() {
    let out = drive(
        fake_engine(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"eval","arguments":{"line":"a"}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"eval","arguments":{"line":"b"}}}"#,
        ],
    );
    assert_eq!(out[0]["result"]["content"][0]["text"], "a");
    assert_eq!(out[1]["result"]["content"][0]["text"], "b");
}

#[test]
fn resources_read_works_without_any_engine() {
    // The scripting-API resource is embedded, so attach mode serves it even with no
    // window running — you can read the API doc before opening the engine.
    let bogus = if cfg!(windows) {
        "127.0.0.1:1".to_string()
    } else {
        std::env::temp_dir()
            .join("rusty-attach-noengine.sock")
            .display()
            .to_string()
    };
    let out = drive(
        bogus,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"rusty://scripting-api.md"}}"#,
        ],
    );
    let text = out[0]["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("# Scripting API reference"));
}
