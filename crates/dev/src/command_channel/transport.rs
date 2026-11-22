use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::{self, Sender};
use std::thread;

use super::{progress_line, Command, Reply};

pub(super) struct Bound {
    pub label: String,
    pub path: std::path::PathBuf,
}

pub(super) fn socket_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("RUSTY_CMD_SOCK") {
        return std::path::PathBuf::from(path);
    }
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return std::path::Path::new(&dir).join("rusty.sock");
    }
    std::path::PathBuf::from("/tmp/rusty.sock")
}

pub(super) fn bind_and_listen(
    tx: Sender<Command>,
    path: std::path::PathBuf,
) -> std::io::Result<Bound> {
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

fn spawn_handler(stream: UnixStream, tx: Sender<Command>) {
    thread::spawn(move || {
        let reader = match stream.try_clone() {
            Ok(r) => BufReader::new(r),
            Err(_) => return,
        };
        let mut writer = stream;
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if line.trim().is_empty() {
                continue;
            }
            let (reply_tx, reply_rx) = mpsc::channel::<Reply>();
            if tx.send((line, reply_tx)).is_err() {
                break;
            }
            // The editor may sleep outside Play, and only a frame drains the queue.
            rusty_core::wake::frame();
            if !write_replies(&mut writer, &reply_rx) {
                break;
            }
        }
    });
}

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
