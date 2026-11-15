//! The frame log's writer (#2206): a thread that owns the segment files, so a
//! frame only hands its lines over and never waits on a disk.
//!
//! Segments are `frames-<n>.jsonl`, numbered up; a log that starts continues after
//! the highest number it finds and keeps the earlier ones under its budget, so a
//! look-back reaches across a restart. Each holds about a tenth of the budget; when
//! it is full it is synced and closed and the next one opens, then the oldest are
//! deleted until the log fits the budget again, and any whose newest record is
//! older than the period. Pruning runs when a segment closes and once per batch
//! of frames the thread drains, never per line.
//!
//! **One writer per folder.** The log takes an advisory lock on `saved/logs/.lock`
//! for its life (released by the OS when the process ends, a crash included); a
//! second process on the same project (a second editor, or a dev `player` with its
//! log on) finds it held, says so once and logs nothing, rather than numbering
//! and pruning segments the first one is writing.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::JoinHandle;

use super::now_ms;
use super::segments::Segments;
pub use super::segments::LOCK;

/// How many frames' lines may wait for the writer before a frame's are dropped.
const QUEUE: usize = 240;

/// Lines dropped while the writer was behind: how many, and the newest one's tick.
#[derive(Default)]
struct Dropped {
    records: AtomicU64,
    tick: AtomicU64,
}

/// The frame's side: a bounded channel into the writer thread.
pub struct Writer {
    tx: Option<SyncSender<Vec<String>>>,
    dropped: Arc<Dropped>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    /// Open the log in `dir` (made if missing) under `budget` bytes and, when
    /// given, `period` ms, and start its thread. Fails when another process holds
    /// the folder's [`LOCK`].
    pub fn start(dir: &Path, budget: u64, period: Option<u64>) -> std::io::Result<Self> {
        let segments = Segments::open(dir, budget, period)?;
        let (tx, rx) = sync_channel(QUEUE);
        let dropped = Arc::new(Dropped::default());
        let behind = Arc::clone(&dropped);
        let thread = std::thread::Builder::new()
            .name("frame-log".into())
            .spawn(move || drain(rx, segments, &behind))?;
        Ok(Self {
            tx: Some(tx),
            dropped,
            thread: Some(thread),
        })
    }

    /// Hand one frame's lines, placed at `tick`, to the writer, never waiting:
    /// when its queue is full they are dropped and counted.
    pub fn send(&mut self, lines: Vec<String>, tick: u64) {
        let (Some(tx), false) = (&self.tx, lines.is_empty()) else {
            return;
        };
        let n = lines.len() as u64;
        match tx.try_send(lines) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.tick.store(tick, Ordering::Relaxed);
                self.dropped.records.fetch_add(n, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => self.tx = None,
        }
    }
}

impl Drop for Writer {
    /// Close the channel and wait for the thread to write what is queued.
    fn drop(&mut self) {
        self.tx = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The writer thread: write each frame's lines, say what was dropped before
/// them, and flush and prune whenever the queue is empty, so a reader sees a
/// frame soon.
fn drain(rx: Receiver<Vec<String>>, mut segments: Segments, dropped: &Dropped) {
    while let Ok(lines) = rx.recv() {
        let mut next = Some(lines);
        while let Some(lines) = next.take() {
            let behind = dropped.records.swap(0, Ordering::Relaxed);
            if behind > 0 {
                let tick = dropped.tick.load(Ordering::Relaxed);
                let line = format!(
                    r#"{{"ch":"dropped","t":{},"tick":{tick},"records":{behind}}}"#,
                    now_ms()
                );
                segments.write(&line);
            }
            for line in &lines {
                segments.write(line);
            }
            next = rx.try_recv().ok();
        }
        segments.flush();
        segments.prune(now_ms());
    }
    segments.flush();
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
