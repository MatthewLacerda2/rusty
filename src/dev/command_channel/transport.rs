//! src/dev/command_channel/transport.rs — the socket listener + per-connection
//! handler threads for the windowed command channel (#282).
//!
//! This is the **off-main-thread** half: it accepts connections and reads/writes
//! lines, but it never touches the world (which is `!Send`). Each line is handed to
//! the main loop over the shared `Sender<Command>`; the handler blocks for the
//! framed reply and writes it back. See the parent module for the protocol.
//!
//! The transport is a unix domain socket, std only (no new dependency).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::{self, Sender};
use std::thread;

use super::{progress_line, Command, Reply};

/// A bound listener's identity: the human-readable address to log, plus the socket
/// path the channel removes on drop.
pub(super) struct Bound {
    pub label: String,
    pub path: std::path::PathBuf,
}

/// Resolve the unix socket path: `$RUSTY_CMD_SOCK`, else `$XDG_RUNTIME_DIR/rusty.sock`,
/// else `/tmp/rusty.sock`. Shared with the dial-in [`super::client`] so a client and
/// the window it connects to never disagree about where the socket is.
pub(super) fn socket_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("RUSTY_CMD_SOCK") {
        return std::path::PathBuf::from(path);
    }
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return std::path::Path::new(&dir).join("rusty.sock");
    }
    std::path::PathBuf::from("/tmp/rusty.sock")
}

/// Bind the unix socket at `path` (clearing any stale file first) and spawn the
/// listener. The path is a parameter so a test binds its own without touching the
/// process environment (#977).
pub(super) fn bind_and_listen(
    tx: Sender<Command>,
    path: std::path::PathBuf,
) -> std::io::Result<Bound> {
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
// Per-connection handler.
// ---------------------------------------------------------------------------

/// Per-connection handler: spawn a thread that reads lines, forwards each to the
/// main loop, blocks for the framed reply, and writes it back (after the progress
/// lines a bake told while the line ran).
///
/// A blank line is skipped (parity with the session). A clone/read/write error or
/// EOF ends only this connection — never the listener or the engine. If the main
/// loop's receiver is gone (channel dropped on shutdown) the handler stops.
fn spawn_handler(stream: UnixStream, tx: Sender<Command>) {
    thread::spawn(move || {
        // Split the connection into independent reader + writer halves. A failed
        // clone just drops this one connection; the listener keeps accepting.
        let reader = match stream.try_clone() {
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
            let (reply_tx, reply_rx) = mpsc::channel::<Reply>();
            if tx.send((line, reply_tx)).is_err() {
                break; // main loop gone (engine shutting down).
            }
            // The editor may be asleep outside Play (#1721): only a frame drains it.
            crate::core::wake::frame();
            if !write_replies(&mut writer, &reply_rx) {
                break; // main loop gone, or the client disconnected.
            }
        }
    });
}

/// Write a command's progress lines as they come (#1999), then its reply. `false`
/// when the main loop dropped the command or the client went away.
pub(super) fn write_replies(writer: &mut impl Write, replies: &mpsc::Receiver<Reply>) -> bool {
    loop {
        let (line, last) = match replies.recv() {
            Ok(Reply::Progress(text)) => (progress_line(&text), false),
            Ok(Reply::Done(line)) => (line, true),
            Err(_) => return false,
        };
        if writeln!(writer, "{line}").is_err() || writer.flush().is_err() {
            return false;
        }
        if last {
            return true;
        }
    }
}
