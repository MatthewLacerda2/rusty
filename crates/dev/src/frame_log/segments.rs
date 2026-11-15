//! The frame log's segment files (#2206), owned by the writer thread: numbered
//! `frames-<n>.jsonl`, rotated at a tenth of the budget, pruned oldest first past
//! the budget or the period, under the folder's lock (`writer`'s module doc).

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use super::now_ms;

/// Segments a budget is cut into.
const SEGMENTS: u64 = 10;

/// The lock file a log holds in its folder.
pub const LOCK: &str = ".lock";

/// One closed segment: its number, its bytes and its newest record's time.
pub(super) struct Closed {
    pub(super) n: u64,
    pub(super) bytes: u64,
    pub(super) newest: u64,
}

/// The segment files: the closed ones oldest first, and the one being written.
pub(super) struct Segments {
    dir: PathBuf,
    budget: u64,
    per_segment: u64,
    period: Option<u64>,
    pub(super) closed: VecDeque<Closed>,
    open: Option<(u64, BufWriter<File>, u64)>,
    next: u64,
    failed: bool,
    /// The folder's [`LOCK`], held while the log runs.
    _lock: File,
}

impl Segments {
    pub(super) fn open(dir: &Path, budget: u64, period: Option<u64>) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let lock = lock(dir)?;
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
            _lock: lock,
        };
        segments.prune(now_ms());
        Ok(segments)
    }

    fn path(&self, n: u64) -> PathBuf {
        segment_path(&self.dir, n)
    }

    /// Append one line, rotating first when it would overfill the open segment.
    pub(super) fn write(&mut self, line: &str) {
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
    pub(super) fn close(&mut self) {
        if let Some((n, out, bytes)) = self.open.take() {
            match out.into_inner() {
                Ok(file) => drop(file.sync_data()),
                Err(err) => self.fail(err.error()),
            }
            let newest = now_ms();
            self.closed.push_back(Closed { n, bytes, newest });
            self.prune(newest);
        }
    }

    pub(super) fn flush(&mut self) {
        if let Some((_, out, _)) = self.open.as_mut() {
            if let Err(err) = out.flush() {
                self.fail(&err);
            }
        }
    }

    /// Delete the oldest closed segments past the budget or the period. The open
    /// segment counts as full, so the log holds its budget until the next prune.
    pub(super) fn prune(&mut self, now: u64) {
        let open = self.per_segment;
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

/// Take `dir`'s [`LOCK`], or say another process holds it.
fn lock(dir: &Path) -> std::io::Result<File> {
    // lint: allow(asset-write) the frame log's lock file, never read as content
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err(std::io::Error::other(
            "another rusty process is writing this project's frame log; this one logs nothing",
        )),
        Err(std::fs::TryLockError::Error(err)) => Err(err),
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
