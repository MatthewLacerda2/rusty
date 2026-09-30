//! The fake engine: a listener speaking the #282 framed line protocol, plus the driver.

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
pub(super) fn fake_engine() -> String {
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
pub(super) fn fake_engine() -> String {
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
pub(super) fn drive(addr: String, lines: &[&str]) -> Vec<Value> {
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
