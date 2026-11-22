//! The fake engine: a listener speaking the framed line protocol, plus the driver.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;

use serde_json::Value;

use rusty::dev::mcp::{self, attach::SocketClient};

/// Replies to each command line with a framed JSON line echoing it as `result`; a line asking for progress (`ASK`) streams one progress line first and has the ask removed.
fn serve(stream: UnixStream) {
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    let reader = BufReader::new(stream);
    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let (asked, line) = rusty::dev::command_channel::asks(&line);
        let line = line.to_string();
        if asked {
            let _ = writeln!(writer, r#"{{"progress":"[Lighting] 0:05 lightmaps 40%"}}"#);
        }
        let reply = format!(r#"{{"ok":true,"result":{}}}"#, Value::String(line));
        if writeln!(writer, "{reply}").is_err() {
            break;
        }
        let _ = writer.flush();
    }
}

/// Spawn a fake engine on a fresh address; the accept loop handles one connection at a time.
pub(super) fn fake_engine() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    // A per-bind counter, not the process id: tests share one process.
    static NEXT: AtomicU32 = AtomicU32::new(0);
    // lint: allow(test-static-write) a unique-name counter: no test reads its value back, so sharing it is the point
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let path =
        // lint: allow(test-temp-dir) a Unix socket path must stay short (sun_path is ~104 bytes)
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

pub(super) fn drive(addr: String, lines: &[&str]) -> Vec<Value> {
    let backend = SocketClient::new(Some(addr));
    let input = lines.join("\n").into_bytes();
    let out = mcp::progress::Captured::default();
    mcp::run(&backend, &input[..], out.clone()).expect("runs to EOF");
    out.text()
        .lines()
        .map(|l| serde_json::from_str(l).expect("each response line is JSON"))
        .collect()
}
