//! src/dev/command_channel/transport.rs — the socket listener + per-connection
//! handler threads for the windowed command channel (#282).
//!
//! This is the **off-main-thread** half: it accepts connections and reads/writes
//! lines, but it never touches the world (which is `!Send`). Each line is handed to
//! the main loop over the shared `Sender<Command>`; the handler blocks for the
//! framed reply and writes it back. See the parent module for the protocol.
//!
//! Two transports behind clean `cfg`s, std only (no new dependency): a unix domain
//! socket on unix, a loopback `TcpListener` on windows. Neither branch references a
//! symbol absent on the other.

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::{self, Sender};
use std::thread;

use super::Command;

/// A bound listener's identity: the human-readable address to log, plus (on unix)
/// the socket path the channel removes on drop. The `path` field is unix-only — TCP
/// needs no filesystem cleanup.
pub(super) struct Bound {
    pub label: String,
    #[cfg(unix)]
    pub path: std::path::PathBuf,
}

// ---------------------------------------------------------------------------
// Transport: unix domain socket.
// ---------------------------------------------------------------------------

/// Resolve the unix socket path: `$RUSTY_CMD_SOCK`, else `$XDG_RUNTIME_DIR/rusty.sock`,
/// else `/tmp/rusty.sock`.
#[cfg(unix)]
fn socket_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("RUSTY_CMD_SOCK") {
        return std::path::PathBuf::from(path);
    }
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return std::path::Path::new(&dir).join("rusty.sock");
    }
    std::path::PathBuf::from("/tmp/rusty.sock")
}

/// Bind the unix socket (clearing any stale file first) and spawn the listener.
#[cfg(unix)]
pub(super) fn bind_and_listen(tx: Sender<Command>) -> std::io::Result<Bound> {
    use std::os::unix::net::UnixListener;

    let path = socket_path();
    // A stale socket file from a prior run blocks the bind; remove it first.
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    let label = path.display().to_string();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            spawn_handler(stream, tx.clone());
        }
    });
    Ok(Bound { label, path })
}

// ---------------------------------------------------------------------------
// Transport: windows TCP loopback (std only — no named-pipe dependency).
// ---------------------------------------------------------------------------

/// Resolve the TCP bind address: `$RUSTY_CMD_ADDR`, else `127.0.0.1:8787`.
#[cfg(windows)]
fn tcp_addr() -> String {
    std::env::var("RUSTY_CMD_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".to_string())
}

/// Bind the loopback TCP listener and spawn the accept loop.
#[cfg(windows)]
pub(super) fn bind_and_listen(tx: Sender<Command>) -> std::io::Result<Bound> {
    use std::net::TcpListener;

    let addr = tcp_addr();
    let listener = TcpListener::bind(&addr)?;
    // Report the actual bound address (handles an explicit `:0` ephemeral port).
    let label = listener.local_addr().map(|a| a.to_string()).unwrap_or(addr);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            spawn_handler(stream, tx.clone());
        }
    });
    Ok(Bound { label })
}

// ---------------------------------------------------------------------------
// Per-connection handler (shared by both transports).
// ---------------------------------------------------------------------------

/// A connection stream: readable + writable, and clonable into a second owned
/// handle to the same connection (one for the buffered reader half, one for the
/// writer half). Implemented for both transports so [`spawn_handler`] stays generic.
trait Stream: std::io::Read + Write + Send + Sized + 'static {
    /// A cloned handle to the same underlying connection, or an I/O error.
    fn dup(&self) -> std::io::Result<Self>;
}

#[cfg(unix)]
impl Stream for std::os::unix::net::UnixStream {
    fn dup(&self) -> std::io::Result<Self> {
        self.try_clone()
    }
}

#[cfg(windows)]
impl Stream for std::net::TcpStream {
    fn dup(&self) -> std::io::Result<Self> {
        self.try_clone()
    }
}

/// Per-connection handler: spawn a thread that reads lines, forwards each to the
/// main loop, blocks for the framed reply, and writes it back. Generic over the
/// stream type so the unix and windows branches share one body.
///
/// A blank line is skipped (parity with the session). A clone/read/write error or
/// EOF ends only this connection — never the listener or the engine. If the main
/// loop's receiver is gone (channel dropped on shutdown) the handler stops.
fn spawn_handler<S: Stream>(stream: S, tx: Sender<Command>) {
    thread::spawn(move || {
        // Split the connection into independent reader + writer halves. A failed
        // clone just drops this one connection; the listener keeps accepting.
        let reader = match stream.dup() {
            Ok(r) => BufReader::new(r),
            Err(_) => return,
        };
        let mut writer = stream;
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break, // read error / EOF: drop this connection.
            };
            if line.trim().is_empty() {
                continue;
            }
            // Hand the line to the main loop and block for its framed reply. The
            // main loop owns the world; we never touch it from this thread.
            let (reply_tx, reply_rx) = mpsc::channel::<String>();
            if tx.send((line, reply_tx)).is_err() {
                break; // main loop gone (engine shutting down).
            }
            let response = match reply_rx.recv() {
                Ok(r) => r,
                Err(_) => break,
            };
            if writeln!(writer, "{}", response).is_err() || writer.flush().is_err() {
                break; // client disconnected.
            }
        }
    });
}
