//! The frame log's writer (#2206): a thread that owns the segment files, so a
//! frame only hands its lines over and never waits on a disk.
//!
//! Segments are `frames-<n>.jsonl`, numbered up; a log that starts continues after
//! the highest number it finds and keeps the earlier ones under its budget, so a
//! look-back reaches across a restart. Each holds about a tenth of the budget; when
//! it is full it is synced and closed and the next one opens, then the oldest are
//! deleted until the log fits the budget again, and any whose newest record is
//! older than the period.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::JoinHandle;

use super::now_ms;

/// How many frames' lines may wait for the writer before a frame's are dropped.
const QUEUE: usize = 240;

/// Segments a budget is cut into.
const SEGMENTS: u64 = 10;

/// The frame's side: a bounded channel into the writer thread.
pub struct Writer {
    dir: PathBuf,
    tx: Option<SyncSender<Vec<String>>>,
    dropped: Arc<AtomicU64>,
    dropped_total: u64,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    /// Open the log in `dir` (made if missing) under `budget` bytes and, when
    /// given, `period` ms, and start its thread.
    pub fn start(dir: PathBuf, budget: u64, period: Option<u64>) -> std::io::Result<Self> {
        let segments = Segments::open(&dir, budget, period)?;
        let (tx, rx) = sync_channel(QUEUE);
        let dropped = Arc::new(AtomicU64::new(0));
        let behind = Arc::clone(&dropped);
        let thread = std::thread::Builder::new()
            .name("frame-log".into())
            .spawn(move || drain(rx, segments, &behind))?;
        Ok(Self {
            dir,
            tx: Some(tx),
            dropped,
            dropped_total: 0,
            thread: Some(thread),
        })
    }

    /// The folder the log writes.
    pub fn dir(&self) -> &PathBuf {
        &self.dir
    }

    /// Records dropped so far because the writer was behind.
    pub fn dropped(&self) -> u64 {
        self.dropped_total
    }

    /// Hand one frame's lines to the writer, never waiting: when its queue is
    /// full they are dropped and counted.
    pub fn send(&mut self, lines: Vec<String>) {
        let (Some(tx), false) = (&self.tx, lines.is_empty()) else {
            return;
        };
        let n = lines.len() as u64;
        match tx.try_send(lines) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(n, Ordering::Relaxed);
                self.dropped_total += n;
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
/// them, and flush whenever the queue is empty, so a reader sees a frame soon.
fn drain(rx: Receiver<Vec<String>>, mut segments: Segments, dropped: &AtomicU64) {
    while let Ok(lines) = rx.recv() {
        let mut next = Some(lines);
        while let Some(lines) = next.take() {
            let behind = dropped.swap(0, Ordering::Relaxed);
            if behind > 0 {
                let line = format!(r#"{{"ch":"dropped","t":{},"records":{behind}}}"#, now_ms());
                segments.write(&line);
            }
            for line in &lines {
                segments.write(line);
            }
            next = rx.try_recv().ok();
        }
        segments.flush();
    }
    segments.flush();
}

/// One closed segment: its number, its bytes and its newest record's time.
struct Closed {
    n: u64,
    bytes: u64,
    newest: u64,
}

/// The segment files: the closed ones oldest first, and the one being written.
struct Segments {
    dir: PathBuf,
    budget: u64,
    per_segment: u64,
    period: Option<u64>,
    closed: VecDeque<Closed>,
    open: Option<(u64, BufWriter<File>, u64)>,
    next: u64,
    failed: bool,
}

impl Segments {
    fn open(dir: &Path, budget: u64, period: Option<u64>) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let closed: VecDeque<Closed> = numbered(dir)
            .into_iter()
            .map(|(n, path)| {
                let meta = std::fs::metadata(&path).ok();
                let newest = meta.as_ref().and_then(|m| m.modified().ok());
                let newest = newest
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis() as u64);
                let bytes = meta.map_or(0, |m| m.len());
                Closed { n, bytes, newest }
            })
            .collect();
        let next = closed.back().map_or(0, |c| c.n + 1);
        let per_segment = (budget / SEGMENTS).max(1);
        let mut segments = Self {
            dir: dir.to_path_buf(),
            budget,
            per_segment,
            period,
            closed,
            open: None,
            next,
            failed: false,
        };
        segments.prune(now_ms());
        Ok(segments)
    }

    fn path(&self, n: u64) -> PathBuf {
        segment_path(&self.dir, n)
    }

    /// Append one line, rotating first when it would overfill the open segment.
    fn write(&mut self, line: &str) {
        let len = line.len() as u64 + 1;
        if self
            .open
            .as_ref()
            .is_some_and(|(_, _, bytes)| *bytes > 0 && bytes + len > self.per_segment)
        {
            self.close();
        }
        if self.open.is_none() && !self.start_segment() {
            return;
        }
        let Some((_, out, bytes)) = self.open.as_mut() else {
            return;
        };
        match writeln!(out, "{line}") {
            Ok(()) => *bytes += len,
            Err(err) => self.fail(&err),
        }
        self.prune(now_ms());
    }

    fn start_segment(&mut self) -> bool {
        let (n, path) = (self.next, self.path(self.next));
        // lint: allow(asset-write) the frame log, read by `Debug.FrameLog`, never by a path-keyed cache
        let file = OpenOptions::new().create(true).append(true).open(&path);
        match file {
            Ok(file) => {
                self.open = Some((n, BufWriter::new(file), 0));
                self.next += 1;
                true
            }
            Err(err) => {
                self.fail(&err);
                false
            }
        }
    }

    /// Close the open segment, flushed and synced, so a power cut after this
    /// leaves it whole.
    fn close(&mut self) {
        if let Some((n, out, bytes)) = self.open.take() {
            match out.into_inner() {
                Ok(file) => drop(file.sync_data()),
                Err(err) => self.fail(err.error()),
            }
            let newest = now_ms();
            self.closed.push_back(Closed { n, bytes, newest });
        }
    }

    fn flush(&mut self) {
        if let Some((_, out, _)) = self.open.as_mut() {
            if let Err(err) = out.flush() {
                self.fail(&err);
            }
        }
    }

    /// Delete the oldest closed segments past the budget or the period.
    fn prune(&mut self, now: u64) {
        let open = self.open.as_ref().map_or(0, |(_, _, bytes)| *bytes);
        let mut total: u64 = self.closed.iter().map(|c| c.bytes).sum::<u64>() + open;
        while let Some(oldest) = self.closed.front() {
            let stale = self.period.is_some_and(|p| oldest.newest + p < now);
            if total <= self.budget && !stale {
                break;
            }
            total -= oldest.bytes;
            let _ = std::fs::remove_file(self.path(oldest.n));
            self.closed.pop_front();
        }
    }

    /// Say a write failed, once: the log then keeps trying without repeating it.
    fn fail(&mut self, err: &std::io::Error) {
        if !self.failed {
            self.failed = true;
            log::warn!("[FrameLog] writing {}: {err}", self.dir.display());
        }
    }
}

/// The path of segment `n` in `dir`.
pub fn segment_path(dir: &Path, n: u64) -> PathBuf {
    dir.join(format!("frames-{n}.jsonl"))
}

/// The segments in `dir`, by number, oldest first.
pub fn numbered(dir: &Path) -> Vec<(u64, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let n = name
                .to_str()?
                .strip_prefix("frames-")?
                .strip_suffix(".jsonl")?;
            Some((n.parse().ok()?, entry.path()))
        })
        .collect();
    found.sort_by_key(|(n, _)| *n);
    found
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
